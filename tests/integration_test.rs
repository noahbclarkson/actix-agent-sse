//! Integration tests for actix-agent-sse.

use actix_agent_sse::{AgentEvent, SseEvent, SseRelay, RelayError, sse_response_with_abort};
use tokio::sync::mpsc;

mod event_tests {
    use super::*;

    #[test]
    fn test_agent_event_text_delta_serialization() {
        let event = AgentEvent::text("Hello, world!");
        assert_eq!(event.event_type(), "text_delta");
        
        let sse = event.to_sse_string().unwrap();
        assert!(sse.starts_with("event: text_delta\n"));
        assert!(sse.contains(r#""content":"Hello, world!"#));
        assert!(sse.ends_with("\n\n"));
    }

    #[test]
    fn test_agent_event_tool_start_serialization() {
        let event = AgentEvent::tool_start("search", "Searching documentation");
        assert_eq!(event.event_type(), "tool_start");
        
        let sse = event.to_sse_string().unwrap();
        assert!(sse.contains("event: tool_start"));
        assert!(sse.contains(r#""tool_name":"search"#));
        assert!(sse.contains(r#""description":"Searching documentation"#));
    }

    #[test]
    fn test_agent_event_tool_result_serialization() {
        let event = AgentEvent::tool_result("search", "Found 5 results");
        let sse = event.to_sse_string().unwrap();
        assert!(sse.contains("event: tool_result"));
        assert!(sse.contains(r#""summary":"Found 5 results"#));
    }

    #[test]
    fn test_agent_event_done_serialization() {
        let event = AgentEvent::done("msg-abc123");
        assert_eq!(event.event_type(), "done");
        
        let sse = event.to_sse_string().unwrap();
        assert!(sse.contains("event: done"));
        assert!(sse.contains(r#""id":"msg-abc123"#));
    }

    #[test]
    fn test_agent_event_error_serialization() {
        let event = AgentEvent::error("API rate limit exceeded", true);
        assert_eq!(event.event_type(), "error");
        
        let sse = event.to_sse_string().unwrap();
        assert!(sse.contains("event: error"));
        assert!(sse.contains(r#""message":"API rate limit exceeded"#));
        assert!(sse.contains(r#""recoverable":true"#));
    }

    #[test]
    fn test_sse_format_double_newline() {
        // SSE spec requires \n\n after each event
        let events = vec![
            AgentEvent::text("test"),
            AgentEvent::done("id"),
            AgentEvent::error("err", false),
        ];
        
        for event in events {
            let sse = event.to_sse_string().unwrap();
            assert!(sse.ends_with("\n\n"), "Event {:?} should end with \\n\\n", event);
        }
    }
}

mod relay_tests {
    use super::*;

    #[tokio::test]
    async fn test_relay_send_and_receive() {
        let (relay, mut rx) = SseRelay::<AgentEvent>::new(32);
        
        relay.send(AgentEvent::text("first")).await.unwrap();
        relay.send(AgentEvent::done("id-1")).await.unwrap();
        
        let event1 = rx.recv().await.unwrap();
        match event1 {
            AgentEvent::TextDelta { content } => assert_eq!(content, "first"),
            _ => panic!("Expected TextDelta"),
        }
        
        let event2 = rx.recv().await.unwrap();
        match event2 {
            AgentEvent::Done { id } => assert_eq!(id, "id-1"),
            _ => panic!("Expected Done"),
        }
    }

    #[tokio::test]
    async fn test_relay_detects_disconnect() {
        let (relay, rx) = SseRelay::<AgentEvent>::new(32);
        
        // Drop the receiver to simulate client disconnect
        drop(rx);
        
        let result = relay.send(AgentEvent::text("test")).await;
        assert!(matches!(result, Err(RelayError::Disconnected)));
    }

    #[test]
    fn test_relay_try_send() {
        let (relay, mut rx) = SseRelay::<AgentEvent>::new(1);
        
        // First send should succeed
        relay.try_send(AgentEvent::text("first")).unwrap();
        
        // Receive it
        let event = rx.blocking_recv().unwrap();
        match event {
            AgentEvent::TextDelta { content } => assert_eq!(content, "first"),
            _ => panic!("Expected TextDelta"),
        }
    }

    #[tokio::test]
    async fn test_relay_abort_signal() {
        let (relay, _rx) = SseRelay::<AgentEvent>::new(32);
        
        // Initially not aborted
        assert!(!relay.is_aborted());
        
        // Abort signal should be accessible
        let signal = relay.abort_signal();
        assert!(!signal.is_aborted());
    }
}

mod abort_signal_tests {
    use super::*;

    #[test]
    fn test_abort_signal_initial_state() {
        let (_tx, rx) = mpsc::channel::<AgentEvent>(32);
        let (_response, signal) = sse_response_with_abort(rx);
        assert!(!signal.is_aborted());
    }

    #[test]
    fn test_abort_signal_clone() {
        let (_tx, rx) = mpsc::channel::<AgentEvent>(32);
        let (_response, signal) = sse_response_with_abort(rx);
        let signal2 = signal.clone();
        assert!(!signal2.is_aborted());
    }
}

mod response_tests {
    use super::*;

    #[test]
    fn test_sse_response_with_abort_returns_signal() {
        let (_tx, rx) = mpsc::channel::<AgentEvent>(32);
        let (response, abort) = sse_response_with_abort(rx);
        
        // Response should have correct content type
        assert!(response.headers().contains_key("content-type"));
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/event-stream"
        );
        
        // Abort signal should start as not aborted
        assert!(!abort.is_aborted());
    }
}

mod custom_event_tests {
    use super::*;
    use serde::Serialize;

    #[derive(Debug, Clone, Serialize)]
    #[serde(tag = "type")]
    enum CustomEvent {
        Progress { percent: u32 },
        Complete { result: String },
    }

    impl SseEvent for CustomEvent {
        fn event_type(&self) -> &str {
            "custom"
        }
    }

    #[test]
    fn test_custom_event_serialization() {
        let event = CustomEvent::Progress { percent: 50 };
        let sse = event.to_sse_string().unwrap();
        
        assert!(sse.contains("event: custom"));
        assert!(sse.contains(r#""percent":50"#));
    }

    #[test]
    fn test_custom_event_with_relay() {
        let (relay, mut rx) = SseRelay::<CustomEvent>::new(32);
        
        relay.try_send(CustomEvent::Progress { percent: 75 }).unwrap();
        
        let event = rx.blocking_recv().unwrap();
        match event {
            CustomEvent::Progress { percent } => assert_eq!(percent, 75),
            CustomEvent::Complete { .. } => panic!("Expected Progress"),
        }
    }
}
