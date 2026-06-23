use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::error::ApiError;
use crate::state::AppState;

/// `GET /api/config` -- return all configuration as a flat key-value map.
pub async fn get_all(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let config = state.config_repo.get_all().await?;
    Ok(Json(json!(config)))
}

/// `PUT /api/config` -- batch update configuration keys. The body is a
/// flat JSON object of key-value pairs. Each key is upserted individually.
/// Auth config changes are also synced to in-memory state.
pub async fn update_all(
    State(state): State<AppState>,
    Json(body): Json<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    state.config_repo.update_all(&body).await?;
    // Sync auth-related config to in-memory state
    sync_auth_config(&state, &body).await;
    Ok(Json(json!({ "updated": true })))
}

/// `GET /api/config/{key}` -- return a single config value wrapped in
/// `{"key":"...","value":"..."}`. Returns 404 if the key is not found.
pub async fn get_one(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let value = state.config_repo.get(&key).await?;
    match value {
        Some(v) => Ok(Json(json!({ "key": key, "value": v }))),
        None => Err(ApiError::NotFound(format!(
            "Config key '{}' not found",
            key
        ))),
    }
}

/// `PUT /api/config/{key}` -- set a single config value. The body must
/// contain `{"value": "..."}`. Returns 400 if the `value` field is missing.
/// Auth config changes are also synced to in-memory state.
pub async fn set_one(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let value = body["value"]
        .as_str()
        .ok_or_else(|| ApiError::BadRequest("Missing 'value' field".into()))?;
    state.config_repo.set(&key, value).await?;

    // Sync auth-related config to in-memory state
    let mut updates = HashMap::new();
    updates.insert(key.clone(), value.to_string());
    sync_auth_config(&state, &updates).await;

    Ok(Json(json!({ "updated": true })))
}

/// If an update contains `auth_enabled` or `admin_token`, reflect the change
/// in the in-memory `AppState` so the auth middleware picks it up immediately.
async fn sync_auth_config(state: &AppState, updates: &HashMap<String, String>) {
    if let Some(v) = updates.get("auth_enabled") {
        state.set_auth_enabled(v == "true");
    }
    if let Some(v) = updates.get("admin_token") {
        state.set_admin_token(v).await;
    }
}
