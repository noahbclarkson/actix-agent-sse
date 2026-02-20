//! Relay channel for background-agent SSE streaming.
//!
//! This module provides [`SseRelay`], a channel abstraction that combines
//! event sending with client disconnect detection. This is useful for
//! the "background agent" pattern where the agent continues running even
//! if the client disconnects.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::event::SseEvent;
use crate::response::AbortSignal;

/// Error type for relay operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RelayError {
    /// The client has disconnected and the channel is closed.
    #[error("Client disconnected")]
    Disconnected,

    /// The channel buffer is full and the send failed.
    #[error("Channel full")]
    ChannelFull,
}

/// Relay channel for SSE streaming with disconnect detection.
///
/// SseRelay combines an mpsc sender with an abort signal, allowing
/// agents to both send events and check client disconnect state.
///
/// # Background Agent Pattern
///
/// Use SseRelay when your agent should continue working even if the
/// client disconnects (e.g., for logging, cleanup, or completing a task).
///
/// ```rust,no_run
/// use actix_agent_sse::{SseRelay, AgentEvent};
///
/// // Create relay with buffer for 64 events
/// let (relay, rx) = SseRelay::new(64);
///
/// // Pass rx to sse_response, keep relay for agent
/// // let response = sse_response(rx);
///
/// // In your agent task:
/// tokio::spawn(async move {
///     // Agent can check disconnect state
///     if relay.is_aborted() {
///         // Client gone, but can still buffer events
///         // or gracefully complete work
///     }
///     
///     relay.send(AgentEvent::text("Hello")).await.ok();
/// });
/// ```
pub struct SseRelay<E: SseEvent> {
    tx: mpsc::Sender<E>,
    abort: AbortSignal,
}

impl<E: SseEvent> SseRelay<E> {
    /// Create a new relay channel.
    ///
    /// Returns a tuple of (relay, receiver). Pass the receiver to
    /// [`sse_response()`](crate::sse_response) or
    /// [`sse_response_with_abort()`](crate::sse_response_with_abort).
    ///
    /// # Arguments
    ///
    /// * `buffer` - Maximum number of events to buffer before backpressure.
    ///
    /// # Example
    ///
    /// ```rust
    /// use actix_agent_sse::{SseRelay, AgentEvent};
    ///
    /// let (relay, rx) = SseRelay::<AgentEvent>::new(32);
    /// ```
    pub fn new(buffer: usize) -> (Self, mpsc::Receiver<E>) {
        let (tx, rx) = mpsc::channel(buffer);
        let abort = AbortSignal {
            aborted: Arc::new(AtomicBool::new(false)),
        };
        (Self { tx, abort }, rx)
    }

    /// Create a relay from existing channel and abort signal.
    ///
    /// This is useful when you want to use [`sse_response_with_abort()`](crate::sse_response_with_abort)
    /// but also want the relay abstraction.
    pub fn from_parts(tx: mpsc::Sender<E>, abort: AbortSignal) -> Self {
        Self { tx, abort }
    }

    /// Send an event to the SSE stream.
    ///
    /// # Errors
    ///
    /// Returns [`RelayError::Disconnected`] if the receiver has been dropped
    /// (e.g., after the HTTP response completes).
    ///
    /// # Example
    ///
    /// ```rust
    /// use actix_agent_sse::{SseRelay, AgentEvent};
    ///
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// let (relay, rx) = SseRelay::<AgentEvent>::new(32);
    /// 
    /// // Drop rx to simulate disconnect
    /// drop(rx);
    /// 
    /// let result = relay.send(AgentEvent::text("test")).await;
    /// assert!(result.is_err());
    /// # });
    /// ```
    pub async fn send(&self, event: E) -> Result<(), RelayError> {
        self.tx.send(event).await.map_err(|_| RelayError::Disconnected)
    }

    /// Try to send an event without waiting.
    ///
    /// Returns immediately with success or error.
    pub fn try_send(&self, event: E) -> Result<(), RelayError> {
        self.tx.try_send(event).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => RelayError::ChannelFull,
            mpsc::error::TrySendError::Closed(_) => RelayError::Disconnected,
        })
    }

    /// Check if the client has disconnected.
    pub fn is_aborted(&self) -> bool {
        self.abort.is_aborted()
    }

    /// Wait for the client to disconnect.
    pub async fn wait_for_abort(&self) {
        self.abort.wait().await
    }

    /// Get a reference to the abort signal.
    pub fn abort_signal(&self) -> &AbortSignal {
        &self.abort
    }

    /// Get a clone of the sender for use in multiple tasks.
    pub fn sender(&self) -> mpsc::Sender<E> {
        self.tx.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentEvent;

    #[tokio::test]
    async fn test_relay_send_receive() {
        let (relay, mut rx) = SseRelay::<AgentEvent>::new(32);
        
        relay.send(AgentEvent::text("hello")).await.unwrap();
        
        let event = rx.recv().await.unwrap();
        match event {
            AgentEvent::TextDelta { content } => assert_eq!(content, "hello"),
            _ => panic!("Wrong event type"),
        }
    }

    #[tokio::test]
    async fn test_relay_disconnect() {
        let (relay, rx) = SseRelay::<AgentEvent>::new(32);
        drop(rx);
        
        let result = relay.send(AgentEvent::text("test")).await;
        assert!(matches!(result, Err(RelayError::Disconnected)));
    }

    #[test]
    fn test_try_send() {
        let (relay, mut rx) = SseRelay::<AgentEvent>::new(1);
        
        relay.try_send(AgentEvent::text("first")).unwrap();
        // Buffer is 1, second should fail (or succeed if async queue allows)
        let _ = relay.try_send(AgentEvent::text("second"));
        
        let event = rx.blocking_recv().unwrap();
        match event {
            AgentEvent::TextDelta { content } => assert_eq!(content, "first"),
            _ => panic!("Wrong event type"),
        }
    }
}
