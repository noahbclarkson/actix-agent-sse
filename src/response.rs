//! SSE response utilities for actix-web.
//!
//! This module provides functions to create SSE HTTP responses from
//! event channels, with optional client disconnect detection.

use actix_web::HttpResponse;
use bytes::Bytes;
use futures::stream::StreamExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::event::SseEvent;

/// Signal that tracks client disconnect state.
///
/// Created by [`sse_response_with_abort()`], this allows the sender
/// to detect when the client has disconnected.
///
/// # Example
///
/// ```rust,no_run
/// use actix_agent_sse::{sse_response_with_abort, AgentEvent};
/// use tokio::sync::mpsc;
///
/// let (tx, rx) = mpsc::channel::<AgentEvent>(32);
/// let (response, abort) = sse_response_with_abort(rx);
///
/// // In your agent task:
/// if abort.is_aborted() {
///     // Client disconnected, clean up
/// }
/// ```
#[derive(Clone)]
pub struct AbortSignal {
    pub(crate) aborted: Arc<AtomicBool>,
}

impl AbortSignal {
    /// Check if the client has disconnected.
    pub fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::Relaxed)
    }

    /// Wait asynchronously until the client disconnects.
    ///
    /// This is a convenience method that polls the abort state.
    pub async fn wait(&self) {
        while !self.is_aborted() {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }
}

/// Wrapper stream that sets abort flag on drop.
pub(crate) struct AbortAwareStream<E: SseEvent> {
    pub(crate) inner: ReceiverStream<E>,
    pub(crate) aborted: Arc<AtomicBool>,
}

impl<E: SseEvent> futures::Stream for AbortAwareStream<E> {
    type Item = Result<Bytes, actix_web::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx).map(|opt| {
            opt.map(|event| match event.to_sse_string() {
                Ok(sse) => Ok(Bytes::from(sse)),
                Err(e) => {
                    tracing::error!("Failed to serialize SSE event: {e}");
                    let fallback = "event: error\ndata: {\"type\":\"error\",\"message\":\"Serialization error\",\"recoverable\":false}\n\n";
                    Ok(Bytes::from(fallback))
                }
            })
        })
    }
}

impl<E: SseEvent> Drop for AbortAwareStream<E> {
    fn drop(&mut self) {
        self.aborted.store(true, Ordering::Relaxed);
    }
}

/// Create an SSE HttpResponse from an mpsc receiver.
///
/// This is the simplest way to create an SSE response. Events from the
/// receiver are streamed to the client as SSE events.
///
/// # Example
///
/// ```rust,no_run
/// use actix_agent_sse::{sse_response, AgentEvent};
/// use tokio::sync::mpsc;
///
/// let (tx, rx) = mpsc::channel(32);
///
/// // Spawn agent that sends events
/// tokio::spawn(async move {
///     let _ = tx.send(AgentEvent::text("Processing...")).await;
///     let _ = tx.send(AgentEvent::done("msg-123")).await;
/// });
///
/// let response = sse_response(rx);
/// ```
pub fn sse_response<E: SseEvent>(rx: mpsc::Receiver<E>) -> HttpResponse {
    let stream = ReceiverStream::new(rx).map(|event| match event.to_sse_string() {
        Ok(sse) => Ok::<Bytes, actix_web::Error>(Bytes::from(sse)),
        Err(e) => {
            tracing::error!("Failed to serialize SSE event: {e}");
            let fallback = "event: error\ndata: {\"type\":\"error\",\"message\":\"Serialization error\",\"recoverable\":false}\n\n";
            Ok(Bytes::from(fallback))
        }
    });

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream)
}

/// Create an SSE HttpResponse with client disconnect detection.
///
/// Returns a tuple of (HttpResponse, AbortSignal). The AbortSignal can be
/// used to detect when the client has disconnected, allowing for cleanup
/// or early termination of background tasks.
///
/// # Example
///
/// ```rust,no_run
/// use actix_agent_sse::{sse_response_with_abort, AgentEvent};
/// use tokio::sync::mpsc;
///
/// let (tx, rx) = mpsc::channel(32);
/// let (response, abort) = sse_response_with_abort(rx);
///
/// // Spawn agent that checks for disconnect
/// tokio::spawn(async move {
///     for i in 0..100 {
///         if abort.is_aborted() {
///             println!("Client disconnected, stopping");
///             break;
///         }
///         let _ = tx.send(AgentEvent::text(format!("Chunk {}", i))).await;
///         tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
///     }
///     let _ = tx.send(AgentEvent::done("complete")).await;
/// });
///
/// // Return response to client
/// // return response;
/// ```
pub fn sse_response_with_abort<E: SseEvent>(
    rx: mpsc::Receiver<E>,
) -> (HttpResponse, AbortSignal) {
    let aborted = Arc::new(AtomicBool::new(false));
    let abort_signal = AbortSignal {
        aborted: aborted.clone(),
    };

    let stream = AbortAwareStream {
        inner: ReceiverStream::new(rx),
        aborted,
    };

    let response = HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream);

    (response, abort_signal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abort_signal_initial_state() {
        let signal = AbortSignal {
            aborted: Arc::new(AtomicBool::new(false)),
        };
        assert!(!signal.is_aborted());
    }

    #[test]
    fn test_abort_signal_set() {
        let flag = Arc::new(AtomicBool::new(false));
        let signal = AbortSignal {
            aborted: flag.clone(),
        };
        flag.store(true, Ordering::Relaxed);
        assert!(signal.is_aborted());
    }
}
