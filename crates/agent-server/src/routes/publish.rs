//! Published result pages -- standalone HTML with dark theme, TOC sidebar,
//! client-side search, syntax highlighting, and a "Download as Markdown" button.
//!
//! - `GET /p/{publish_id}` -- serve a published workflow result
//! - `DELETE /api/publish/{id}` -- delete a published page (stub)

use axum::extract::{Path, State};
use axum::response::Html;
use axum::Json;
use base64::Engine as _;
use comrak::ComrakOptions;
use serde_json::{json, Value};
use tracing::info;

use crate::error::ApiError;
use crate::state::AppState;

/// `GET /p/{publish_id}` -- serve a published workflow result as a rendered
/// HTML page with dark theme and full Markdown formatting.  Returns a 404-style
/// page if the publish ID is not found, but always with HTTP 200.
pub async fn get_published(
    State(state): State<AppState>,
    Path(publish_id): Path<String>,
) -> Result<Html<String>, ApiError> {
    // Look up the workflow run that generated this publish
    let run = state.workflow_repo.get_run(&publish_id).await?;

    let run = match run {
        Some(r) if r.publish_url.is_some() => r,
        _ => {
            return Ok(Html(render_not_found(&publish_id)));
        }
    };

    let result_content = run.result.unwrap_or_else(|| "No content".into());
    let status = &run.status;
    let started = &run.started_at;
    let finished = run.finished_at.as_deref().unwrap_or("N/A");

    info!("Serving published page: {}", publish_id);

    Ok(Html(render_publish_page(
        &publish_id,
        status,
        started,
        finished,
        &result_content,
    )))
}

/// `DELETE /api/publish/{id}` -- delete a published page. Currently a
/// stub that always returns success. Full implementation pending.
pub async fn delete_published(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({ "deleted": true, "id": id })))
}

// ── HTML Templates ─────────────────────────────────────────────────────────

fn render_publish_page(
    id: &str,
    status: &str,
    started: &str,
    finished: &str,
    content: &str,
) -> String {
    let status_html = match status {
        "success" => "<span class=\"badge badge-success\">Success</span>",
        "error" => "<span class=\"badge badge-error\">Error</span>",
        _ => "<span class=\"badge badge-info\">Running</span>",
    };

    let rendered_content = render_markdown_to_html(content);
    let reading_time = estimate_reading_time(content);

    // Base64-encode the raw markdown for the "Download" button
    let raw_b64 = base64::engine::general_purpose::STANDARD.encode(content.as_bytes());

    let mut html = String::with_capacity(32768);
    html.push_str("<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n");
    html.push_str("<meta charset=\"UTF-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str("<title>AI Agent — Published Result</title>\n");
    html.push_str("<link rel=\"preconnect\" href=\"https://fonts.googleapis.com\">\n");
    html.push_str("<link rel=\"preconnect\" href=\"https://fonts.gstatic.com\" crossorigin>\n");
    html.push_str("<link href=\"https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap\" rel=\"stylesheet\">\n");
    // highlight.js for syntax highlighting in code blocks
    html.push_str("<link rel=\"stylesheet\" href=\"https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/atom-one-dark.min.css\">\n");
    html.push_str("<style>\n");
    html.push_str(PUBLISH_CSS);
    html.push_str("</style>\n");
    html.push_str("</head>\n<body>\n");

    // ── TOC toggle button (mobile) ──
    html.push_str("<button class=\"toc-toggle\" id=\"toc-toggle\" aria-label=\"Toggle table of contents\" title=\"Table of Contents\">\n");
    html.push_str("<svg width=\"20\" height=\"20\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><line x1=\"3\" y1=\"6\" x2=\"21\" y2=\"6\"/><line x1=\"3\" y1=\"12\" x2=\"21\" y2=\"12\"/><line x1=\"3\" y1=\"18\" x2=\"21\" y2=\"18\"/></svg>\n");
    html.push_str("</button>\n");

    // ── TOC overlay (mobile) ──
    html.push_str("<div class=\"toc-overlay\" id=\"toc-overlay\"></div>\n");

    // ── Main layout ──
    html.push_str("<div class=\"pub-layout\">\n");

    // ── TOC Sidebar ──
    html.push_str("<nav class=\"toc-sidebar\" id=\"toc-sidebar\">\n");
    html.push_str("<div class=\"toc-header\">\n");
    html.push_str("<span class=\"toc-title\">Contents</span>\n");
    html.push_str("<button class=\"toc-close-btn\" id=\"toc-close-btn\" aria-label=\"Close TOC\">&times;</button>\n");
    html.push_str("</div>\n");
    html.push_str("<div class=\"toc-list\" id=\"toc-list\">\n");
    html.push_str("<div class=\"toc-loading\">Loading...</div>\n");
    html.push_str("</div>\n");
    html.push_str("</nav>\n");

    // ── Main content area ──
    html.push_str("<main class=\"pub-main\">\n");

    // ── Top toolbar ──
    html.push_str("<div class=\"pub-toolbar\">\n");
    // Search
    html.push_str("<div class=\"search-box\" id=\"search-box\">\n");
    html.push_str("<svg class=\"search-icon\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"11\" cy=\"11\" r=\"8\"/><line x1=\"21\" y1=\"21\" x2=\"16.65\" y2=\"16.65\"/></svg>\n");
    html.push_str("<input type=\"text\" class=\"search-input\" id=\"search-input\" placeholder=\"Search page... (Ctrl+F)\">\n");
    html.push_str("<span class=\"search-count\" id=\"search-count\"></span>\n");
    html.push_str("<button class=\"search-prev\" id=\"search-prev\" title=\"Previous match\" aria-label=\"Previous match\">&uarr;</button>\n");
    html.push_str("<button class=\"search-next\" id=\"search-next\" title=\"Next match\" aria-label=\"Next match\">&darr;</button>\n");
    html.push_str("<button class=\"search-close\" id=\"search-close\" title=\"Close search\" aria-label=\"Close search\">&times;</button>\n");
    html.push_str("</div>\n");
    // Download button
    html.push_str("<div class=\"pub-toolbar-actions\">\n");
    html.push_str(
        "<button class=\"btn-download\" id=\"btn-download\" title=\"Download as Markdown\">\n",
    );
    html.push_str("<svg width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4\"/><polyline points=\"7 10 12 15 17 10\"/><line x1=\"12\" y1=\"15\" x2=\"12\" y2=\"3\"/></svg>\n");
    html.push_str("<span>Download .md</span>\n");
    html.push_str("</button>\n");
    html.push_str("</div>\n");
    html.push_str("</div>\n");

    // ── Header ──
    html.push_str("<div class=\"header\">\n");
    html.push_str("<div class=\"logo\">AI Agent — Published Result</div>\n");
    html.push_str("<div class=\"meta\">\n");
    html.push_str(&format!("<span>ID: {}</span>\n", escape_html(id)));
    html.push_str(&format!("<span>{}</span>\n", status_html));
    html.push_str(&format!("<span>Started: {}</span>\n", escape_html(started)));
    html.push_str(&format!(
        "<span>Finished: {}</span>\n",
        escape_html(finished)
    ));
    html.push_str(&format!(
        "<span class=\"reading-time\">{} read</span>\n",
        reading_time
    ));
    html.push_str("</div>\n</div>\n");

    // ── Content ──
    html.push_str("<div class=\"content\" id=\"pub-content\">\n");
    html.push_str(&rendered_content);
    html.push_str("\n</div>\n");

    // ── Footer ──
    html.push_str("<div class=\"footer\">\nGenerated by <a href=\"/\">AI Agent</a> · Powered by DeepSeek\n</div>\n");

    html.push_str("</main>\n");
    html.push_str("</div>\n"); // .pub-layout

    // ── Embedded raw markdown (base64) for download ──
    html.push_str(&format!(
        "<script id=\"raw-markdown\" type=\"text/plain\" data-b64=\"{}\"></script>\n",
        raw_b64
    ));

    // ── highlight.js ──
    html.push_str("<script src=\"https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js\"></script>\n");
    html.push_str("<script>\n");
    html.push_str(PUBLISH_JS);
    html.push_str("</script>\n");

    html.push_str("</body>\n</html>");

    html
}

fn render_not_found(id: &str) -> String {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n");
    html.push_str("<meta charset=\"UTF-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str("<title>Page Not Found — AI Agent</title>\n");
    html.push_str("<link rel=\"preconnect\" href=\"https://fonts.googleapis.com\">\n");
    html.push_str("<link href=\"https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap\" rel=\"stylesheet\">\n");
    html.push_str("<style>");
    html.push_str("*{margin:0;padding:0;box-sizing:border-box}body{font-family:'Inter',-apple-system,sans-serif;background:#0a0a0f;color:#f0f0f5;min-height:100vh;display:flex;align-items:center;justify-content:center;-webkit-font-smoothing:antialiased}");
    html.push_str(".error-card{text-align:center;padding:48px}");
    html.push_str(".error-code{font-size:5rem;font-weight:700;background:linear-gradient(135deg,#8b5cf6,#3b82f6);-webkit-background-clip:text;-webkit-text-fill-color:transparent;margin-bottom:16px}");
    html.push_str(".error-msg{color:#a0a0b8;margin-bottom:24px;font-size:1rem}");
    html.push_str(".back-link{color:#8b5cf6;text-decoration:none;font-weight:500}");
    html.push_str("</style>\n</head>\n<body>\n");
    html.push_str("<div class=\"error-card\">\n");
    html.push_str("<div class=\"error-code\">404</div>\n");
    html.push_str(&format!(
        "<div class=\"error-msg\">Published result not found: {}</div>\n",
        escape_html(id)
    ));
    html.push_str("<a href=\"/\" class=\"back-link\">Back to AI Agent</a>\n");
    html.push_str("</div>\n</body>\n</html>");
    html
}

// ── Markdown rendering ─────────────────────────────────────────────────────

/// Render Markdown to HTML using comrak with full CommonMark + extensions.
fn render_markdown_to_html(text: &str) -> String {
    let mut options = ComrakOptions::default();
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.footnotes = true;
    options.extension.autolink = true;
    options.extension.tagfilter = true;

    comrak::markdown_to_html(text, &options)
}

// ── Reading time ───────────────────────────────────────────────────────────

/// Estimate reading time. Uses ~200 wpm for Latin text and ~300 cpm for CJK.
fn estimate_reading_time(text: &str) -> String {
    let latin_words = text
        .split_whitespace()
        .filter(|w| w.chars().any(|c| c.is_alphabetic() && c.is_ascii()))
        .count();
    let cjk_chars = text.chars().filter(|c| is_cjk(*c)).count();

    let minutes = (latin_words as f64 / 200.0) + (cjk_chars as f64 / 300.0);
    let total = (minutes * 60.0).round() as u64;

    if total < 60 {
        format!("{} sec", total.max(1))
    } else if total < 3600 {
        format!("{} min", total / 60)
    } else {
        format!("{}h {}m", total / 3600, (total % 3600) / 60)
    }
}

fn is_cjk(c: char) -> bool {
    matches!(
        c,
        '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK Unified Ideographs Extension A
        | '\u{F900}'..='\u{FAFF}' // CJK Compatibility Ideographs
        | '\u{3040}'..='\u{309F}' // Hiragana
        | '\u{30A0}'..='\u{30FF}' // Katakana
        | '\u{AC00}'..='\u{D7AF}' // Hangul Syllables
    )
}

// ── HTML escape ─────────────────────────────────────────────────────────────

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ── CSS ────────────────────────────────────────────────────────────────────

const PUBLISH_CSS: &str = "\
/* ═══════════════════════════════════════════
   Design tokens (match main app base.css)
   ═══════════════════════════════════════════ */
:root {
  --bg-primary: #0a0a0f;
  --bg-secondary: #111118;
  --bg-tertiary: #1a1a24;
  --bg-elevated: #22222f;
  --bg-glass: rgba(255,255,255,0.03);
  --text-primary: #f0f0f5;
  --text-secondary: #a0a0b8;
  --text-tertiary: #606078;
  --border-color: rgba(255,255,255,0.06);
  --border-highlight: rgba(255,255,255,0.12);
  --accent-primary: #8b5cf6;
  --accent-secondary: #3b82f6;
  --accent-gradient: linear-gradient(135deg, #8b5cf6, #3b82f6);
  --success: #22c55e;
  --warning: #f59e0b;
  --error: #ef4444;
  --font-sans: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
  --font-mono: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
  --toc-width: 250px;
}

/* ── Reset & Base ── */
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
html { font-size: 15px; -webkit-font-smoothing: antialiased; scroll-behavior: smooth; }
body {
  font-family: var(--font-sans);
  background: var(--bg-primary);
  color: var(--text-primary);
  line-height: 1.6;
  min-height: 100vh;
}
a { color: var(--accent-primary); text-decoration: none; }
a:hover { opacity: 0.85; }
::selection { background: var(--accent-primary); color: #fff; }
::-webkit-scrollbar { width: 6px; height: 6px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: var(--border-highlight); border-radius: 3px; }

/* ── Layout ── */
.pub-layout {
  display: flex;
  min-height: 100vh;
}

/* ── TOC Sidebar ── */
.toc-sidebar {
  width: var(--toc-width);
  min-width: var(--toc-width);
  background: var(--bg-secondary);
  border-right: 1px solid var(--border-color);
  height: 100vh;
  position: sticky;
  top: 0;
  display: flex;
  flex-direction: column;
  z-index: 50;
  transition: transform 250ms cubic-bezier(0.4,0,0.2,1);
}
.toc-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 18px 12px;
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}
.toc-title {
  font-size: 0.85rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--text-secondary);
}
.toc-close-btn {
  display: none;
  background: none;
  border: none;
  color: var(--text-tertiary);
  font-size: 1.4rem;
  cursor: pointer;
  padding: 0 4px;
  line-height: 1;
  font-family: var(--font-sans);
}
.toc-close-btn:hover { color: var(--text-primary); }
.toc-list {
  flex: 1;
  overflow-y: auto;
  padding: 12px 0;
}
.toc-loading {
  padding: 12px 18px;
  font-size: 0.82rem;
  color: var(--text-tertiary);
}
.toc-item {
  display: block;
  padding: 6px 18px;
  font-size: 0.82rem;
  color: var(--text-secondary);
  text-decoration: none;
  border-left: 2px solid transparent;
  transition: all 120ms ease-out;
  line-height: 1.4;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.toc-item:hover { background: var(--bg-glass); color: var(--text-primary); }
.toc-item.active { color: var(--accent-primary); border-left-color: var(--accent-primary); background: rgba(139,92,246,0.05); }
.toc-item.toc-h1 { font-weight: 600; font-size: 0.85rem; padding-top: 8px; padding-bottom: 4px; }
.toc-item.toc-h2 { padding-left: 30px; font-size: 0.8rem; }
.toc-item.toc-h3 { padding-left: 42px; font-size: 0.76rem; color: var(--text-tertiary); }

/* ── TOC Toggle Button (mobile) ── */
.toc-toggle {
  display: none;
  position: fixed;
  bottom: 24px;
  right: 24px;
  z-index: 100;
  width: 44px;
  height: 44px;
  border-radius: 50%;
  background: var(--bg-elevated);
  border: 1px solid var(--border-highlight);
  color: var(--text-primary);
  cursor: pointer;
  align-items: center;
  justify-content: center;
  box-shadow: 0 4px 16px rgba(0,0,0,0.4);
  transition: all 150ms ease-out;
}
.toc-toggle:hover { transform: scale(1.08); border-color: var(--accent-primary); }
.toc-toggle:active { transform: scale(0.95); }

/* ── TOC overlay (mobile) ── */
.toc-overlay {
  display: none;
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,0.5);
  backdrop-filter: blur(4px);
  z-index: 60;
}

/* ── Main Content ── */
.pub-main {
  flex: 1;
  min-width: 0;
  max-width: 900px;
  margin: 0 auto;
  padding: 24px 32px 48px;
  width: 100%;
}

/* ── Toolbar ── */
.pub-toolbar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
  margin-bottom: 20px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border-color);
  flex-wrap: wrap;
}
.pub-toolbar-actions {
  display: flex;
  gap: 8px;
}

/* ── Search Box ── */
.search-box {
  display: flex;
  align-items: center;
  gap: 6px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  padding: 6px 10px;
  transition: border-color 150ms ease-out, box-shadow 150ms ease-out;
}
.search-box:focus-within {
  border-color: var(--accent-primary);
  box-shadow: 0 0 0 3px rgba(139,92,246,0.12);
}
.search-box .search-icon { color: var(--text-tertiary); flex-shrink: 0; }
.search-input {
  background: none;
  border: none;
  outline: none;
  color: var(--text-primary);
  font-size: 0.85rem;
  font-family: var(--font-sans);
  width: 180px;
}
.search-input::placeholder { color: var(--text-tertiary); }
.search-count {
  font-size: 0.72rem;
  color: var(--text-tertiary);
  white-space: nowrap;
  min-width: 48px;
  text-align: center;
}
.search-prev, .search-next, .search-close {
  background: none;
  border: none;
  color: var(--text-tertiary);
  cursor: pointer;
  padding: 2px 6px;
  font-size: 0.9rem;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  line-height: 1;
  transition: all 120ms ease-out;
}
.search-prev:hover, .search-next:hover, .search-close:hover {
  color: var(--text-primary);
  background: var(--bg-glass);
}
.search-close { font-size: 1.2rem; margin-left: 2px; }

/* ── Download Button ── */
.btn-download {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 16px;
  font-size: 0.82rem;
  font-weight: 500;
  font-family: var(--font-sans);
  background: var(--bg-tertiary);
  border: 1px solid var(--border-highlight);
  border-radius: 8px;
  color: var(--text-primary);
  cursor: pointer;
  transition: all 150ms ease-out;
  white-space: nowrap;
}
.btn-download:hover {
  background: var(--bg-elevated);
  border-color: var(--accent-primary);
}
.btn-download:active { transform: scale(0.97); }

/* ── Header ── */
.header {
  text-align: center;
  margin-bottom: 36px;
  padding-bottom: 20px;
  border-bottom: 1px solid var(--border-color);
}
.logo {
  font-size: 1.4rem;
  font-weight: 700;
  background: var(--accent-gradient);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  margin-bottom: 14px;
}
.meta {
  display: flex;
  justify-content: center;
  gap: 16px;
  font-size: 0.8rem;
  color: var(--text-tertiary);
  flex-wrap: wrap;
}
.reading-time {
  background: var(--bg-glass);
  padding: 2px 10px;
  border-radius: 12px;
  font-size: 0.75rem;
  color: var(--text-secondary);
}

/* ── Badges ── */
.badge {
  display: inline-flex;
  align-items: center;
  padding: 3px 12px;
  font-size: 0.76rem;
  font-weight: 600;
  border-radius: 20px;
  letter-spacing: 0.02em;
}
.badge-success { background: rgba(34,197,94,0.12); color: var(--success); }
.badge-error { background: rgba(239,68,68,0.12); color: var(--error); }
.badge-info { background: rgba(59,130,246,0.12); color: var(--info); }

/* ── Content ── */
.content {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 16px;
  padding: 36px 40px;
  font-size: 0.95rem;
  line-height: 1.8;
  overflow-wrap: break-word;
}
.content h1, .content h2, .content h3, .content h4, .content h5, .content h6 {
  color: var(--text-primary);
  margin-top: 28px;
  margin-bottom: 12px;
  line-height: 1.3;
  scroll-margin-top: 24px;
}
.content h1 { font-size: 1.7rem; font-weight: 700; border-bottom: 1px solid var(--border-color); padding-bottom: 8px; }
.content h2 { font-size: 1.35rem; font-weight: 600; }
.content h3 { font-size: 1.12rem; font-weight: 600; }
.content h4 { font-size: 1rem; font-weight: 600; }
.content p { margin-bottom: 14px; }
.content p:last-child { margin-bottom: 0; }
.content a { color: var(--accent-primary); }
.content a:hover { text-decoration: underline; }
.content strong { font-weight: 600; color: var(--text-primary); }
.content em { font-style: italic; }
.content del { text-decoration: line-through; opacity: 0.6; }
.content img { max-width: 100%; border-radius: 8px; margin: 12px 0; }
.content hr { border: none; border-top: 1px solid var(--border-color); margin: 28px 0; }
.content blockquote {
  border-left: 3px solid var(--accent-primary);
  padding: 8px 18px;
  margin: 16px 0;
  color: var(--text-secondary);
  background: var(--bg-glass);
  border-radius: 0 8px 8px 0;
}
.content blockquote p { margin-bottom: 6px; }
.content ul, .content ol { margin: 12px 0; padding-left: 26px; }
.content li { margin-bottom: 6px; }
.content li > ul, .content li > ol { margin: 4px 0; }
/* task lists */
.content ul.contains-task-list { list-style: none; padding-left: 4px; }
.content .task-list-item { display: flex; align-items: flex-start; gap: 8px; }
.content .task-list-item input[type=\"checkbox\"] {
  margin-top: 0.35em;
  accent-color: var(--accent-primary);
  flex-shrink: 0;
}

/* ── Tables ── */
.content table {
  border-collapse: collapse;
  width: 100%;
  margin: 18px 0;
  font-size: 0.88rem;
  border-radius: 8px;
  overflow: hidden;
}
.content th, .content td {
  border: 1px solid var(--border-highlight);
  padding: 10px 16px;
  text-align: left;
}
.content th {
  background: rgba(139,92,246,0.08);
  font-weight: 600;
  color: var(--text-primary);
  font-size: 0.82rem;
  text-transform: uppercase;
  letter-spacing: 0.03em;
}
.content tr:nth-child(even) td { background: var(--bg-glass); }

/* ── Footnotes ── */
.content .footnote-definition { font-size: 0.85rem; color: var(--text-secondary); margin: 8px 0; padding: 4px 0; }
.content .footnote-definition sup { margin-right: 4px; }
.content .footnote-definition p { display: inline; }
.content a[href^=\"#fn\"] { font-size: 0.8em; vertical-align: super; }

/* ── Code ── */
.content code {
  font-family: var(--font-mono);
  font-size: 0.85em;
}
.content code:not(pre code) {
  background: var(--bg-glass);
  border: 1px solid var(--border-color);
  border-radius: 5px;
  padding: 2px 7px;
  color: var(--accent-primary);
}

/* ── Code Blocks ── */
.content pre {
  background: #0d0d14;
  border: 1px solid var(--border-highlight);
  border-radius: 10px;
  margin: 18px 0;
  overflow: hidden;
  position: relative;
}
.content pre code {
  display: block;
  padding: 18px 20px;
  overflow-x: auto;
  font-family: var(--font-mono);
  font-size: 0.84rem;
  line-height: 1.6;
  color: #e0e0ec;
  background: none;
  border: none;
  border-radius: 0;
}
/* hljs overrides for dark theme consistency */
.content pre code.hljs { background: transparent; padding: 18px 20px; }

/* ── Copy button on code blocks ── */
.code-block-wrapper { position: relative; }
.copy-btn {
  position: absolute;
  top: 8px;
  right: 8px;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 5px 10px;
  font-size: 0.72rem;
  font-family: var(--font-sans);
  background: var(--bg-elevated);
  color: var(--text-tertiary);
  border: 1px solid var(--border-highlight);
  border-radius: 6px;
  cursor: pointer;
  transition: all 120ms ease-out;
  opacity: 0;
  z-index: 5;
}
.code-block-wrapper:hover .copy-btn { opacity: 1; }
.copy-btn:hover {
  color: var(--text-primary);
  border-color: var(--accent-primary);
  background: var(--bg-tertiary);
}
.copy-btn.copied {
  color: var(--success);
  border-color: rgba(34,197,94,0.3);
}

/* ── Search highlight ── */
mark.search-highlight {
  background: rgba(245,158,11,0.35);
  color: inherit;
  padding: 1px 2px;
  border-radius: 2px;
}
mark.search-highlight.current {
  background: rgba(245,158,11,0.6);
  outline: 2px solid var(--warning);
  outline-offset: 1px;
}

/* ── Footer ── */
.footer {
  text-align: center;
  margin-top: 40px;
  padding-top: 20px;
  border-top: 1px solid var(--border-color);
  color: var(--text-tertiary);
  font-size: 0.8rem;
}
.footer a { color: var(--accent-primary); }

/* ── Responsive ── */
@media (max-width: 860px) {
  .toc-sidebar {
    position: fixed;
    left: 0;
    top: 0;
    height: 100vh;
    transform: translateX(-100%);
    box-shadow: 4px 0 24px rgba(0,0,0,0.5);
    z-index: 70;
  }
  .toc-sidebar.open { transform: translateX(0); }
  .toc-overlay.active { display: block; }
  .toc-toggle { display: flex; }
  .toc-close-btn { display: flex; }
  .pub-main { padding: 20px 16px 48px; }
  .content { padding: 24px 20px; }
  .search-input { width: 120px; }
  .header { margin-bottom: 24px; }
}

@media (max-width: 480px) {
  .pub-main { padding: 16px 12px 40px; }
  .content { padding: 18px 14px; font-size: 0.9rem; }
  .content h1 { font-size: 1.35rem; }
  .content h2 { font-size: 1.15rem; }
  .content h3 { font-size: 1rem; }
  .pub-toolbar { flex-direction: column; align-items: stretch; gap: 8px; }
  .search-input { width: 100%; }
  .search-box { flex: 1; }
  .pub-toolbar-actions { justify-content: flex-end; }
  .meta { gap: 8px; font-size: 0.72rem; }
}

/* ── Print ── */
@media print {
  .toc-sidebar, .toc-toggle, .toc-overlay, .pub-toolbar,
  .copy-btn, .btn-download, .search-box, .footer {
    display: none !important;
  }
  body { background: #fff; color: #000; font-size: 12pt; }
  .pub-layout { display: block; }
  .pub-main { max-width: 100%; padding: 0; }
  .content {
    background: #fff;
    border: none;
    border-radius: 0;
    padding: 0;
    box-shadow: none;
    font-size: 11pt;
    line-height: 1.6;
    color: #000;
  }
  .content h1, .content h2, .content h3, .content h4,
  .content strong { color: #000; }
  .content pre {
    background: #f5f5f5;
    border: 1px solid #ddd;
    page-break-inside: avoid;
  }
  .content pre code { color: #333; }
  .content code { color: #333; background: #f5f5f5; border: 1px solid #ddd; }
  .content code:not(pre code) { padding: 1px 4px; }
  .content a { color: #0066cc; }
  .content blockquote {
    border-left-color: #666;
    color: #444;
    background: #f9f9f9;
  }
  .content th { background: #eee; color: #000; }
  .content th, .content td { border-color: #ccc; }
  .header { border-bottom: 2px solid #000; padding-bottom: 12px; margin-bottom: 24px; }
  .logo { -webkit-text-fill-color: #000; background: none; color: #000; }
  .meta { color: #666; }
  .badge { color: #000; background: #eee; }
  @page { margin: 2cm; }
}
";

// ── JavaScript ──────────────────────────────────────────────────────────────

const PUBLISH_JS: &str = r##"
(function(){
  'use strict';

  // ── TOC Generation ──
  function buildTOC() {
    var content = document.getElementById('pub-content');
    if (!content) return;
    var headings = content.querySelectorAll('h1, h2, h3');
    if (headings.length === 0) {
      document.getElementById('toc-list').innerHTML = '<div class="toc-loading">No headings</div>';
      return;
    }
    var tocList = document.getElementById('toc-list');
    tocList.innerHTML = '';
    var usedSlugs = {};

    headings.forEach(function(h, idx) {
      // Generate slug from text content
      var raw = h.textContent.trim().replace(/\s+/g, ' ');
      var slug = raw
        .toLowerCase()
        .replace(/[^\w一-鿿぀-ゟ゠-ヿ가-힯\s-]/g, '')
        .replace(/\s+/g, '-')
        .replace(/-+/g, '-')
        .replace(/^-|-$/g, '');
      if (!slug) slug = 'heading-' + idx;
      // Deduplicate
      var base = slug;
      var count = 1;
      while (usedSlugs[slug]) {
        slug = base + '-' + (++count);
      }
      usedSlugs[slug] = true;

      // Set heading id
      h.id = slug;

      // Create TOC entry
      var a = document.createElement('a');
      a.href = '#' + slug;
      a.className = 'toc-item toc-' + h.tagName.toLowerCase();
      a.textContent = raw;
      a.addEventListener('click', function(e) {
        e.preventDefault();
        var target = document.getElementById(this.getAttribute('href').slice(1));
        if (target) target.scrollIntoView({ behavior: 'smooth' });
        // Close mobile TOC
        closeTOC();
      });
      tocList.appendChild(a);
    });

    // Scroll-spy using IntersectionObserver
    var tocLinks = tocList.querySelectorAll('.toc-item');
    if (tocLinks.length && 'IntersectionObserver' in window) {
      var observer = new IntersectionObserver(function(entries) {
        entries.forEach(function(entry) {
          var link = tocList.querySelector('a[href="#' + entry.target.id + '"]');
          if (!link) return;
          if (entry.isIntersecting) {
            tocLinks.forEach(function(l) { l.classList.remove('active'); });
            link.classList.add('active');
            // Scroll TOC to keep active item visible
            link.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
          }
        });
      }, { rootMargin: '-10% 0px -70% 0px' });
      headings.forEach(function(h) { observer.observe(h); });
    }
  }

  // ── TOC Toggle (mobile) ──
  var tocSidebar = document.getElementById('toc-sidebar');
  var tocOverlay = document.getElementById('toc-overlay');
  var tocToggle = document.getElementById('toc-toggle');
  var tocCloseBtn = document.getElementById('toc-close-btn');

  function openTOC() {
    tocSidebar.classList.add('open');
    tocOverlay.classList.add('active');
  }
  function closeTOC() {
    tocSidebar.classList.remove('open');
    tocOverlay.classList.remove('active');
  }
  if (tocToggle) tocToggle.addEventListener('click', openTOC);
  if (tocOverlay) tocOverlay.addEventListener('click', closeTOC);
  if (tocCloseBtn) tocCloseBtn.addEventListener('click', closeTOC);

  // ── Copy Buttons on Code Blocks ──
  function addCopyButtons() {
    var pres = document.querySelectorAll('#pub-content pre');
    pres.forEach(function(pre) {
      if (pre.parentNode.classList.contains('code-block-wrapper')) return;
      var wrapper = document.createElement('div');
      wrapper.className = 'code-block-wrapper';
      pre.parentNode.insertBefore(wrapper, pre);
      wrapper.appendChild(pre);

      var btn = document.createElement('button');
      btn.className = 'copy-btn';
      btn.textContent = 'Copy';
      btn.addEventListener('click', function() {
        var code = pre.querySelector('code') || pre;
        var text = code.textContent;
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(text).then(function() {
            btn.textContent = 'Copied!';
            btn.classList.add('copied');
            setTimeout(function() { btn.textContent = 'Copy'; btn.classList.remove('copied'); }, 2000);
          }).catch(function() { fallbackCopy(text, btn); });
        } else {
          fallbackCopy(text, btn);
        }
      });
      wrapper.appendChild(btn);

      function fallbackCopy(text, btn) {
        var ta = document.createElement('textarea');
        ta.value = text;
        ta.style.position = 'fixed';
        ta.style.opacity = '0';
        document.body.appendChild(ta);
        ta.select();
        try { document.execCommand('copy'); btn.textContent = 'Copied!'; btn.classList.add('copied'); }
        catch(e) { btn.textContent = 'Error'; }
        document.body.removeChild(ta);
        setTimeout(function() { btn.textContent = 'Copy'; btn.classList.remove('copied'); }, 2000);
      }
    });
  }

  // ── Syntax Highlighting ──
  function highlightCode() {
    if (typeof hljs === 'undefined') return;
    document.querySelectorAll('#pub-content pre code').forEach(function(block) {
      hljs.highlightElement(block);
    });
  }

  // ── Search ──
  var searchBox = document.getElementById('search-box');
  var searchInput = document.getElementById('search-input');
  var searchCount = document.getElementById('search-count');
  var searchPrev = document.getElementById('search-prev');
  var searchNext = document.getElementById('search-next');
  var searchClose = document.getElementById('search-close');
  var contentEl = document.getElementById('pub-content');
  var highlights = [];
  var currentHighlight = -1;

  function clearHighlights() {
    highlights.forEach(function(m) { m.classList.remove('search-highlight','current'); });
    highlights = [];
    currentHighlight = -1;
    // Restore text nodes
    var marks = contentEl.querySelectorAll('mark.search-highlight');
    marks.forEach(function(m) {
      var parent = m.parentNode;
      var text = document.createTextNode(m.textContent);
      parent.replaceChild(text, m);
    });
    // Normalize after cleanup
    contentEl.normalize();
  }

  function doSearch() {
    clearHighlights();
    var query = searchInput.value.trim();
    if (!query) { searchCount.textContent = ''; return; }

    // Walk text nodes and wrap matches
    var walker = document.createTreeWalker(contentEl, NodeFilter.SHOW_TEXT, null, false);
    var textNodes = [];
    while (walker.nextNode()) { textNodes.push(walker.currentNode); }

    var regex = new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi');
    var count = 0;

    // Process text nodes in reverse to preserve indices
    for (var i = textNodes.length - 1; i >= 0; i--) {
      var node = textNodes[i];
      if (node.parentNode.tagName === 'MARK' && node.parentNode.classList.contains('search-highlight')) continue;
      var text = node.textContent;
      var match;
      var fragments = [];
      var lastIdx = 0;
      regex.lastIndex = 0;

      while ((match = regex.exec(text)) !== null) {
        count++;
        if (match.index > lastIdx) {
          fragments.push({ type: 'text', value: text.slice(lastIdx, match.index) });
        }
        fragments.push({ type: 'mark', value: match[0] });
        lastIdx = regex.lastIndex;
        if (match[0].length === 0) regex.lastIndex++;
      }
      if (lastIdx < text.length) {
        fragments.push({ type: 'text', value: text.slice(lastIdx) });
      }

      if (fragments.length > 1) {
        var parent = node.parentNode;
        fragments.forEach(function(f) {
          if (f.type === 'text') {
            parent.insertBefore(document.createTextNode(f.value), node);
          } else {
            var mark = document.createElement('mark');
            mark.className = 'search-highlight';
            mark.textContent = f.value;
            highlights.push(mark);
            parent.insertBefore(mark, node);
          }
        });
        parent.removeChild(node);
      }
    }

    // Sort highlights by DOM order
    highlights.sort(function(a, b) {
      return (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING) ? -1 : 1;
    });

    searchCount.textContent = count > 0 ? '1 of ' + count : '0 matches';
    if (count > 0) {
      currentHighlight = 0;
      highlights[0].classList.add('current');
      highlights[0].scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  }

  function navigateSearch(delta) {
    if (highlights.length === 0) return;
    highlights[currentHighlight].classList.remove('current');
    currentHighlight = (currentHighlight + delta + highlights.length) % highlights.length;
    highlights[currentHighlight].classList.add('current');
    highlights[currentHighlight].scrollIntoView({ behavior: 'smooth', block: 'center' });
    searchCount.textContent = (currentHighlight + 1) + ' of ' + highlights.length;
  }

  if (searchInput) {
    searchInput.addEventListener('input', doSearch);
    searchInput.addEventListener('keydown', function(e) {
      if (e.key === 'Enter') { e.preventDefault(); navigateSearch(e.shiftKey ? -1 : 1); }
      if (e.key === 'Escape') { searchInput.value = ''; clearHighlights(); searchCount.textContent = ''; searchInput.blur(); }
    });
  }
  if (searchPrev) searchPrev.addEventListener('click', function() { navigateSearch(-1); });
  if (searchNext) searchNext.addEventListener('click', function() { navigateSearch(1); });
  if (searchClose) {
    searchClose.addEventListener('click', function() {
      searchInput.value = '';
      clearHighlights();
      searchCount.textContent = '';
    });
  }

  // ── Keyboard Shortcuts ──
  document.addEventListener('keydown', function(e) {
    // Ctrl+F / Cmd+F -> focus search
    if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
      e.preventDefault();
      searchInput.focus();
      searchInput.select();
    }
  });

  // ── Download Button ──
  var downloadBtn = document.getElementById('btn-download');
  if (downloadBtn) {
    downloadBtn.addEventListener('click', function() {
      var scriptEl = document.getElementById('raw-markdown');
      if (!scriptEl) return;
      var b64 = scriptEl.getAttribute('data-b64') || '';
      var raw;
      try {
        raw = atob(b64);
      } catch(e) {
        alert('Failed to decode markdown content.');
        return;
      }
      var blob = new Blob([raw], { type: 'text/markdown;charset=utf-8' });
      var url = URL.createObjectURL(blob);
      var a = document.createElement('a');
      a.href = url;
      a.download = 'published-result.md';
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    });
  }

  // ── Init ──
  buildTOC();
  addCopyButtons();
  highlightCode();
})();
"##;
