//! Monitoring endpoints for the built-in dashboard.
//!
//! - `GET /api/monitor` -- Full JSON monitoring stats.
//! - `POST /api/monitor/reset` -- Reset runtime counters.
//! - `GET /api/monitor/timeseries` -- Raw time-series data for charts.
//! - `GET /monitor` -- Self-contained HTML dashboard with auto-refresh and sparklines.

use axum::extract::State;
use axum::response::Html;
use axum::Json;
use serde_json::{json, Value};

use crate::state::AppState;

/// GET /api/monitor — returns all monitoring stats.
pub async fn monitor(State(state): State<AppState>) -> Json<Value> {
    // ── Uptime ─────────────────────────────────────
    let uptime_secs = state.start_time.elapsed().as_secs();

    // ── Request count ──────────────────────────────
    let request_count = state.get_request_count();

    // ── Active SSE connections ─────────────────────
    let active_sse = state.get_active_sse();

    // ── Active WebSocket connections ────────────────
    let active_ws = state.get_active_ws();

    // ── Total sessions, messages ──────────────────
    let total_sessions = state
        .session_repo
        .list()
        .await
        .map(|s| s.len())
        .unwrap_or(0);

    let total_messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    // ── DB size ────────────────────────────────────
    let db_size_bytes = tokio::fs::metadata(&state.db_path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);

    // ── Memory usage (RSS) ─────────────────────────
    let memory_rss = get_memory_rss();

    // ── LLM API stats ──────────────────────────────
    let (llm_total, llm_success, llm_error) = agent_core::llm::stats::get_llm_stats();

    // ── Per-channel message stats ──────────────────
    let per_channel = state.per_channel_msgs.read().await.clone();

    // ── Recent errors ──────────────────────────────
    let recent_errors: Vec<_> = crate::error::RECENT_ERRORS.read().await.clone();

    // ── Sample time-series ring buffers ─────────────
    state.sample_request_history().await;
    if let Some(rss) = memory_rss {
        state.sample_memory_history(rss).await;
    }
    state.sample_active_conn_history().await;

    let request_history = state.get_request_history().await;
    let memory_history = state.get_memory_history().await;
    let active_conn_history = state.get_active_conn_history().await;

    Json(json!({
        "server": {
            "uptime_secs": uptime_secs,
            "uptime_display": format_uptime(uptime_secs),
            "request_count": request_count,
            "active_sse_connections": active_sse,
            "active_ws_connections": active_ws,
        },
        "data": {
            "total_sessions": total_sessions,
            "total_messages": total_messages,
            "db_size_bytes": db_size_bytes,
            "db_size_display": format_bytes(db_size_bytes),
            "memory_rss_bytes": memory_rss,
            "memory_rss_display": memory_rss.map(format_bytes).unwrap_or_else(|| "N/A".into()),
        },
        "llm": {
            "calls_total": llm_total,
            "calls_success": llm_success,
            "calls_error": llm_error,
        },
        "channels": per_channel,
        "recent_errors": recent_errors,
        "timeseries": {
            "request_rate": request_history,
            "memory_usage": memory_history,
            "active_connections": active_conn_history,
        },
    }))
}

/// POST /api/monitor/reset — reset all runtime counters.
pub async fn monitor_reset(State(state): State<AppState>) -> Json<Value> {
    state.reset_counters().await;
    crate::error::RECENT_ERRORS.write().await.clear();
    Json(json!({
        "status": "ok",
        "message": "All monitoring counters have been reset",
    }))
}

/// GET /monitor — serve the monitoring dashboard HTML page.
pub async fn monitor_dashboard() -> Html<&'static str> {
    Html(MONITOR_HTML)
}

/// GET /api/monitor/timeseries — returns raw time-series data for external use.
pub async fn monitor_timeseries(State(state): State<AppState>) -> Json<Value> {
    let request_history = state.get_request_history().await;
    let memory_history = state.get_memory_history().await;
    let active_conn_history = state.get_active_conn_history().await;

    Json(json!({
        "request_rate": request_history,
        "memory_usage": memory_history,
        "active_connections": active_conn_history,
    }))
}

/// Self-contained monitoring dashboard page with auto-refresh and SVG sparkline charts.
const MONITOR_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>AI Agent Monitor</title>
<style>
  :root { --bg: #0f172a; --card: #1e293b; --text: #e2e8f0; --muted: #94a3b8; --accent: #38bdf8; --green: #4ade80; --red: #f87171; --yellow: #fbbf24; --purple: #a78bfa; --blue: #818cf8; --pink: #f472b6; }
  * { margin:0; padding:0; box-sizing:border-box; }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: var(--bg); color: var(--text); min-height: 100vh; }
  .header { background: var(--card); padding: 1.25rem 2rem; border-bottom: 1px solid #334155; display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:1rem; }
  .header h1 { font-size: 1.4rem; font-weight: 600; }
  .status { display:flex; align-items:center; gap:0.5rem; font-size:0.875rem; }
  .status-dot { width:10px; height:10px; border-radius:50%; background:var(--green); animation: pulse 2s infinite; }
  @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.4} }
  .container { max-width: 1200px; margin: 0 auto; padding: 1.5rem; display:grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap:1.25rem; }
  .card { background: var(--card); border-radius: 12px; padding: 1.25rem; border: 1px solid #334155; }
  .card h2 { font-size:0.8rem; text-transform:uppercase; letter-spacing:0.05em; color:var(--muted); margin-bottom:0.75rem; }
  .stat-row { display:flex; justify-content:space-between; align-items:center; padding:0.4rem 0; border-bottom:1px solid #1e293b; }
  .stat-row:last-child { border-bottom:none; }
  .stat-label { color: var(--muted); font-size:0.85rem; }
  .stat-value { font-family: 'Cascadia Code', 'Fira Code', monospace; font-size:0.95rem; font-weight:600; }
  .stat-value.green { color: var(--green); }
  .stat-value.red { color: var(--red); }
  .stat-value.accent { color: var(--accent); }
  .errors { max-height: 320px; overflow-y: auto; font-size:0.8rem; }
  .error-entry { padding:0.5rem 0; border-bottom:1px solid #1e293b; }
  .error-entry .ts { color: var(--muted); font-size:0.7rem; }
  .error-entry .msg { color: var(--red); margin-top:0.15rem; word-break:break-all; }
  .full-width { grid-column: 1 / -1; }
  .btn { background: var(--accent); color: #0f172a; border:none; padding:0.5rem 1.25rem; border-radius:8px; font-weight:600; cursor:pointer; font-size:0.85rem; }
  .btn:hover { opacity: 0.9; }
  .refresh-indicator { font-size:0.75rem; color:var(--muted); }
  .loading { opacity:0.5; }
  /* Sparkline chart grid */
  .charts-grid { display:grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap:1.5rem; width:100%; }
  .chart-box { text-align:center; }
  .chart-box .chart-label { font-size:0.7rem; text-transform:uppercase; letter-spacing:0.05em; color:var(--muted); margin-bottom:0.5rem; }
  .chart-box .chart-value { font-size:1.1rem; font-weight:700; font-family: 'Cascadia Code', 'Fira Code', monospace; margin-bottom:0.35rem; }
  .chart-box svg { margin:0 auto; }
  .chart-empty { color:var(--muted); font-size:0.75rem; padding:1.5rem 0; }
</style>
</head>
<body>
<div class="header">
  <div>
    <h1>AI Agent Monitor</h1>
    <span class="refresh-indicator" id="lastUpdate">Loading...</span>
  </div>
  <div style="display:flex;gap:0.75rem;align-items:center;">
    <span class="status"><span class="status-dot"></span> Online</span>
    <button class="btn" onclick="resetCounters()">Reset Counters</button>
  </div>
</div>
<div class="container" id="dashboard">
  <div class="card full-width" id="charts-card"><h2>Realtime Charts</h2><div class="charts-grid" id="charts-grid">Loading...</div></div>
  <div class="card"><h2>Server</h2><div id="server-stats">Loading...</div></div>
  <div class="card"><h2>Data</h2><div id="data-stats">Loading...</div></div>
  <div class="card"><h2>LLM API</h2><div id="llm-stats">Loading...</div></div>
  <div class="card"><h2>Channels</h2><div id="channel-stats">Loading...</div></div>
  <div class="card full-width"><h2>Recent Errors (last 20)</h2><div class="errors" id="error-stats">Loading...</div></div>
</div>

<script>
const API = '/api/monitor';
const RESET = '/api/monitor/reset';

let sparklineIdCounter = 0;

async function fetchStats() {
  try {
    const resp = await fetch(API);
    const data = await resp.json();
    render(data);
  } catch(e) {
    console.error('Monitor fetch error:', e);
  }
  document.getElementById('lastUpdate').textContent = 'Last update: ' + new Date().toLocaleTimeString();
}

function sparklineSVG(data, width, height, color, fmtVal) {
  if (!data || data.length < 2) {
    return '<div class="chart-empty">Collecting data...</div>';
  }
  var values = data.map(function(d) { return d[1]; });
  var max = Math.max.apply(null, values);
  var min = Math.min.apply(null, values);
  if (max === min) { max = min + 1; } // avoid flat line collapsing to zero height
  var range = max - min;
  var linePts = '';
  var stepX = width / (values.length - 1);
  for (var i = 0; i < values.length; i++) {
    var x = i * stepX;
    var y = height - 2 - ((values[i] - min) / range) * (height - 6);
    linePts += x.toFixed(1) + ',' + y.toFixed(1) + ' ';
  }
  // Close polygon from bottom corners for gradient fill
  var fillPts = '0,' + height + ' ' + linePts.trim() + ' ' + width + ',' + height;
  var gid = 'sparkGrad' + (++sparklineIdCounter);
  var currentVal = fmtVal ? fmtVal(values[values.length - 1]) : values[values.length - 1].toLocaleString();
  return '<div class="chart-value" style="color:' + color + '">' + currentVal + '</div>' +
    '<svg width="' + width + '" height="' + height + '" viewBox="0 0 ' + width + ' ' + height + '" style="display:block;">' +
    '<defs><linearGradient id="' + gid + '" x1="0" y1="0" x2="0" y2="1">' +
    '<stop offset="0%" stop-color="' + color + '" stop-opacity="0.25"/>' +
    '<stop offset="100%" stop-color="' + color + '" stop-opacity="0.02"/>' +
    '</linearGradient></defs>' +
    '<polygon points="' + fillPts + '" fill="url(#' + gid + ')"/>' +
    '<polyline points="' + linePts.trim() + '" fill="none" stroke="' + color + '" stroke-width="1.5" stroke-linejoin="round" stroke-linecap="round"/>' +
    '</svg>';
}

function formatRSS(bytes) {
  if (!bytes) return '0 B';
  var units = ['B','KB','MB','GB','TB'];
  var u = 0;
  var v = bytes;
  while (v >= 1024 && u < units.length - 1) { v /= 1024; u++; }
  return v.toFixed(1) + ' ' + units[u];
}

function renderCharts(ts) {
  if (!ts) { document.getElementById('charts-grid').innerHTML = '<div class="chart-empty">No timeseries data available</div>'; return; }
  var html = '';
  html += '<div class="chart-box"><div class="chart-label">Request Count</div>' +
    sparklineSVG(ts.request_rate, 300, 60, '#a78bfa', null) + '</div>';
  html += '<div class="chart-box"><div class="chart-label">Memory Usage (RSS)</div>' +
    sparklineSVG(ts.memory_usage, 300, 60, '#818cf8', formatRSS) + '</div>';
  html += '<div class="chart-box"><div class="chart-label">Active Connections (SSE + WS)</div>' +
    sparklineSVG(ts.active_connections, 300, 60, '#f472b6', null) + '</div>';
  document.getElementById('charts-grid').innerHTML = html;
}

function render(d) {
  // Sparkline charts (top card)
  renderCharts(d.timeseries);

  document.getElementById('server-stats').innerHTML =
    row('Uptime', d.server.uptime_display) +
    row('Requests', d.server.request_count.toLocaleString(), 'accent') +
    row('Active SSE', d.server.active_sse_connections.toLocaleString(), d.server.active_sse_connections>0?'green':'') +
    row('Active WS', (d.server.active_ws_connections||0).toLocaleString(), (d.server.active_ws_connections||0)>0?'green':'');

  document.getElementById('data-stats').innerHTML =
    row('Sessions', d.data.total_sessions.toLocaleString()) +
    row('Messages', d.data.total_messages.toLocaleString()) +
    row('DB Size', d.data.db_size_display) +
    row('Memory RSS', d.data.memory_rss_display);

  const llm = d.llm;
  const errRate = llm.calls_total > 0 ? ((llm.calls_error / llm.calls_total)*100).toFixed(1) : 0;
  document.getElementById('llm-stats').innerHTML =
    row('Total Calls', llm.calls_total.toLocaleString(), 'accent') +
    row('Success', llm.calls_success.toLocaleString(), 'green') +
    row('Errors', llm.calls_error.toLocaleString(), llm.calls_error>0?'red':'') +
    row('Error Rate', errRate + '%', errRate>5?'red':'');

  let chHTML = '';
  const ch = d.channels || {};
  if (Object.keys(ch).length === 0) {
    chHTML = '<div class="stat-row"><span class="stat-label">No channel activity yet</span></div>';
  } else {
    for (const [name, count] of Object.entries(ch)) {
      chHTML += row(name, count.toLocaleString());
    }
  }
  document.getElementById('channel-stats').innerHTML = chHTML;

  const errs = d.recent_errors || [];
  let errHTML = '';
  if (errs.length === 0) {
    errHTML = '<div class="stat-row"><span class="stat-label">No recent errors</span></div>';
  } else {
    for (const e of errs) {
      errHTML += `<div class="error-entry"><div class="ts">${e.timestamp}</div><div class="msg">${esc(e.message)}</div></div>`;
    }
  }
  document.getElementById('error-stats').innerHTML = errHTML;
}

function row(label, value, cls) {
  return `<div class="stat-row"><span class="stat-label">${label}</span><span class="stat-value ${cls||''}">${value}</span></div>`;
}

function esc(s) { return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;'); }

async function resetCounters() {
  if (!confirm('Reset all monitoring counters?')) return;
  try {
    await fetch(RESET, { method: 'POST' });
    fetchStats();
  } catch(e) {
    alert('Reset failed: ' + e);
  }
}

fetchStats();
setInterval(fetchStats, 3000);
</script>
</body>
</html>"#;

// ── Helpers ──────────────────────────────────────────────────────────

fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, seconds)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    format!("{:.1} {}", size, UNITS[unit_idx])
}

/// Get the current process RSS memory in bytes using the `sysinfo` crate.
fn get_memory_rss() -> Option<u64> {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let mut sys = System::new();
    let pid = Pid::from_u32(std::process::id());
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    sys.process(pid).map(|p| p.memory())
}
