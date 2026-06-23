use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::error::ApiError;
use crate::state::AppState;

/// Query parameters for `GET /api/search`.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// The search query string.
    pub q: Option<String>,
    /// Filter by result type: "sessions", "messages", or empty/absent for both.
    #[serde(default)]
    pub r#type: Option<String>,
    /// Page number (1-based).
    #[serde(default = "default_page")]
    pub page: u32,
    /// Results per page (max 100).
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_page() -> u32 {
    1
}
fn default_limit() -> u32 {
    20
}

/// A single search result item.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    /// "session" or "message"
    pub r#type: String,
    /// The session ID this result belongs to.
    pub session_id: String,
    /// The session name.
    pub session_name: String,
    /// A snippet of text with the match in context.
    pub snippet: String,
    /// Relevance score 0-100.
    pub score: u32,
    /// The message ID (only for message results).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
}

/// The search response envelope.
#[derive(Debug, serde::Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total: u64,
    pub page: u32,
}

/// `GET /api/search?q=query&type=sessions|messages&page=1&limit=20`
///
/// Searches sessions by name and messages by content using SQL LIKE.
/// Returns ranked results with relevance snippets and supports pagination.
pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Value>, ApiError> {
    let query = params.q.as_deref().unwrap_or("").trim();
    if query.is_empty() {
        return Ok(Json(json!(SearchResponse {
            results: vec![],
            total: 0,
            page: 1,
        })));
    }

    let search_type = params.r#type.as_deref().unwrap_or("");

    // Normalise pagination
    let page = params.page.max(1);
    let limit = params.limit.clamp(1, 100);
    let offset = ((page - 1) * limit) as usize;

    let mut all_results: Vec<SearchResult> = Vec::new();

    // ── Search sessions ──
    if search_type.is_empty() || search_type == "sessions" {
        let session_results = search_sessions(&state.db, query).await?;
        all_results.extend(session_results);
    }

    // ── Search messages ──
    if search_type.is_empty() || search_type == "messages" {
        let message_results = search_messages(&state.db, query).await?;
        all_results.extend(message_results);
    }

    // Sort by score descending, then by type (sessions first)
    all_results.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.r#type.cmp(&b.r#type))
    });

    let total = all_results.len() as u64;

    // Paginate the merged results
    let paged_results: Vec<SearchResult> = if offset < all_results.len() {
        let end = (offset + limit as usize).min(all_results.len());
        all_results[offset..end].to_vec()
    } else {
        vec![]
    };

    Ok(Json(json!(SearchResponse {
        total,
        page,
        results: paged_results,
    })))
}

/// Row type for session search queries.
#[derive(Debug, sqlx::FromRow)]
struct SessionSearchRow {
    id: String,
    name: String,
}

/// Search sessions by name matching.
async fn search_sessions(
    pool: &SqlitePool,
    query: &str,
) -> Result<Vec<SearchResult>, ApiError> {
    let pattern = format!("%{}%", query);

    let rows = sqlx::query_as::<_, SessionSearchRow>(
        "SELECT id, name FROM sessions WHERE name LIKE ?1 ORDER BY updated_at DESC",
    )
    .bind(&pattern)
    .fetch_all(pool)
    .await
    .map_err(agent_db::error::DbError::from)?;

    let results = rows
        .into_iter()
        .map(|row| {
            let snippet = build_session_snippet(&row.name, query);
            let score = calculate_score(&row.name, query);
            SearchResult {
                r#type: "session".to_string(),
                session_id: row.id,
                session_name: row.name,
                snippet,
                score,
                message_id: None,
            }
        })
        .collect();

    Ok(results)
}

/// Row type for message search queries.
#[derive(Debug, sqlx::FromRow)]
struct MessageSearchRow {
    id: String,
    session_id: String,
    content: String,
    session_name: String,
}

/// Search messages by content matching.
async fn search_messages(
    pool: &SqlitePool,
    query: &str,
) -> Result<Vec<SearchResult>, ApiError> {
    let pattern = format!("%{}%", query);

    let rows = sqlx::query_as::<_, MessageSearchRow>(
        "SELECT m.id, m.session_id, m.content, s.name AS session_name
         FROM messages m
         JOIN sessions s ON s.id = m.session_id
         WHERE m.content LIKE ?1
         ORDER BY m.created_at DESC
         LIMIT 200",
    )
    .bind(&pattern)
    .fetch_all(pool)
    .await
    .map_err(agent_db::error::DbError::from)?;

    let results = rows
        .into_iter()
        .map(|row| {
            let snippet = build_message_snippet(&row.content, query);
            let score = calculate_score(&row.content, query);
            SearchResult {
                r#type: "message".to_string(),
                session_id: row.session_id,
                session_name: row.session_name,
                snippet,
                score,
                message_id: Some(row.id),
            }
        })
        .collect();

    Ok(results)
}

/// Build a snippet for session name matches.
/// Shows the full name (it is short enough).
fn build_session_snippet(text: &str, _query: &str) -> String {
    if text.len() > 60 {
        format!("{}...", &text[..57])
    } else {
        text.to_string()
    }
}

/// Build a snippet for message content matches.
/// Extracts a window of context around the first match position.
fn build_message_snippet(content: &str, query: &str) -> String {
    let lower_content = content.to_lowercase();
    let lower_query = query.to_lowercase();

    const CONTEXT_CHARS: usize = 60;

    if let Some(pos) = lower_content.find(&lower_query) {
        let match_end = pos + query.len();

        // Determine context window (character-based, approximate)
        let context_start = pos.saturating_sub(CONTEXT_CHARS);
        let context_end = (match_end + CONTEXT_CHARS).min(content.len());

        let prefix = if context_start > 0 { "..." } else { "" };
        let suffix = if context_end < content.len() { "..." } else { "" };

        // Get the window, trying to break at word boundaries
        let window = &content[context_start..context_end];
        let window = window.trim();

        format!("{}{}{}", prefix, window, suffix)
    } else {
        // Fallback: first few characters
        if content.len() > 150 {
            format!("{}...", &content[..147])
        } else {
            content.to_string()
        }
    }
}

/// Calculate a relevance score (0-100) for a piece of text against the query.
fn calculate_score(text: &str, query: &str) -> u32 {
    let lower_text = text.to_lowercase();
    let lower_query = query.to_lowercase();

    if lower_text == lower_query {
        return 100;
    }

    if lower_text.starts_with(&lower_query) {
        return 85;
    }

    if lower_text.contains(&lower_query) {
        // How much of the text is the match?
        let ratio = query.len() as f64 / text.len().max(1) as f64;
        if ratio > 0.5 {
            return 75;
        }
        if ratio > 0.25 {
            return 60;
        }
        return 50;
    }

    // Partial word matching (for multi-word queries)
    let query_words: Vec<&str> = lower_query.split_whitespace().collect();
    if !query_words.is_empty() {
        let mut matched_words = 0u32;
        for word in &query_words {
            if lower_text.contains(word) {
                matched_words += 1;
            }
        }
        if matched_words > 0 {
            let ratio = matched_words as f64 / query_words.len() as f64;
            return (40.0 * ratio) as u32;
        }
    }

    0
}
