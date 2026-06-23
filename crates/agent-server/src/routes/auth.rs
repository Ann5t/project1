//! Authentication endpoints.
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | `GET` | `/api/auth/status` | Check whether auth is configured and enabled |
//! | `POST` | `/api/auth/login` | Validate a token and return session info |

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::state::AppState;

/// `GET /api/auth/status` -- public; returns auth configuration status.
///
/// No authentication required.  The response tells the frontend whether
/// token-based auth is active and whether an admin token has been set.
pub async fn auth_status(
    State(state): State<AppState>,
) -> Json<Value> {
    let token_set = !state.get_admin_token().await.is_empty();
    Json(json!({
        "auth_enabled": state.is_auth_enabled(),
        "configured": token_set,
    }))
}

/// Login request body.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub token: String,
}

/// `POST /api/auth/login` -- public; validate a token.
///
/// Accepts `{"token": "<candidate>"}`.  Returns `{"valid": true, "token": "..."}`
/// on success or 401 on a mismatch.
pub async fn auth_login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<Value>, ApiError> {
    if body.token.is_empty() {
        return Err(ApiError::BadRequest("Missing 'token' field".into()));
    }

    let admin_token = state.get_admin_token().await;
    if body.token == admin_token {
        Ok(Json(json!({
            "valid": true,
            "token": body.token,
            "message": "Login successful",
        })))
    } else {
        Err(ApiError::Unauthorized("Invalid token".into()))
    }
}
