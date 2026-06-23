//! WebSocket endpoint for real-time event push to the frontend.
//!
//! `GET /api/ws` upgrades to a WebSocket connection. The server pushes
//! JSON events (session creation, messages, workflow execution, task runs,
//! channel messages) to all connected clients with a 15-second keep-alive
//! ping.

use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use serde_json::json;
use tracing::{debug, info, warn};

use crate::state::AppState;

/// `GET /api/ws` -- upgrade to a WebSocket and begin receiving real-time events.
///
/// The token for authentication is extracted from the `?token=` query parameter
/// by the global `authenticate` middleware before this handler runs.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    info!(
        "WebSocket upgrade request (active conns: {})",
        state.get_active_ws()
    );
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Core WebSocket event loop: subscribe to the broadcast channel, forward
/// events as JSON text frames, and send keep-alive pings every 15 seconds.
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.ws_tx.subscribe();
    state.ws_connected();
    info!(
        "WebSocket client connected (total: {})",
        state.get_active_ws()
    );

    let mut keep_alive = tokio::time::interval(Duration::from_secs(15));

    // Send an initial "connected" event so the client knows it is live.
    let connected_event = json!({
        "type": "connected",
        "data": {
            "active_connections": state.get_active_ws(),
            "server_time": chrono::Utc::now().to_rfc3339(),
        },
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    if socket
        .send(Message::Text(connected_event.to_string().into()))
        .await
        .is_err()
    {
        state.ws_disconnected();
        return;
    }

    loop {
        tokio::select! {
            _ = keep_alive.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    debug!("WebSocket keep-alive ping failed, client likely disconnected");
                    break;
                }
            }
            // Also handle incoming messages (close frames, pongs, etc.)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => {
                        debug!("WebSocket client sent close frame");
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        warn!("WebSocket receive error: {}", e);
                        break;
                    }
                    // Ignore text/binary/pong from client
                    _ => {}
                }
            }
            event = rx.recv() => {
                match event {
                    Ok(ws_event) => {
                        let json_str = match serde_json::to_string(&ws_event) {
                            Ok(s) => s,
                            Err(e) => {
                                warn!("Failed to serialize WsEvent: {}", e);
                                continue;
                            }
                        };
                        if socket.send(Message::Text(json_str.into())).await.is_err() {
                            debug!("WebSocket send failed, client likely disconnected");
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(
                            "WebSocket client lagged behind broadcast (skipped {} messages)",
                            skipped
                        );
                        // Resubscribe to reset the lag counter
                        rx = state.ws_tx.subscribe();
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        debug!("Broadcast channel closed");
                        break;
                    }
                }
            }
        }
    }

    state.ws_disconnected();
    info!(
        "WebSocket client disconnected (total: {})",
        state.get_active_ws()
    );
}
