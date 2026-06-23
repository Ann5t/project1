//! Prometheus `/api/metrics` endpoint.
//!
//! Returns Prometheus text-format metrics for scraping by Prometheus,
//! Grafana, or any OpenMetrics-compatible collector.
//!
//! All metric names use the `ai_agent_` prefix to follow Prometheus
//! naming conventions for a single binary.

use axum::extract::State;
use axum::http::header;
use axum::response::{IntoResponse, Response};

use crate::state::AppState;

/// GET /api/metrics — returns Prometheus text-format metrics.
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let body = build_metrics(&state).await;

    Response::builder()
        .header(header::CONTENT_TYPE, "text/plain; version=0.0.4")
        .body(axum::body::Body::from(body))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(500)
                .body(axum::body::Body::from("Internal Server Error"))
                .expect("Failed to build metrics error response")
        })
}

/// Build the full Prometheus text-format body from current state.
async fn build_metrics(state: &AppState) -> String {
    let mut out = String::with_capacity(4096);

    // ── ai_agent_uptime_seconds (gauge) ──────────────────────────
    let uptime = state.start_time.elapsed().as_secs_f64();
    out.push_str("# HELP ai_agent_uptime_seconds Server uptime in seconds\n");
    out.push_str("# TYPE ai_agent_uptime_seconds gauge\n");
    out.push_str(&format!("ai_agent_uptime_seconds {:.3}\n", uptime));
    out.push('\n');

    // ── ai_agent_requests_total (counter, labels: method, path, status) ─
    let breakdown = state.get_request_breakdown_snapshot().await;
    out.push_str("# HELP ai_agent_requests_total Total HTTP requests\n");
    out.push_str("# TYPE ai_agent_requests_total counter\n");
    for (key, count) in breakdown.iter() {
        // key format: "METHOD|path|status"
        let mut parts = key.splitn(3, '|');
        let method = parts.next().unwrap_or("UNKNOWN");
        let path = parts.next().unwrap_or("/");
        let status = parts.next().unwrap_or("0");
        out.push_str(&format!(
            "ai_agent_requests_total{{method=\"{}\",path=\"{}\",status=\"{}\"}} {}\n",
            method, path, status, count
        ));
    }
    // Always emit at least one line (empty metric makes promtool complain).
    if breakdown.is_empty() {
        out.push_str("ai_agent_requests_total{method=\"\",path=\"\",status=\"\"} 0\n");
    }
    out.push('\n');

    // ── ai_agent_active_sse_connections (gauge) ───────────────────
    let sse = state.get_active_sse();
    out.push_str("# HELP ai_agent_active_sse_connections Active SSE connections\n");
    out.push_str("# TYPE ai_agent_active_sse_connections gauge\n");
    out.push_str(&format!("ai_agent_active_sse_connections {}\n", sse));
    out.push('\n');

    // ── ai_agent_active_ws_connections (gauge) ───────────────────
    let ws = state.get_active_ws();
    out.push_str("# HELP ai_agent_active_ws_connections Active WebSocket connections\n");
    out.push_str("# TYPE ai_agent_active_ws_connections gauge\n");
    out.push_str(&format!("ai_agent_active_ws_connections {}\n", ws));
    out.push('\n');

    // ── ai_agent_sessions_total (gauge) ───────────────────────────
    let total_sessions = state
        .session_repo
        .list()
        .await
        .map(|s| s.len())
        .unwrap_or(0);
    out.push_str("# HELP ai_agent_sessions_total Total sessions\n");
    out.push_str("# TYPE ai_agent_sessions_total gauge\n");
    out.push_str(&format!("ai_agent_sessions_total {}\n", total_sessions));
    out.push('\n');

    // ── ai_agent_messages_total (gauge) ───────────────────────────
    let total_messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    out.push_str("# HELP ai_agent_messages_total Total messages\n");
    out.push_str("# TYPE ai_agent_messages_total gauge\n");
    out.push_str(&format!("ai_agent_messages_total {}\n", total_messages));
    out.push('\n');

    // ── ai_agent_llm_calls_total (counter, labels: status) ────────
    let (_llm_total, llm_success, llm_error) = agent_core::llm::stats::get_llm_stats();
    out.push_str("# HELP ai_agent_llm_calls_total Total LLM API calls\n");
    out.push_str("# TYPE ai_agent_llm_calls_total counter\n");
    out.push_str(&format!(
        "ai_agent_llm_calls_total{{status=\"success\"}} {}\n",
        llm_success
    ));
    out.push_str(&format!(
        "ai_agent_llm_calls_total{{status=\"error\"}} {}\n",
        llm_error
    ));
    out.push('\n');

    // ── ai_agent_db_size_bytes (gauge) ───────────────────────────
    let db_size_bytes = tokio::fs::metadata(&state.db_path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    out.push_str("# HELP ai_agent_db_size_bytes Database file size in bytes\n");
    out.push_str("# TYPE ai_agent_db_size_bytes gauge\n");
    out.push_str(&format!("ai_agent_db_size_bytes {}\n", db_size_bytes));
    out.push('\n');

    // ── ai_agent_memory_rss_bytes (gauge) ─────────────────────────
    let memory_rss = get_memory_rss().unwrap_or(0);
    out.push_str("# HELP ai_agent_memory_rss_bytes Process RSS memory in bytes\n");
    out.push_str("# TYPE ai_agent_memory_rss_bytes gauge\n");
    out.push_str(&format!("ai_agent_memory_rss_bytes {}\n", memory_rss));
    out.push('\n');

    // ── ai_agent_channel_messages_total (counter, labels: channel) ─
    let per_channel = state.per_channel_msgs.read().await.clone();
    out.push_str("# HELP ai_agent_channel_messages_total Messages per channel\n");
    out.push_str("# TYPE ai_agent_channel_messages_total counter\n");
    if per_channel.is_empty() {
        out.push_str("ai_agent_channel_messages_total{channel=\"\"} 0\n");
    } else {
        for (channel, count) in per_channel.iter() {
            out.push_str(&format!(
                "ai_agent_channel_messages_total{{channel=\"{}\"}} {}\n",
                channel, count
            ));
        }
    }
    out.push('\n');

    // ── ai_agent_rate_limits_total (counter) ──────────────────────
    let rate_limits = state.get_rate_limits_total();
    out.push_str("# HELP ai_agent_rate_limits_total Total rate-limited requests\n");
    out.push_str("# TYPE ai_agent_rate_limits_total counter\n");
    out.push_str(&format!("ai_agent_rate_limits_total {}\n", rate_limits));
    out.push('\n');

    out
}

/// Get the current process RSS memory in bytes using the `sysinfo` crate.
fn get_memory_rss() -> Option<u64> {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let mut sys = System::new();
    let pid = Pid::from_u32(std::process::id());
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    sys.process(pid).map(|p| p.memory())
}
