//! API error handling and global error logging.
//!
//! [`ApiError`] is the unified error type for all route handlers. It implements
//! [`IntoResponse`] so Axum can convert it to an HTTP response with the
//! appropriate status code and a JSON error body.
//!
//! The [`RECENT_ERRORS`] static holds the last 20 errors for the monitoring
//! dashboard, keyed by timestamp.

use std::sync::LazyLock;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use tokio::sync::RwLock;
use tracing;

/// Global error log for the monitoring dashboard (last 20 errors).
///
/// Errors are recorded by [`record_error`] and read by the
/// `GET /api/monitor` endpoint. The log is capped at 20 entries
/// (oldest evicted first).
pub static RECENT_ERRORS: LazyLock<RwLock<Vec<ErrorEntry>>> =
    LazyLock::new(|| RwLock::new(Vec::new()));

/// A recorded error entry for the monitor dashboard.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorEntry {
    /// ISO 8601 timestamp when the error occurred.
    pub timestamp: String,
    /// Human-readable error message.
    pub message: String,
}

/// Record an error to the global error log (max 20 entries, newest first).
/// Also sends an email notification if the email notifier is configured and
/// `notify_on_error` is enabled.
pub async fn record_error(msg: &str) {
    let entry = ErrorEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        message: msg.to_string(),
    };
    let mut errors = RECENT_ERRORS.write().await;
    errors.insert(0, entry);
    errors.truncate(20);

    // Send email notification via the global notifier singleton.
    if let Some(notifier) = crate::notifications::email::GLOBAL_EMAIL_NOTIFIER.get() {
        let notifier = notifier.clone();
        let msg = msg.to_string();
        tokio::spawn(async move {
            notifier.notify_error("ServerError", &msg).await;
        });
    }
}

/// Unified API error type for all route handlers.
///
/// Variants map to HTTP status codes via [`IntoResponse`]:
///
/// | Variant | Status |
/// |---------|--------|
/// | `Core::SessionNotFound` | 404 |
/// | `Core::WorkflowNotFound` | 404 |
/// | `Core::RateLimited` | 429 |
/// | `Core::InvalidConfig` + `BadRequest` | 400 |
/// | `NotFound` | 404 |
/// | All others (Db, Internal, Core catch-all) | 500 |
///
/// 500 errors and DB errors are automatically recorded in [`RECENT_ERRORS`].
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    Core(#[from] agent_core::CoreError),

    #[error("{0}")]
    Db(#[from] agent_db::error::DbError),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    Unauthorized(String),

    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::Core(e) => {
                use agent_core::CoreError;
                match e {
                    CoreError::SessionNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
                    CoreError::WorkflowNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
                    CoreError::RateLimited { .. } => {
                        (StatusCode::TOO_MANY_REQUESTS, self.to_string())
                    }
                    CoreError::InvalidConfig(_) => (StatusCode::BAD_REQUEST, self.to_string()),
                    _ => {
                        // Record internal server errors in the global error log
                        let msg = self.to_string();
                        tokio::task::spawn(async move {
                            record_error(&msg).await;
                        });
                        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
                    }
                }
            }
            ApiError::Db(ref e) => {
                let msg = self.to_string();
                tracing::error!("Database error: {e}");
                tokio::task::spawn(async move {
                    record_error(&msg).await;
                });
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::Internal(_) => {
                let msg = self.to_string();
                tokio::task::spawn(async move {
                    record_error(&msg).await;
                });
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".into())
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
