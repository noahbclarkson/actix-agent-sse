//! SSE streaming middleware for actix-web AI agents.
//!
//! This crate provides a standardized way to stream AI agent events to clients
//! using Server-Sent Events (SSE) with actix-web.
//!
//! # Features
//!
//! - Flexible event system via [`SseEvent`] trait
//! - Built-in [`AgentEvent`] enum for common agent patterns
//! - Simple [`sse_response()`] for basic streaming
//! - Abort-aware [`sse_response_with_abort()`] for client disconnect detection
//! - [`SseRelay`] for background agent pattern
//! - [`SseResponseBuilder`] for configuration
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use actix_web::{web, App, HttpServer};
//! use actix_agent_sse::{AgentEvent, sse_response_with_abort};
//! use tokio::sync::mpsc;
//!
//! async fn stream_handler() -> actix_web::HttpResponse {
//!     let (tx, rx) = mpsc::channel(32);
//!     let (response, _abort) = sse_response_with_abort(rx);
//!     
//!     // Spawn agent task that sends events
//!     tokio::spawn(async move {
//!         let _ = tx.send(AgentEvent::text("Hello!")).await;
//!         let _ = tx.send(AgentEvent::done("msg-123")).await;
//!     });
//!     
//!     response
//! }
//! ```

pub mod event;
pub mod response;
pub mod relay;
pub mod builder;

pub use event::{SseEvent, AgentEvent};
pub use response::{sse_response, sse_response_with_abort, AbortSignal};
pub use relay::{SseRelay, RelayError};
pub use builder::SseResponseBuilder;
