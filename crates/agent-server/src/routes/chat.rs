use axum::extract::State;
use axum::response::sse::{Event as SseEvent, Sse};
use axum::Json;
use futures::stream::Stream;
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio_stream::StreamExt;
use tracing::info;

use crate::error::ApiError;
use crate::state::AppState;

/// Stream wrapper that decrements the active SSE connection counter when
/// the client disconnects (via `Drop`).
struct SseConnectionGuard<S> {
    inner: S,
    counter: Arc<AtomicU64>,
}

impl<S: Stream + Unpin> Stream for SseConnectionGuard<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().inner).poll_next(cx)
    }
}

impl<S> Drop for SseConnectionGuard<S> {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Chat request body for both non-streaming and streaming endpoints.
///
/// If `session_id` is omitted, a new session is auto-created with
/// default parameters and returned in the response.
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    /// Optional: existing session ID. Omit to auto-create a new session.
    pub session_id: Option<String>,
    /// The user message content.
    pub message: String,
    /// Optional: override the session's default model.
    #[serde(default)]
    pub model: Option<String>,
}

/// `POST /api/chat` -- send a message and get the full AI response.
///
/// Auto-creates a session when no `session_id` is provided. Increments
/// the `web` channel message counter for the monitoring dashboard.
pub async fn send_message(
    State(state): State<AppState>,
    Json(body): Json<ChatRequest>,
) -> Result<Json<Value>, ApiError> {
    // Validate message length to prevent memory exhaustion from oversized inputs.
    const MAX_MESSAGE_LEN: usize = 64 * 1024; // 64 KiB max per message
    if body.message.len() > MAX_MESSAGE_LEN {
        return Err(ApiError::BadRequest(format!(
            "Message too long: {} bytes (max {} bytes)",
            body.message.len(),
            MAX_MESSAGE_LEN
        )));
    }

    let session_id = match body.session_id {
        Some(id) => id,
        None => {
            // Auto-create a session if none provided
            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().to_rfc3339();
            let session = agent_db::models::SessionRow {
                id: id.clone(),
                name: "New Chat".into(),
                agent_id: None,
                system_prompt: None,
                model: body.model.clone().unwrap_or_else(|| "deepseek-chat".into()),
                temperature: 0.7,
                max_tokens: 4096,
                channel: "web".into(),
                channel_chat_id: None,
                created_at: now.clone(),
                updated_at: now,
            };
            state.session_repo.create(&session).await?;
            id
        }
    };

    // Track per-channel message
    state.increment_channel_msg("web").await;

    info!("Processing message for session {}", session_id);

    // Broadcast message_received event
    state.broadcast_event(
        "message_received",
        json!({
            "session_id": session_id,
            "role": "user",
            "content_preview": &body.message[..body.message.len().min(200)],
        }),
    );

    let response = state
        .session_manager
        .process_message(&session_id, &body.message, body.model.as_deref())
        .await?;

    // Broadcast message_sent event
    state.broadcast_event(
        "message_sent",
        json!({
            "session_id": session_id,
            "role": "assistant",
            "content_preview": &response[..response.len().min(200)],
        }),
    );

    Ok(Json(json!({
        "session_id": session_id,
        "message": response,
    })))
}

/// `POST /api/chat/stream` -- send a message and receive an SSE (Server-Sent
/// Events) stream in response.
///
/// The stream yields JSON events discriminated by `"type"`:
/// `"thinking"`, `"delta"`, `"tool_start"`, `"tool_end"`, `"done"`, `"error"`.
/// The response includes a 15-second keep-alive heartbeat. Active connections
/// are tracked for the monitoring dashboard.
pub async fn stream_message(
    State(state): State<AppState>,
    Json(body): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, ApiError> {
    // Validate message length to prevent memory exhaustion from oversized inputs.
    const MAX_MESSAGE_LEN: usize = 64 * 1024; // 64 KiB max per message
    if body.message.len() > MAX_MESSAGE_LEN {
        return Err(ApiError::BadRequest(format!(
            "Message too long: {} bytes (max {} bytes)",
            body.message.len(),
            MAX_MESSAGE_LEN
        )));
    }

    let session_id = match body.session_id {
        Some(id) => id,
        None => {
            // Auto-create a session if none provided
            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().to_rfc3339();
            let session = agent_db::models::SessionRow {
                id: id.clone(),
                name: "New Chat".into(),
                agent_id: None,
                system_prompt: None,
                model: body.model.clone().unwrap_or_else(|| "deepseek-chat".into()),
                temperature: 0.7,
                max_tokens: 4096,
                channel: "web".into(),
                channel_chat_id: None,
                created_at: now.clone(),
                updated_at: now,
            };
            state.session_repo.create(&session).await?;
            id
        }
    };

    // Track per-channel message and SSE connection
    state.increment_channel_msg("web").await;
    state.sse_connected();

    info!("Streaming message for session {}", session_id);

    let event_stream = state.session_manager.process_message_stream(
        &session_id,
        &body.message,
        body.model.as_deref(),
    );

    let sse_stream = event_stream.map(|result| {
        let event = match result {
            Ok(value) => SseEvent::default().data(value.to_string()),
            Err(e) => SseEvent::default()
                .data(json!({"type": "error", "message": e.to_string()}).to_string()),
        };
        Ok(event)
    });

    // Wrap the stream so we decrement the SSE counter when the client disconnects
    let guarded_stream = SseConnectionGuard {
        inner: sse_stream,
        counter: state.active_sse_connections.clone(),
    };

    Ok(Sse::new(guarded_stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keep-alive"),
    ))
}
