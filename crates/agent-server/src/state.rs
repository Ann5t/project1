//! Shared application state injected into all Axum handlers via
//! [`axum::extract::State`].
//!
//! [`AppState`] owns the database pool, LLM client, tool registry,
//! session/workflow/scheduler engines, repository layer, and monitoring
//! counters. It is `Clone` so Axum can hand a reference to every handler.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use agent_core::llm::client::LlmClient;
use agent_core::scheduler::engine::TaskSchedulerEngine;
use agent_core::session::manager::SessionManager;
use agent_core::tool::registry::ToolRegistry;
use agent_core::workflow::engine::WorkflowEngine;
use agent_db::repo::{
    ChannelRepo, ConfigRepo, MessageRepo, SessionRepo, TaskRepo, WorkflowRepo,
};
use serde::Serialize;
use serde_json::Value;
use sqlx::SqlitePool;
use tokio::sync::{broadcast, RwLock};

use crate::middleware::rate_limit::RateLimiter;

/// A WebSocket event pushed to all connected clients.
#[derive(Debug, Clone, Serialize)]
pub struct WsEvent {
    /// Event type discriminator (e.g. "session_created", "message_received").
    #[serde(rename = "type")]
    pub event_type: String,
    /// Arbitrary JSON payload associated with the event.
    pub data: Value,
    /// ISO-8601 timestamp when the event was emitted.
    pub timestamp: String,
}

/// Shared application state injected into all Axum handlers.
///
/// Constructed once at startup in `main.rs` and cloned
/// into every request handler via `State<AppState>`.
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub llm: Arc<dyn LlmClient + Send + Sync>,
    pub tools: Arc<ToolRegistry>,
    pub session_manager: Arc<SessionManager>,
    pub workflow_engine: Arc<WorkflowEngine>,
    pub scheduler: Arc<TaskSchedulerEngine>,

    // Repos
    pub config_repo: ConfigRepo,
    pub session_repo: SessionRepo,
    pub message_repo: MessageRepo,
    pub channel_repo: ChannelRepo,
    pub workflow_repo: WorkflowRepo,
    pub task_repo: TaskRepo,

    // ── Monitoring ────────────────────────────
    /// When the server started (for uptime calculation)
    pub start_time: Instant,
    /// Total HTTP requests served (updated by middleware)
    pub request_count: Arc<AtomicU64>,
    /// Request breakdown by (method, path, status) for Prometheus metrics
    pub request_breakdown: Arc<RwLock<HashMap<String, u64>>>,
    /// Active SSE connections count
    pub active_sse_connections: Arc<AtomicU64>,
    /// Active WebSocket connections count
    pub active_ws_connections: Arc<AtomicU64>,
    /// Per-channel message counts
    pub per_channel_msgs: Arc<RwLock<HashMap<String, u64>>>,
    /// Database file path (for size check)
    pub db_path: String,
    /// Total rate-limited requests (429 responses)
    pub rate_limits_total: Arc<AtomicU64>,

    // ── Time-series ring buffers (monitor sparklines) ──
    /// Request-rate history: (unix_timestamp_secs, count)
    pub request_history: Arc<RwLock<VecDeque<(i64, u64)>>>,
    /// Memory usage history: (unix_timestamp_secs, bytes)
    pub memory_history: Arc<RwLock<VecDeque<(i64, u64)>>>,
    /// Active connections (SSE + WS) history: (unix_timestamp_secs, count)
    pub active_conn_history: Arc<RwLock<VecDeque<(i64, u64)>>>,

    // ── Authentication ────────────────────────
    /// Whether token-based auth is enabled (shared so config changes take effect immediately)
    pub auth_enabled: Arc<AtomicBool>,
    /// The admin token used to authenticate API requests (shared for live updates)
    pub admin_token: Arc<RwLock<String>>,

    // ── Rate limiting ─────────────────────────
    /// Per-IP sliding-window rate limiter for global and chat endpoints.
    pub rate_limiter: RateLimiter,

    // ── WebSocket broadcast ───────────────────
    /// Broadcast sender for pushing real-time events to all connected WebSocket clients.
    pub ws_tx: broadcast::Sender<WsEvent>,

    // ── Email notifications ───────────────────
    /// Optional email notifier for SMTP notifications.
    pub email_notifier: Option<Arc<crate::notifications::email::EmailNotifier>>,
}

impl AppState {
    /// Build the full application state from a database pool, LLM client, and
    /// tool registry. All other fields (session manager, workflow engine,
    /// scheduler, repos, monitoring counters) are created internally.
    pub fn new(
        db: SqlitePool,
        llm: Arc<dyn LlmClient + Send + Sync>,
        tools: Arc<ToolRegistry>,
    ) -> Self {
        let session_manager = Arc::new(SessionManager::new(
            llm.clone(),
            tools.clone(),
            db.clone(),
        ));

        let workflow_engine = Arc::new(WorkflowEngine::new(
            llm.clone(),
            tools.clone(),
        ));

        let scheduler = Arc::new(TaskSchedulerEngine::new(
            llm.clone(),
            tools.clone(),
        ));

        // Broadcast channel for WebSocket real-time events (capacity: 256).
        let (ws_tx, _) = broadcast::channel(256);

        Self {
            config_repo: ConfigRepo::new(db.clone()),
            session_repo: SessionRepo::new(db.clone()),
            message_repo: MessageRepo::new(db.clone()),
            channel_repo: ChannelRepo::new(db.clone()),
            workflow_repo: WorkflowRepo::new(db.clone()),
            task_repo: TaskRepo::new(db.clone()),
            db,
            llm,
            tools,
            session_manager,
            workflow_engine,
            scheduler,
            start_time: Instant::now(),
            request_count: Arc::new(AtomicU64::new(0)),
            request_breakdown: Arc::new(RwLock::new(HashMap::new())),
            active_sse_connections: Arc::new(AtomicU64::new(0)),
            active_ws_connections: Arc::new(AtomicU64::new(0)),
            per_channel_msgs: Arc::new(RwLock::new(HashMap::new())),
            db_path: String::new(),
            rate_limits_total: Arc::new(AtomicU64::new(0)),
            request_history: Arc::new(RwLock::new(VecDeque::with_capacity(60))),
            memory_history: Arc::new(RwLock::new(VecDeque::with_capacity(60))),
            active_conn_history: Arc::new(RwLock::new(VecDeque::with_capacity(60))),
            auth_enabled: Arc::new(AtomicBool::new(false)),
            admin_token: Arc::new(RwLock::new(String::new())),
            rate_limiter: RateLimiter::new(100, 10, 5, false),
            ws_tx,
            email_notifier: None,
        }
    }

    /// Check whether auth is currently enabled.
    pub fn is_auth_enabled(&self) -> bool {
        self.auth_enabled.load(Ordering::Relaxed)
    }

    /// Enable or disable auth at runtime.
    pub fn set_auth_enabled(&self, enabled: bool) {
        self.auth_enabled.store(enabled, Ordering::Relaxed);
    }

    /// Get a copy of the current admin token.
    pub async fn get_admin_token(&self) -> String {
        self.admin_token.read().await.clone()
    }

    /// Update the admin token at runtime.
    pub async fn set_admin_token(&self, token: &str) {
        *self.admin_token.write().await = token.to_string();
    }

    /// Increment the per-channel message counter (for the monitoring dashboard).
    pub async fn increment_channel_msg(&self, channel: &str) {
        let mut map = self.per_channel_msgs.write().await;
        *map.entry(channel.to_string()).or_insert(0) += 1;
    }

    /// Get the current request count.
    pub fn get_request_count(&self) -> u64 {
        self.request_count.load(Ordering::Relaxed)
    }

    /// Increment the request count (called from middleware).
    pub fn increment_request_count(&self) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the current active SSE connections count.
    pub fn get_active_sse(&self) -> u64 {
        self.active_sse_connections.load(Ordering::Relaxed)
    }

    /// Increment active SSE connections.
    pub fn sse_connected(&self) {
        self.active_sse_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active SSE connections.
    pub fn sse_disconnected(&self) {
        self.active_sse_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get the current active WebSocket connections count.
    pub fn get_active_ws(&self) -> u64 {
        self.active_ws_connections.load(Ordering::Relaxed)
    }

    /// Increment active WebSocket connections.
    pub fn ws_connected(&self) {
        self.active_ws_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active WebSocket connections.
    pub fn ws_disconnected(&self) {
        self.active_ws_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Broadcast a real-time event to all connected WebSocket clients.
    ///
    /// This is a fire-and-forget operation: if there are no receivers the
    /// event is simply dropped by the broadcast channel.
    pub fn broadcast_event(&self, event_type: &str, data: Value) {
        let event = WsEvent {
            event_type: event_type.to_string(),
            data,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        // Ignore send errors (no receivers is the most common case)
        let _ = self.ws_tx.send(event);
    }

    /// Increment the rate-limit hit counter (called from rate-limit middleware on 429).
    pub fn increment_rate_limit(&self) {
        self.rate_limits_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the current rate-limit hit count.
    pub fn get_rate_limits_total(&self) -> u64 {
        self.rate_limits_total.load(Ordering::Relaxed)
    }

    /// Increment the per-(method, path, status) request breakdown counter.
    pub async fn increment_request_breakdown(&self, method: &str, path: &str, status: u16) {
        let key = format!("{}|{}|{}", method, path, status);
        let mut map = self.request_breakdown.write().await;
        *map.entry(key).or_insert(0) += 1;
    }

    /// Get a snapshot of the request breakdown map.
    pub async fn get_request_breakdown_snapshot(&self) -> HashMap<String, u64> {
        self.request_breakdown.read().await.clone()
    }

    /// Sample the current request count into the ring buffer.
    pub async fn sample_request_history(&self) {
        let count = self.get_request_count();
        let ts = chrono::Utc::now().timestamp();
        let mut hist = self.request_history.write().await;
        hist.push_back((ts, count));
        if hist.len() > 60 {
            hist.pop_front();
        }
    }

    /// Sample current memory usage (bytes) into the ring buffer.
    pub async fn sample_memory_history(&self, rss: u64) {
        let ts = chrono::Utc::now().timestamp();
        let mut hist = self.memory_history.write().await;
        hist.push_back((ts, rss));
        if hist.len() > 60 {
            hist.pop_front();
        }
    }

    /// Sample current active connections (SSE + WS) into the ring buffer.
    pub async fn sample_active_conn_history(&self) {
        let count = self.get_active_sse() + self.get_active_ws();
        let ts = chrono::Utc::now().timestamp();
        let mut hist = self.active_conn_history.write().await;
        hist.push_back((ts, count));
        if hist.len() > 60 {
            hist.pop_front();
        }
    }

    /// Get a snapshot of the request-rate history.
    pub async fn get_request_history(&self) -> Vec<(i64, u64)> {
        self.request_history.read().await.iter().copied().collect()
    }

    /// Get a snapshot of the memory-usage history.
    pub async fn get_memory_history(&self) -> Vec<(i64, u64)> {
        self.memory_history.read().await.iter().copied().collect()
    }

    /// Get a snapshot of the active-connections history.
    pub async fn get_active_conn_history(&self) -> Vec<(i64, u64)> {
        self.active_conn_history.read().await.iter().copied().collect()
    }

    /// Reset all monitoring counters.
    pub async fn reset_counters(&self) {
        self.request_count.store(0, Ordering::Relaxed);
        self.request_breakdown.write().await.clear();
        self.active_sse_connections.store(0, Ordering::Relaxed);
        self.active_ws_connections.store(0, Ordering::Relaxed);
        self.per_channel_msgs.write().await.clear();
        self.rate_limits_total.store(0, Ordering::Relaxed);
        self.request_history.write().await.clear();
        self.memory_history.write().await.clear();
        self.active_conn_history.write().await.clear();
        agent_core::llm::stats::reset_llm_stats();
    }
}
