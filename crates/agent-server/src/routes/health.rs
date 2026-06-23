use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::state::AppState;

/// `GET /api/health` -- health check, no database dependency.
///
/// Always returns 200 with server version and current UTC timestamp.
pub async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// `GET /api/info` -- returns server version, status, counts (sessions,
/// channels, workflows, tasks, tools), supported channels, and feature flags.
pub async fn system_info(State(state): State<AppState>) -> Json<Value> {
    let sessions_count = state
        .session_repo
        .list()
        .await
        .map(|s| s.len())
        .unwrap_or(0);

    let channels_count = state
        .channel_repo
        .list()
        .await
        .map(|c| c.iter().filter(|ch| ch.enabled).count())
        .unwrap_or(0);

    let workflows_count = state
        .workflow_repo
        .list()
        .await
        .map(|w| w.len())
        .unwrap_or(0);

    let tasks_count = state
        .task_repo
        .list()
        .await
        .map(|t| t.iter().filter(|tk| tk.enabled).count())
        .unwrap_or(0);

    let tools = state.tools.list_names().await;

    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "status": "running",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "stats": {
            "sessions": sessions_count,
            "active_channels": channels_count,
            "workflows": workflows_count,
            "enabled_tasks": tasks_count,
            "tools": tools.len()
        },
        "tools": tools,
        "channels": ["feishu", "qq", "wechat_work", "webhook"],
        "features": {
            "chat": true,
            "streaming": true,
            "workflows": true,
            "scheduler": true,
            "publishing": true,
            "onboarding": true
        }
    }))
}
