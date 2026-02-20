//! Event types and traits for SSE streaming.
//!
//! This module provides the [`SseEvent`] trait for custom event types
//! and the built-in [`AgentEvent`] enum for common AI agent patterns.

use serde::Serialize;

/// Trait for types that can be serialized as SSE events.
///
/// Events are serialized in the format:
/// ```text
/// event: {type}
/// data: {json}
///
/// ```
///
/// # Example
///
/// ```rust
/// use actix_agent_sse::SseEvent;
/// use serde::Serialize;
///
/// #[derive(Debug, Clone, Serialize)]
/// #[serde(tag = "type", rename_all = "snake_case")]
/// enum MyEvent {
///     Custom { value: i32 },
/// }
///
/// impl SseEvent for MyEvent {
///     fn event_type(&self) -> &str {
///         match self {
///             MyEvent::Custom { .. } => "custom",
///         }
///     }
/// }
/// ```
pub trait SseEvent: Serialize + Send + 'static {
    /// Returns the SSE event type name (used in `event:` field).
    fn event_type(&self) -> &str;

    /// Serialize this event as an SSE-formatted string.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    fn to_sse_string(&self) -> Result<String, serde_json::Error> {
        let data = serde_json::to_string(self)?;
        Ok(format!("event: {}\ndata: {}\n\n", self.event_type(), data))
    }
}

/// Built-in generic agent events for AI streaming.
///
/// This enum covers common patterns in AI agent applications.
/// For custom events, implement [`SseEvent`] on your own type.
///
/// # Variants
///
/// | Variant | SSE Event Type | Description |
/// |---------|---------------|-------------|
/// | `TextDelta` | `text_delta` | Streaming text content |
/// | `ToolStart` | `tool_start` | Tool execution starting |
/// | `ToolResult` | `tool_result` | Tool execution completed |
/// | `Done` | `done` | Stream complete |
/// | `Error` | `error` | Error occurred |
///
/// # Example
///
/// ```rust
/// use actix_agent_sse::AgentEvent;
/// use actix_agent_sse::SseEvent;
///
/// let event = AgentEvent::text("Hello, world!");
/// assert_eq!(event.event_type(), "text_delta");
/// let sse = event.to_sse_string().unwrap();
/// assert!(sse.starts_with("event: text_delta\ndata:"));
/// ```
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// AI text content being streamed (token by token or chunk by chunk).
    TextDelta {
        /// The text content to append.
        content: String,
    },

    /// AI is about to use a tool.
    ToolStart {
        /// Name of the tool being invoked.
        tool_name: String,
        /// Human-readable description of what the tool will do.
        description: String,
    },

    /// Tool execution completed.
    ToolResult {
        /// Name of the tool that completed.
        tool_name: String,
        /// Summary of the tool result.
        summary: String,
    },

    /// Stream complete.
    Done {
        /// Unique identifier for the completed message/session.
        id: String,
    },

    /// Error occurred during streaming.
    Error {
        /// Human-readable error message.
        message: String,
        /// Whether the error is recoverable and streaming can continue.
        recoverable: bool,
    },
}

impl AgentEvent {
    /// Create a text delta event.
    pub fn text(content: impl Into<String>) -> Self {
        AgentEvent::TextDelta {
            content: content.into(),
        }
    }

    /// Create a tool start event.
    pub fn tool_start(tool_name: impl Into<String>, description: impl Into<String>) -> Self {
        AgentEvent::ToolStart {
            tool_name: tool_name.into(),
            description: description.into(),
        }
    }

    /// Create a tool result event.
    pub fn tool_result(tool_name: impl Into<String>, summary: impl Into<String>) -> Self {
        AgentEvent::ToolResult {
            tool_name: tool_name.into(),
            summary: summary.into(),
        }
    }

    /// Create a done event.
    pub fn done(id: impl Into<String>) -> Self {
        AgentEvent::Done { id: id.into() }
    }

    /// Create an error event.
    pub fn error(message: impl Into<String>, recoverable: bool) -> Self {
        AgentEvent::Error {
            message: message.into(),
            recoverable,
        }
    }
}

impl SseEvent for AgentEvent {
    fn event_type(&self) -> &str {
        match self {
            AgentEvent::TextDelta { .. } => "text_delta",
            AgentEvent::ToolStart { .. } => "tool_start",
            AgentEvent::ToolResult { .. } => "tool_result",
            AgentEvent::Done { .. } => "done",
            AgentEvent::Error { .. } => "error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_delta_event() {
        let event = AgentEvent::text("Hello");
        assert_eq!(event.event_type(), "text_delta");
        let sse = event.to_sse_string().unwrap();
        assert!(sse.contains("event: text_delta"));
        assert!(sse.contains(r#""content":"Hello""#));
    }

    #[test]
    fn test_done_event() {
        let event = AgentEvent::done("msg-123");
        assert_eq!(event.event_type(), "done");
        let sse = event.to_sse_string().unwrap();
        assert!(sse.contains("event: done"));
        assert!(sse.contains(r#""id":"msg-123""#));
    }

    #[test]
    fn test_error_event() {
        let event = AgentEvent::error("Something went wrong", false);
        assert_eq!(event.event_type(), "error");
        let sse = event.to_sse_string().unwrap();
        assert!(sse.contains("event: error"));
        assert!(sse.contains(r#""recoverable":false"#));
    }

    #[test]
    fn test_sse_format() {
        let event = AgentEvent::text("test");
        let sse = event.to_sse_string().unwrap();
        assert!(sse.ends_with("\n\n"));
    }
}
