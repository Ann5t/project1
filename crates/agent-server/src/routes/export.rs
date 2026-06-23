//! Data export endpoints for sessions and workflows.
//!
//! - `GET  /api/export/session/:id?format=json|markdown|html` — Export a full conversation session
//! - `GET  /api/export/workflow/:id/runs?format=json|csv`     — Export all workflow execution history
//! - `POST /api/export/bulk`                                   — Export multiple sessions as a zip file
//!
//! JSON returns raw structured data. Markdown returns formatted conversation with
//! roles and timestamps. HTML returns a styled dark-themed page similar to publish
//! pages but with the full conversation rendered via comrak.

use std::io::{Cursor, Write};

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use comrak::ComrakOptions;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::state::AppState;

// ── Query / body types ──────────────────────────────────────────────────

/// `format` query parameter for single-session and workflow exports.
#[derive(Debug, Deserialize)]
pub struct ExportFormat {
    #[serde(default = "default_export_format")]
    pub format: String,
}

fn default_export_format() -> String {
    "json".into()
}

/// Body for `POST /api/export/bulk`.
#[derive(Debug, Deserialize)]
pub struct BulkExportRequest {
    pub session_ids: Vec<String>,
    #[serde(default = "default_export_format")]
    pub format: String,
}

/// `format` query parameter for workflow-run exports (json or csv).
#[derive(Debug, Deserialize)]
pub struct WorkflowExportFormat {
    #[serde(default = "default_workflow_export_format")]
    pub format: String,
}

fn default_workflow_export_format() -> String {
    "json".into()
}

// ── Route: Export session ───────────────────────────────────────────────

/// `GET /api/export/session/:id`
///
/// Export a full conversation session in one of three formats:
///
/// | Format     | Content-Type            | Description |
/// |------------|-------------------------|-------------|
/// | `json`     | `application/json`      | Raw message array with session metadata |
/// | `markdown` | `text/markdown`         | Formatted conversation with roles and timestamps |
/// | `html`     | `text/html`             | Styled page similar to publish pages but with full conversation |
pub async fn export_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<ExportFormat>,
) -> Result<Response, ApiError> {
    let session = state
        .session_repo
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Session '{}' not found", id)))?;

    let messages = state.message_repo.list_by_session(&id).await?;

    match query.format.as_str() {
        "json" => {
            let exported_at = chrono::Utc::now().to_rfc3339();
            let body = json!({
                "exported_at": exported_at,
                "session": session,
                "messages": messages,
                "message_count": messages.len(),
            });
            Ok((StatusCode::OK, Json(body)).into_response())
        }
        "markdown" => {
            let md = render_session_markdown(&session, &messages);
            let headers = [
                (header::CONTENT_TYPE, HeaderValue::from_static("text/markdown; charset=utf-8")),
            ];
            Ok((headers, Body::from(md)).into_response())
        }
        "html" => {
            let html = render_session_html(&session, &messages);
            let headers = [
                (header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8")),
            ];
            Ok((headers, Body::from(html)).into_response())
        }
        _ => Err(ApiError::BadRequest(format!(
            "Unsupported format '{}'. Use json, markdown, or html.",
            query.format
        ))),
    }
}

// ── Route: Export workflow runs ──────────────────────────────────────────

/// `GET /api/export/workflow/:id/runs`
///
/// Export all workflow execution history as JSON or CSV.
///
/// | Format | Content-Type       | Description |
/// |--------|--------------------|-------------|
/// | `json` | `application/json` | Array of run objects with metadata |
/// | `csv`  | `text/csv`         | Tabular CSV with all run fields |
pub async fn export_workflow_runs(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<WorkflowExportFormat>,
) -> Result<Response, ApiError> {
    // Verify the workflow exists before listing runs.
    let wf = state
        .workflow_repo
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Workflow '{}' not found", id)))?;

    // Fetch all runs (use a large limit to get everything).
    let runs = state.workflow_repo.list_runs(&id, 100_000).await?;

    match query.format.as_str() {
        "json" => {
            let exported_at = chrono::Utc::now().to_rfc3339();
            let body = json!({
                "exported_at": exported_at,
                "workflow_id": id,
                "workflow_name": wf.name,
                "runs": runs,
                "run_count": runs.len(),
            });
            Ok((StatusCode::OK, Json(body)).into_response())
        }
        "csv" => {
            let csv = render_runs_csv(&runs);
            let headers = [
                (header::CONTENT_TYPE, HeaderValue::from_static("text/csv; charset=utf-8")),
            ];
            Ok((headers, Body::from(csv)).into_response())
        }
        _ => Err(ApiError::BadRequest(format!(
            "Unsupported format '{}'. Use json or csv.",
            query.format
        ))),
    }
}

// ── Route: Bulk export ───────────────────────────────────────────────────

/// `POST /api/export/bulk`
///
/// Export multiple sessions at once, returned as a zip file containing one
/// file per session. The filename inside the archive is the session ID with
/// the appropriate extension.
///
/// Body: `{"session_ids": ["id1", "id2"], "format": "json"}`
///
/// Limited to 50 sessions per request to prevent abuse. Each entry in the zip
/// uses deflate compression.
pub async fn export_bulk(
    State(state): State<AppState>,
    Json(body): Json<BulkExportRequest>,
) -> Result<Response, ApiError> {
    if body.session_ids.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one session_id is required".into(),
        ));
    }

    if body.session_ids.len() > 50 {
        return Err(ApiError::BadRequest(
            "Maximum 50 sessions per bulk export".into(),
        ));
    }

    // Validate format early.
    let ext = match body.format.as_str() {
        "json" => "json",
        "markdown" => "md",
        "html" => "html",
        f => {
            return Err(ApiError::BadRequest(format!(
                "Unsupported format '{}'. Use json, markdown, or html.",
                f
            )))
        }
    };

    // Collect all session data first so errors are surfaced before we start
    // building the zip.
    struct SessionExport {
        id: String,
        content: String,
    }

    let mut exports: Vec<SessionExport> = Vec::with_capacity(body.session_ids.len());

    for sid in &body.session_ids {
        let session = state
            .session_repo
            .get(sid)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("Session '{}' not found", sid)))?;

        let messages = state.message_repo.list_by_session(sid).await?;

        let content = match body.format.as_str() {
            "json" => {
                let exported_at = chrono::Utc::now().to_rfc3339();
                serde_json::to_string_pretty(&json!({
                    "exported_at": exported_at,
                    "session": session,
                    "messages": messages,
                    "message_count": messages.len(),
                }))
                .map_err(|e| {
                    ApiError::Internal(anyhow::anyhow!("JSON serialization failed: {}", e))
                })?
            }
            "markdown" => render_session_markdown(&session, &messages),
            "html" => render_session_html(&session, &messages),
            _ => unreachable!(),
        };

        exports.push(SessionExport {
            id: sid.clone(),
            content,
        });
    }

    // Build the zip archive in memory.
    let mut buf: Vec<u8> = Vec::new();
    {
        let cursor = Cursor::new(&mut buf);
        let mut zip_writer = zip::ZipWriter::new(cursor);

        let options = zip::write::FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for export in &exports {
            let filename = format!("{}.{}", export.id, ext);
            zip_writer
                .start_file(&filename, options)
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("Zip error: {}", e)))?;
            zip_writer
                .write_all(export.content.as_bytes())
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("Zip write error: {}", e)))?;
        }

        zip_writer
            .finish()
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Zip finalize error: {}", e)))?;
    } // cursor (and its mutable borrow of buf) is dropped here.

    let disposition = format!(
        "attachment; filename=\"sessions-export-{}.zip\"",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let headers = [
        (header::CONTENT_TYPE, HeaderValue::from_static("application/zip")),
        (
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&disposition).unwrap_or(HeaderValue::from_static("attachment")),
        ),
    ];

    Ok((headers, Body::from(buf)).into_response())
}

// ── Markdown rendering ───────────────────────────────────────────────────

/// Build a Markdown string from a session and its messages.
fn render_session_markdown(
    session: &agent_db::models::SessionRow,
    messages: &[agent_db::models::MessageRow],
) -> String {
    let mut md = String::new();

    // Header with session metadata
    md.push_str(&format!("# Session: {}\n\n", session.name));
    md.push_str(&format!("- **ID:** `{}`\n", session.id));
    if let Some(ref agent_id) = session.agent_id {
        md.push_str(&format!("- **Agent:** {}\n", agent_id));
    }
    md.push_str(&format!("- **Model:** {}\n", session.model));
    md.push_str(&format!("- **Temperature:** {}\n", session.temperature));
    md.push_str(&format!("- **Max Tokens:** {}\n", session.max_tokens));
    if let Some(ref sp) = session.system_prompt {
        md.push_str(&format!(
            "- **System Prompt:** {}\n",
            sp.replace('\n', " ")
        ));
    }
    md.push_str(&format!("- **Created:** {}\n", session.created_at));
    md.push_str(&format!("- **Updated:** {}\n", session.updated_at));
    md.push_str(&format!("- **Messages:** {}\n", messages.len()));
    md.push_str("\n---\n\n");

    // Conversation body
    for msg in messages {
        let role_badge = match msg.role.as_str() {
            "user" => "### User",
            "assistant" => "### Assistant",
            "system" => "### System",
            "tool" => "### Tool",
            other => &format!("### {}", other),
        };
        md.push_str(&format!(
            "{} <small>({})</small>\n\n",
            role_badge, msg.created_at
        ));

        md.push_str(&msg.content);
        md.push_str("\n\n");

        // Include tool call details if present.
        if let Some(ref tc) = msg.tool_calls {
            md.push_str("<details>\n<summary>Tool Calls</summary>\n\n```json\n");
            // Try to pretty-print the JSON for readability.
            if let Ok(v) = serde_json::from_str::<Value>(tc) {
                if let Ok(pretty) = serde_json::to_string_pretty(&v) {
                    md.push_str(&pretty);
                } else {
                    md.push_str(tc);
                }
            } else {
                md.push_str(tc);
            }
            md.push_str("\n```\n</details>\n\n");
        }

        md.push_str("---\n\n");
    }

    md
}

// ── HTML rendering ───────────────────────────────────────────────────────

/// Render a session as a self-contained HTML page styled like the publish pages.
fn render_session_html(
    session: &agent_db::models::SessionRow,
    messages: &[agent_db::models::MessageRow],
) -> String {
    // Convert the Markdown conversation to HTML via comrak.
    let md_text = render_session_markdown(session, messages);
    let body_html = comrak::markdown_to_html(&md_text, &ComrakOptions::default());

    // Wrap in dark-themed page shell.
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n");
    html.push_str("<meta charset=\"UTF-8\">\n");
    html.push_str(
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n",
    );
    html.push_str(&format!(
        "<title>{} — Exported Session — AI Agent</title>\n",
        escape_html(&session.name)
    ));
    html.push_str(
        "<link rel=\"preconnect\" href=\"https://fonts.googleapis.com\">\n",
    );
    html.push_str(
        "<link href=\"https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap\" rel=\"stylesheet\">\n",
    );
    html.push_str("<style>\n");
    html.push_str(EXPORT_PAGE_CSS);
    html.push_str("</style>\n</head>\n<body>\n");
    html.push_str("<div class=\"container\">\n");

    // Header block
    html.push_str("<div class=\"header\">\n");
    html.push_str(
        "<div class=\"logo\">AI Agent — Exported Session</div>\n",
    );
    html.push_str("<div class=\"meta\">\n");
    html.push_str(&format!(
        "<span>Session: {}</span>\n",
        escape_html(&session.name)
    ));
    html.push_str(&format!(
        "<span>Model: {}</span>\n",
        escape_html(&session.model)
    ));
    html.push_str(&format!(
        "<span>{} messages</span>\n",
        messages.len()
    ));
    html.push_str(
        "<span style=\"background:rgba(34,197,94,0.15);color:#22c55e;padding:4px 12px;border-radius:20px;font-size:0.8rem;font-weight:600;\">Exported</span>\n",
    );
    html.push_str("</div>\n</div>\n");

    // Content
    html.push_str("<div class=\"content\">\n");
    html.push_str(&body_html);
    html.push_str("\n</div>\n");

    // Footer
    html.push_str("<div class=\"footer\">\n");
    html.push_str("Exported from <a href=\"/\">AI Agent</a> &middot; ");
    html.push_str(&format!(
        "Generated {}",
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
    ));
    html.push_str("\n</div>\n");
    html.push_str("</div>\n</body>\n</html>");

    html
}

/// CSS shared between export HTML pages (matches the publish page aesthetic).
const EXPORT_PAGE_CSS: &str = "\
*{margin:0;padding:0;box-sizing:border-box}\
body{font-family:'Inter',-apple-system,sans-serif;background:#0a0a0f;color:#f0f0f5;min-height:100vh;line-height:1.6;-webkit-font-smoothing:antialiased}\
.container{max-width:800px;margin:0 auto;padding:40px 24px}\
.header{text-align:center;margin-bottom:40px;padding-bottom:24px;border-bottom:1px solid rgba(255,255,255,.06)}\
.logo{font-size:1.4rem;font-weight:700;background:linear-gradient(135deg,#8b5cf6,#3b82f6);-webkit-background-clip:text;-webkit-text-fill-color:transparent;margin-bottom:16px}\
.meta{display:flex;justify-content:center;gap:16px;font-size:.8rem;color:#606078;margin-top:12px;flex-wrap:wrap}\
.content{background:#111118;border:1px solid rgba(255,255,255,.06);border-radius:16px;padding:32px;font-size:.95rem;line-height:1.8}\
.content h1,.content h2,.content h3{color:#f0f0f5;margin-top:24px;margin-bottom:12px}\
.content h1{font-size:1.6rem}\
.content h2{font-size:1.3rem}\
.content h3{font-size:1.1rem}\
.content p{margin-bottom:12px}\
.content pre{background:#0a0a0f;border:1px solid rgba(255,255,255,.08);border-radius:10px;padding:16px 20px;overflow-x:auto;font-size:.85rem;line-height:1.5;margin:16px 0}\
.content code{font-family:'JetBrains Mono','Fira Code',monospace;font-size:.85em}\
.content a{color:#8b5cf6}\
.content ul,.content ol{margin:12px 0;padding-left:24px}\
.content li{margin-bottom:6px}\
.content blockquote{border-left:3px solid #8b5cf6;padding-left:16px;color:#a0a0b8;margin:16px 0}\
.content hr{border:none;border-top:1px solid rgba(255,255,255,.06);margin:24px 0}\
.content table{border-collapse:collapse;width:100%;margin:16px 0}\
.content th,.content td{border:1px solid rgba(255,255,255,.1);padding:10px 14px;text-align:left}\
.content th{background:rgba(139,92,246,.1);font-weight:600}\
.content img{max-width:100%;border-radius:8px}\
.content details{margin:12px 0}\
.content summary{cursor:pointer;color:#8b5cf6;font-weight:500}\
.content small{color:#606078;font-size:.8em}\
.footer{text-align:center;margin-top:40px;padding-top:20px;border-top:1px solid rgba(255,255,255,.06);color:#606078;font-size:.8rem}\
.footer a{color:#8b5cf6;text-decoration:none}\
@media(max-width:640px){.container{padding:20px 16px}.content{padding:20px}}\
";

// ── CSV rendering ────────────────────────────────────────────────────────

/// Build a CSV string from a slice of workflow run rows.
fn render_runs_csv(runs: &[agent_db::models::WorkflowRunRow]) -> String {
    let mut csv = String::from(
        "id,workflow_id,status,started_at,finished_at,result,publish_url\n",
    );
    for run in runs {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            csv_quote(&run.id),
            csv_quote(&run.workflow_id),
            csv_quote(&run.status),
            csv_quote(&run.started_at),
            csv_quote(run.finished_at.as_deref().unwrap_or("")),
            csv_quote(run.result.as_deref().unwrap_or("")),
            csv_quote(run.publish_url.as_deref().unwrap_or("")),
        ));
    }
    csv
}

/// Quote a CSV field if it contains special characters.
fn csv_quote(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Basic HTML escaping.
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
