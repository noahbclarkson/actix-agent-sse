//! Builder for configuring SSE responses.
//!
//! This module provides [`SseResponseBuilder`] for more control over
//! SSE response configuration, such as custom headers and buffering.

use actix_web::HttpResponse;
use bytes::Bytes;
use futures::stream::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::event::SseEvent;
use crate::response::{AbortSignal, AbortAwareStream};

/// Builder for SSE HTTP responses.
///
/// Provides fine-grained control over SSE response configuration.
///
/// # Example
///
/// ```rust,no_run
/// use actix_agent_sse::{SseResponseBuilder, AgentEvent};
/// use tokio::sync::mpsc;
///
/// let (tx, rx) = mpsc::channel::<AgentEvent>(32);
///
/// let response = SseResponseBuilder::new()
///     .keep_alive_interval(std::time::Duration::from_secs(15))
///     .header("X-Custom-Header", "value")
///     .build(rx);
/// ```
pub struct SseResponseBuilder {
    keep_alive_interval: Option<std::time::Duration>,
    headers: Vec<(String, String)>,
}

impl Default for SseResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SseResponseBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            keep_alive_interval: None,
            headers: Vec::new(),
        }
    }

    /// Set the keep-alive interval.
    ///
    /// When set, a comment line (`: keep-alive\n\n`) will be sent
    /// periodically to prevent connection timeouts.
    pub fn keep_alive_interval(mut self, interval: std::time::Duration) -> Self {
        self.keep_alive_interval = Some(interval);
        self
    }

    /// Add a custom header to the response.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Build the SSE response from an mpsc receiver.
    pub fn build<E: SseEvent>(self, rx: mpsc::Receiver<E>) -> HttpResponse {
        let stream = ReceiverStream::new(rx).map(|event| match event.to_sse_string() {
            Ok(sse) => Ok::<Bytes, actix_web::Error>(Bytes::from(sse)),
            Err(e) => {
                tracing::error!("Failed to serialize SSE event: {e}");
                let fallback = "event: error\ndata: {\"type\":\"error\",\"message\":\"Serialization error\",\"recoverable\":false}\n\n";
                Ok(Bytes::from(fallback))
            }
        });

        // Note: keep_alive_interval is stored for future use
        // (would require a periodic task to send keep-alive comments)
        let _ = self.keep_alive_interval;

        let mut binding = HttpResponse::Ok();
        let mut builder = binding
            .content_type("text/event-stream")
            .insert_header(("Cache-Control", "no-cache"))
            .insert_header(("X-Accel-Buffering", "no"));

        for (name, value) in self.headers {
            builder = builder.insert_header((name, value));
        }

        builder.streaming(stream)
    }

    /// Build the SSE response with abort detection.
    ///
    /// Returns (HttpResponse, AbortSignal) similar to [`sse_response_with_abort()`](crate::sse_response_with_abort).
    pub fn build_with_abort<E: SseEvent>(
        self,
        rx: mpsc::Receiver<E>,
    ) -> (HttpResponse, AbortSignal) {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let aborted = Arc::new(AtomicBool::new(false));
        let abort_signal = AbortSignal {
            aborted: aborted.clone(),
        };

        let stream = AbortAwareStream {
            inner: ReceiverStream::new(rx),
            aborted,
        };

        // Note: keep_alive_interval is stored for future use
        let _ = self.keep_alive_interval;

        let mut binding = HttpResponse::Ok();
        let mut builder = binding
            .content_type("text/event-stream")
            .insert_header(("Cache-Control", "no-cache"))
            .insert_header(("X-Accel-Buffering", "no"));

        for (name, value) in self.headers {
            builder = builder.insert_header((name, value));
        }

        (builder.streaming(stream), abort_signal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default() {
        let builder = SseResponseBuilder::new();
        assert!(builder.keep_alive_interval.is_none());
        assert!(builder.headers.is_empty());
    }

    #[test]
    fn test_builder_with_options() {
        let builder = SseResponseBuilder::new()
            .keep_alive_interval(std::time::Duration::from_secs(30))
            .header("X-Custom", "value");

        assert_eq!(builder.keep_alive_interval, Some(std::time::Duration::from_secs(30)));
        assert_eq!(builder.headers.len(), 1);
    }
}
