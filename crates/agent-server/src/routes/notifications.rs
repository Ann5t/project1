//! Notification API endpoints.
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | `POST` | `/api/notifications/test-email` | Send a test email to verify SMTP config |

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::notifications::email::GLOBAL_EMAIL_NOTIFIER;
use crate::state::AppState;

/// `POST /api/notifications/test-email`
///
/// Sends a test email to the configured recipients.  Returns 503 if the
/// email notifier is not configured, or 200 with the SMTP host/from info
/// on success.
pub async fn test_email(State(_state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let notifier = GLOBAL_EMAIL_NOTIFIER
        .get()
        .ok_or_else(|| {
            ApiError::BadRequest(
                "SMTP email notifier is not configured. \
                 Set smtp_enabled=true and provide smtp_username, smtp_password, \
                 smtp_from, and smtp_to in config.".into(),
            )
        })?;

    let subject = "AI Agent — Test Email";
    let body = format!(
        r#"<!DOCTYPE html>
<html>
<body style="font-family:Arial,sans-serif;padding:20px;max-width:600px">
  <h2 style="color:#2a7d2a">Test Email</h2>
  <p>This is a test email from your AI Agent server.</p>
  <p>If you received this, your SMTP configuration is working correctly.</p>
  <table style="border-collapse:collapse">
    <tr><td style="padding:4px 12px 4px 0"><strong>Server time</strong></td>
        <td>{timestamp}</td></tr>
  </table>
  <hr>
  <p style="color:#888;font-size:12px">
    Sent by AI Agent
  </p>
</body>
</html>"#,
        timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
    );

    notifier
        .send_notification(subject, &body)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(json!({
        "success": true,
        "message": "Test email sent. Check your inbox."
    })))
}
