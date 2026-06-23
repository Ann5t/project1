//! SMTP email notification sender built on the `lettre` crate.
//!
//! [`EmailNotifier`] reads SMTP configuration from database config keys
//! (see below) and exposes typed notification helpers for workflows, tasks,
//! and errors.  A global singleton ([`GLOBAL_EMAIL_NOTIFIER`]) is populated
//! at startup so that error-reporting paths (which do not have access to
//! `AppState`) can also fire notifications.
//!
//! # Config keys (all stored in the `config` DB table)
//!
//! | Key | Default | Description |
//! |-----|---------|-------------|
//! | `smtp_enabled` | `false` | Master switch for email notifications |
//! | `smtp_host` | `smtp.gmail.com` | SMTP server hostname |
//! | `smtp_port` | `587` | SMTP port (587 = STARTTLS) |
//! | `smtp_username` | — | SMTP authentication username |
//! | `smtp_password` | — | SMTP authentication password |
//! | `smtp_from` | — | Sender address (e.g. `"Agent <agent@example.com>"`) |
//! | `smtp_to` | — | Comma-separated recipient addresses |
//! | `notify_on_workflow_complete` | `true` | Send email on workflow completion |
//! | `notify_on_task_complete` | `true` | Send email on task execution |
//! | `notify_on_error` | `true` | Send email on server error |

use std::sync::Arc;
use std::sync::OnceLock;

use anyhow::Context;
use lettre::message::{Mailbox, Message, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use tracing::{info, warn};

use agent_db::repo::ConfigRepo;

/// Global email notifier singleton initialized at startup when SMTP is
/// configured.  Used by error-reporting paths (`record_error`) that do not
/// have access to `AppState`.
pub static GLOBAL_EMAIL_NOTIFIER: OnceLock<Arc<EmailNotifier>> = OnceLock::new();

/// Sends email notifications for workflows, tasks, and errors via SMTP.
///
/// The transport is created once at startup and is cheap to clone (the
/// underlying configuration is `Arc`-wrapped).  Notification preferences
/// (`notify_on_*`) are read from the database on every call so that runtime
/// config changes take effect without a restart.
#[derive(Clone)]
pub struct EmailNotifier {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: String,
    config_repo: ConfigRepo,
}

impl EmailNotifier {
    /// Build an `EmailNotifier` from database configuration.
    ///
    /// Returns `None` when:
    /// - `smtp_enabled` is absent or `false`
    /// - `smtp_username`, `smtp_password`, or `smtp_from` is missing/empty
    /// - The SMTP transport fails to build (e.g. invalid hostname)
    pub async fn from_config(config_repo: &ConfigRepo) -> Option<Self> {
        let enabled = config_repo
            .get("smtp_enabled")
            .await
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false);

        if !enabled {
            info!("SMTP email notifications are disabled (smtp_enabled=false)");
            return None;
        }

        let host = config_repo
            .get("smtp_host")
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "smtp.gmail.com".to_string());

        let port: u16 = config_repo
            .get("smtp_port")
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(587);

        let username = config_repo
            .get("smtp_username")
            .await
            .ok()
            .flatten()
            .unwrap_or_default();

        let password = config_repo
            .get("smtp_password")
            .await
            .ok()
            .flatten()
            .unwrap_or_default();

        let from = config_repo
            .get("smtp_from")
            .await
            .ok()
            .flatten()
            .unwrap_or_default();

        if username.is_empty() || password.is_empty() || from.is_empty() {
            warn!(
                "SMTP enabled but credentials incomplete \
                 (username/password/from required); notifications disabled"
            );
            return None;
        }

        let creds = Credentials::new(username, password);

        let transport = match AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host) {
            Ok(builder) => builder.credentials(creds).port(port).build(),
            Err(e) => {
                warn!(
                    "Failed to build SMTP transport for {}:{} — {}",
                    host, port, e
                );
                return None;
            }
        };

        info!(
            "Email notifier ready: host={}, port={}, from={}",
            host, port, from
        );

        Some(Self {
            transport,
            from,
            config_repo: config_repo.clone(),
        })
    }

    // ── Core send ──────────────────────────────────────────────────────

    /// Send an HTML notification email to all addresses in `smtp_to`.
    ///
    /// If `smtp_to` is empty the method silently succeeds (no-op).
    pub async fn send_notification(&self, subject: &str, body_html: &str) -> anyhow::Result<()> {
        let to_str = self
            .config_repo
            .get("smtp_to")
            .await
            .ok()
            .flatten()
            .unwrap_or_default();

        if to_str.is_empty() {
            warn!("No smtp_to recipients configured; skipping notification");
            return Ok(());
        }

        let from_mbox: Mailbox = self.from.parse().context("Invalid smtp_from address")?;

        let to_mboxes: Vec<Mailbox> = to_str
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse::<Mailbox>()
                    .with_context(|| format!("Invalid recipient address: {}", s))
            })
            .collect::<Result<Vec<_>, _>>()?;

        if to_mboxes.is_empty() {
            return Ok(());
        }

        // Build a plain-text fallback by stripping HTML tags.
        let plain = strip_html(body_html);

        // Build message: multipart/alternative (plain + HTML).
        let mut builder = Message::builder().from(from_mbox).subject(subject);

        for to in &to_mboxes {
            builder = builder.to(to.clone());
        }

        let email = builder
            .multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(plain))
                    .singlepart(SinglePart::html(body_html.to_string())),
            )
            .context("Failed to build email message")?;

        self.transport
            .send(email)
            .await
            .context("SMTP send failed")?;

        info!(
            "Email sent: subject=\"{}\", recipients={}",
            subject,
            to_mboxes.len()
        );

        Ok(())
    }

    // ── Typed notification helpers ─────────────────────────────────────

    /// Send a workflow-completion notification.
    ///
    /// The email is only sent when `notify_on_workflow_complete` is `true`
    /// in the database config.
    pub async fn notify_workflow_complete(&self, name: &str, status: &str, result_url: &str) {
        if !self.should_notify("notify_on_workflow_complete").await {
            return;
        }

        let status_upper = status.to_uppercase();
        let color = if status == "error" { "red" } else { "green" };

        let subject = format!("Workflow '{}' completed — {}", name, status_upper);

        let body = format!(
            r#"<!DOCTYPE html>
<html>
<body style="font-family:Arial,sans-serif;padding:20px;max-width:600px">
  <h2 style="color:{color}">Workflow Execution Complete</h2>
  <table style="border-collapse:collapse">
    <tr><td style="padding:4px 12px 4px 0"><strong>Workflow</strong></td>
        <td>{name}</td></tr>
    <tr><td style="padding:4px 12px 4px 0"><strong>Status</strong></td>
        <td style="color:{color};font-weight:bold">{status_upper}</td></tr>
  </table>
  <p><a href="{result_url}">View result</a></p>
  <hr>
  <p style="color:#888;font-size:12px">
    Sent by AI Agent at {timestamp}
  </p>
</body>
</html>"#,
            name = name,
            color = color,
            status_upper = status_upper,
            result_url = result_url,
            timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        );

        if let Err(e) = self.send_notification(&subject, &body).await {
            warn!("Failed to send workflow notification: {}", e);
        }
    }

    /// Send a task-completion notification.
    ///
    /// The email is only sent when `notify_on_task_complete` is `true`
    /// in the database config.
    pub async fn notify_task_complete(&self, name: &str, status: &str, output: &str) {
        if !self.should_notify("notify_on_task_complete").await {
            return;
        }

        let status_upper = status.to_uppercase();
        let color = if status == "error" { "red" } else { "green" };

        let preview = if output.len() > 500 {
            format!("{}...", &output[..500])
        } else {
            output.to_string()
        };

        let subject = format!("Task '{}' executed — {}", name, status_upper);

        let body = format!(
            r#"<!DOCTYPE html>
<html>
<body style="font-family:Arial,sans-serif;padding:20px;max-width:600px">
  <h2 style="color:{color}">Task Execution Complete</h2>
  <table style="border-collapse:collapse">
    <tr><td style="padding:4px 12px 4px 0"><strong>Task</strong></td>
        <td>{name}</td></tr>
    <tr><td style="padding:4px 12px 4px 0"><strong>Status</strong></td>
        <td style="color:{color};font-weight:bold">{status_upper}</td></tr>
  </table>
  <h3>Output</h3>
  <pre style="background:#f4f4f4;padding:10px;border-radius:4px;
              white-space:pre-wrap;font-size:13px">{preview}</pre>
  <hr>
  <p style="color:#888;font-size:12px">
    Sent by AI Agent at {timestamp}
  </p>
</body>
</html>"#,
            name = name,
            color = color,
            status_upper = status_upper,
            preview = preview,
            timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        );

        if let Err(e) = self.send_notification(&subject, &body).await {
            warn!("Failed to send task notification: {}", e);
        }
    }

    /// Send an error notification.
    ///
    /// The email is only sent when `notify_on_error` is `true` in the
    /// database config.  This method is designed to be called from
    /// error-recording paths.
    pub async fn notify_error(&self, error_type: &str, message: &str) {
        if !self.should_notify("notify_on_error").await {
            return;
        }

        let subject = format!("AI Agent Error — {}", error_type);

        let body = format!(
            r#"<!DOCTYPE html>
<html>
<body style="font-family:Arial,sans-serif;padding:20px;max-width:600px">
  <h2 style="color:#cc0000">Error Notification</h2>
  <table style="border-collapse:collapse">
    <tr><td style="padding:4px 12px 4px 0"><strong>Type</strong></td>
        <td>{error_type}</td></tr>
    <tr><td style="padding:4px 12px 4px 0"><strong>Time</strong></td>
        <td>{time}</td></tr>
  </table>
  <h3>Details</h3>
  <pre style="background:#fff0f0;padding:10px;border-radius:4px;
              white-space:pre-wrap;font-size:13px;
              border:1px solid #ffcccc">{message}</pre>
  <hr>
  <p style="color:#888;font-size:12px">
    Sent by AI Agent at {timestamp}
  </p>
</body>
</html>"#,
            error_type = error_type,
            time = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            message = message,
            timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        );

        if let Err(e) = self.send_notification(&subject, &body).await {
            warn!("Failed to send error notification: {}", e);
        }
    }

    // ── Helpers ────────────────────────────────────────────────────────

    /// Check whether a notification toggle is enabled in the config DB.
    /// Defaults to `true` when the key is absent (per spec).
    async fn should_notify(&self, key: &str) -> bool {
        self.config_repo
            .get(key)
            .await
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(true)
    }
}

// ── Plain-text fallback ──────────────────────────────────────────────

/// Strip basic HTML tags to produce a plain-text fallback for multipart
/// emails.  This is a best-effort conversion — it does not handle every
/// edge case.
fn strip_html(html: &str) -> String {
    // Remove tags, collapse whitespace.
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }

    // Decode common entities.
    let text = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");

    // Collapse runs of whitespace into single spaces.
    let words: Vec<&str> = text.split_whitespace().collect();
    words.join(" ")
}
