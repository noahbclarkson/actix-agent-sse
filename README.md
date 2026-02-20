# actix-agent-sse

[![crates.io](https://img.shields.io/crates/v/actix-agent-sse.svg)](https://crates.io/crates/actix-agent-sse)
[![docs.rs](https://docs.rs/actix-agent-sse/badge.svg)](https://docs.rs/actix-agent-sse)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](https://opensource.org/licenses/MIT)

SSE streaming middleware for actix-web AI agents.

## Problem

Building AI agents with streaming responses in actix-web requires boilerplate:
- Setting up SSE headers correctly
- Converting events to SSE format
- Handling client disconnects
- Managing channels between agent tasks and HTTP responses

**actix-agent-sse** solves this with a clean, reusable API.

## Quick Start

```rust
use actix_web::{web, App, HttpServer};
use actix_agent_sse::{AgentEvent, sse_response_with_abort};
use tokio::sync::mpsc;

async fn stream_handler() -> actix_web::HttpResponse {
    let (tx, rx) = mpsc::channel(32);
    let (response, abort) = sse_response_with_abort(rx);
    
    tokio::spawn(async move {
        let _ = tx.send(AgentEvent::text("Hello!")).await;
        let _ = tx.send(AgentEvent::done("msg-123")).await;
    });
    
    response
}
```

## AgentEvent Variants

| Variant | SSE Event Type | Use Case |
|---------|---------------|----------|
| `TextDelta` | `text_delta` | Streaming AI text content |
| `ToolStart` | `tool_start` | Tool invocation starting |
| `ToolResult` | `tool_result` | Tool execution completed |
| `Done` | `done` | Stream complete |
| `Error` | `error` | Error with recovery hint |

## SseRelay: Background Agent Pattern

When your agent should continue after client disconnect:

```rust
use actix_agent_sse::{SseRelay, AgentEvent, sse_response};

async fn background_agent() -> actix_web::HttpResponse {
    let (relay, rx) = SseRelay::new(64);
    
    tokio::spawn(async move {
        for chunk in process_items() {
            if relay.is_aborted() {
                // Client gone, but continue work
                relay.send(AgentEvent::text(chunk)).await.ok();
            }
        }
    });
    
    sse_response(rx)
}
```

## Custom Events

Implement `SseEvent` for your own types:

```rust
use actix_agent_sse::SseEvent;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
enum MyEvent {
    Progress { percent: u8 },
}

impl SseEvent for MyEvent {
    fn event_type(&self) -> &str {
        "progress"
    }
}
```

## Features

- **`SseEvent` trait** - Flexible event serialization
- **`AgentEvent` enum** - Built-in AI agent events
- **`sse_response()`** - Simple SSE response creation
- **`sse_response_with_abort()`** - Client disconnect detection
- **`SseRelay`** - Background agent pattern support
- **`SseResponseBuilder`** - Custom headers, keep-alive config

## License

MIT OR Apache-2.0
