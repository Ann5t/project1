use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub name: String,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: i64,
}

fn default_model() -> String {
    "deepseek-chat".into()
}
fn default_temperature() -> f64 {
    0.7
}
fn default_max_tokens() -> i64 {
    4096
}

#[derive(Debug, Deserialize)]
pub struct UpdateSessionRequest {
    pub name: Option<String>,
    pub agent_id: Option<String>,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
}

/// `GET /api/sessions` -- list all sessions ordered by most recently updated.
pub async fn list(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let sessions = state.session_repo.list().await?;
    Ok(Json(json!(sessions)))
}

/// `POST /api/sessions` -- create a new conversation session. Requires at
/// least a `name`. Supports optional `system_prompt`, `model`, `temperature`,
/// and `max_tokens` fields. Defaults: model=`"deepseek-chat"`, temp=0.7,
/// max_tokens=4096, channel=`"web"`.
pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<Json<Value>, ApiError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // Capture fields for broadcast before moving them into SessionRow
    let session_name = body.name.clone();
    let session_model = body.model.clone();

    let session = agent_db::models::SessionRow {
        id: id.clone(),
        name: body.name,
        agent_id: body.agent_id,
        system_prompt: body.system_prompt,
        model: body.model,
        temperature: body.temperature,
        max_tokens: body.max_tokens,
        channel: "web".into(),
        channel_chat_id: None,
        created_at: now.clone(),
        updated_at: now,
    };

    state.session_repo.create(&session).await?;

    // Broadcast real-time event
    state.broadcast_event(
        "session_created",
        json!({
            "session_id": id,
            "name": session_name,
            "model": session_model,
            "channel": "web",
        }),
    );

    Ok(Json(json!(session)))
}

/// `GET /api/sessions/{id}` -- return a single session by ID. Returns 404
/// if not found.
pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let session = state.session_repo.get(&id).await?;
    match session {
        Some(s) => Ok(Json(json!(s))),
        None => Err(ApiError::NotFound(format!("Session '{}' not found", id))),
    }
}

/// `PUT /api/sessions/{id}` -- update session fields. All fields optional.
/// Returns 404 if the session ID is not found.
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateSessionRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut session = state
        .session_repo
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Session '{}' not found", id)))?;

    if let Some(name) = body.name {
        session.name = name;
    }
    if let Some(agent_id) = body.agent_id {
        session.agent_id = Some(agent_id);
    }
    if let Some(sp) = body.system_prompt {
        session.system_prompt = Some(sp);
    }
    if let Some(model) = body.model {
        session.model = model;
    }
    if let Some(temp) = body.temperature {
        session.temperature = temp;
    }
    if let Some(mt) = body.max_tokens {
        session.max_tokens = mt;
    }

    state.session_repo.update(&session).await?;
    Ok(Json(json!(session)))
}

/// `DELETE /api/sessions/{id}` -- delete a session. All messages belonging
/// to this session are cascade-deleted.
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state.session_repo.delete(&id).await?;
    Ok(Json(json!({ "deleted": true })))
}

/// `GET /api/sessions/{id}/messages` -- return all messages for a session
/// ordered by creation time.
pub async fn messages(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let msgs = state.message_repo.list_by_session(&id).await?;
    Ok(Json(json!(msgs)))
}
