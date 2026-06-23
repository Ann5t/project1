use agent_server::config;
use agent_server::middleware;
use agent_server::notifications::email::{EmailNotifier, GLOBAL_EMAIL_NOTIFIER};
use agent_server::routes;
use agent_server::state;

use std::sync::Arc;

use axum::http::StatusCode;
use axum::middleware as axum_mw;
use axum::routing::{delete, get, post, put};
use axum::Router;
use std::time::Duration;

use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::info;

use agent_core::llm::client::DeepSeekClient;
use agent_core::tool::builtin::{
    CalculatorTool, CurrentTimeTool, ExecuteShellTool, ReadFileTool, WebSearchTool,
};
use agent_core::tool::registry::ToolRegistry;
use config::ServerConfig;
use state::AppState;

use futures::future::FutureExt;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Helper: map the result of a tokio timeout so `Elapsed` becomes an
/// `Ok(408 response)` and the error type stays `Infallible`.
fn handle_timeout_error<E>(
    result: Result<Result<axum::response::Response, E>, tokio::time::error::Elapsed>,
) -> std::future::Ready<Result<axum::response::Response, E>> {
    match result {
        Ok(inner) => std::future::ready(inner),
        Err(_elapsed) => {
            let resp = axum::response::Response::builder()
                .status(StatusCode::REQUEST_TIMEOUT)
                .body(axum::body::Body::from("Request timed out"))
                .unwrap();
            std::future::ready(Ok(resp))
        }
    }
}

/// Custom Layer that wraps each request with a tokio timeout.
/// Returns HTTP 408 on timeout so the error type stays `Infallible`,
/// compatible with axum 0.8's `Router::layer` constraint.
#[derive(Clone, Debug)]
struct RequestTimeoutLayer {
    duration: Duration,
}

impl RequestTimeoutLayer {
    fn from_env_or(default_secs: u64) -> Self {
        let secs = std::env::var("REQUEST_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default_secs);
        Self {
            duration: Duration::from_secs(secs),
        }
    }
}

impl<S> Layer<S> for RequestTimeoutLayer {
    type Service = RequestTimeoutService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestTimeoutService {
            inner,
            duration: self.duration,
        }
    }
}

#[derive(Clone, Debug)]
struct RequestTimeoutService<S> {
    inner: S,
    duration: Duration,
}

impl<S, ReqBody> Service<axum::http::Request<ReqBody>> for RequestTimeoutService<S>
where
    S: Service<axum::http::Request<ReqBody>, Response = axum::response::Response>,
    S::Error: Into<S::Error>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = futures::future::Then<
        tokio::time::Timeout<S::Future>,
        std::future::Ready<Result<Self::Response, Self::Error>>,
        fn(
            Result<Result<Self::Response, Self::Error>, tokio::time::error::Elapsed>,
        ) -> std::future::Ready<Result<Self::Response, Self::Error>>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: axum::http::Request<ReqBody>) -> Self::Future {
        let fut = self.inner.call(req);
        let duration = self.duration;
        tokio::time::timeout(duration, fut).then(handle_timeout_error)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Load configuration
    let config = ServerConfig::from_env();

    info!("Starting AI Agent Server...");
    info!("Database path: {}", config.database_path);
    info!("Frontend dir: {}", config.frontend_dir);
    info!("Binding to: {}", config.bind_address);

    // Initialize database
    let db = agent_db::init_db(&config.database_path).await?;
    info!("Database initialized");

    // ── Authentication setup ──────────────────
    // Create a ConfigRepo for configuration lookups
    let config_repo = agent_db::repo::ConfigRepo::new(db.clone());

    // Generate an admin token on first startup if none exists
    let admin_token = {
        let existing = config_repo.get("admin_token").await?.unwrap_or_default();
        if existing.is_empty() {
            let token = uuid::Uuid::new_v4().to_string();
            config_repo.set("admin_token", &token).await?;
            // WARNING: The admin token is the only credential for the admin panel.
            // It is stored in the config DB. Treat it like a password.
            // The token is NOT logged here to prevent leakage via logs/stdout.
            info!("Generated new admin_token (not logged). Save it from the config DB.");
            info!("IMPORTANT: Retrieve the token from the admin_token config key or database.");
            token
        } else {
            // Mask the token in logs
            let masked = if existing.len() > 8 {
                format!("{}...{}", &existing[..4], &existing[existing.len() - 4..])
            } else {
                "****".to_string()
            };
            info!("Loaded existing admin_token: {}", masked);
            existing
        }
    };

    // Check whether auth is enabled
    let auth_enabled = config_repo
        .get("auth_enabled")
        .await?
        .is_some_and(|v| v == "true");
    info!("Authentication enabled: {}", auth_enabled);

    // ── Rate limit configuration ──────────────
    let rate_limit_enabled = config_repo
        .get("rate_limit_enabled")
        .await?
        .is_none_or(|v| v == "true");
    let rate_limit_global_rpm: u32 = config_repo
        .get("rate_limit_global_rpm")
        .await?
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    let rate_limit_chat_rpm: u32 = config_repo
        .get("rate_limit_chat_rpm")
        .await?
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    info!(
        "Rate limiting: enabled={}, global={} rpm, chat={} rpm",
        rate_limit_enabled, rate_limit_global_rpm, rate_limit_chat_rpm
    );

    // Get LLM configuration from DB
    let api_key = { config_repo.get("api_key").await?.unwrap_or_default() };

    // Create LLM client (keep concrete type for shutdown signalling)
    let deepseek = Arc::new(DeepSeekClient::new(
        api_key,
        Some("https://api.deepseek.com/v1".into()),
        Some("deepseek-chat".into()),
    ));
    let llm_shutdown_notify = deepseek.shutdown_notify.clone();
    let llm: Arc<dyn agent_core::llm::client::LlmClient + Send + Sync> = deepseek.clone();

    // Create tool registry and register built-in tools
    let tools = Arc::new(ToolRegistry::new());
    tools.register(Arc::new(CalculatorTool)).await;
    tools.register(Arc::new(CurrentTimeTool)).await;
    tools.register(Arc::new(WebSearchTool)).await;
    tools.register(Arc::new(ReadFileTool::from_env())).await;

    // Shell tool is opt-in: only registered when SHELL_TOOL_ENABLED=true
    #[allow(clippy::disallowed_methods)]
    let shell_enabled = std::env::var("SHELL_TOOL_ENABLED")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false);
    if shell_enabled {
        tracing::warn!(
            "ExecuteShellTool is ENABLED. This allows shell command execution on the server."
        );
        tools.register(Arc::new(ExecuteShellTool::from_env())).await;
    } else {
        tracing::info!("ExecuteShellTool is disabled. Set SHELL_TOOL_ENABLED=true to enable it.");
    }

    info!("Tools registered: {:?}", tools.list_names().await);

    // Build application state
    let mut app_state = AppState::new(db, llm, tools);
    app_state.db_path.clone_from(&config.database_path);
    app_state.set_auth_enabled(auth_enabled);
    app_state.set_admin_token(&admin_token).await;
    // Override the default rate limiter with configured values
    app_state.rate_limiter = agent_server::middleware::rate_limit::RateLimiter::new(
        rate_limit_global_rpm,
        rate_limit_chat_rpm,
        5, // SSE max concurrent per IP
        rate_limit_enabled,
    );

    // Start scheduler
    let scheduler = Arc::clone(&app_state.scheduler);
    tokio::spawn(async move {
        if let Err(e) = scheduler.start() {
            tracing::error!("Scheduler failed to start: {}", e);
        }
    });

    // ── Email notifier ─────────────────────────
    let email_notifier = EmailNotifier::from_config(&config_repo).await;
    if let Some(ref notifier) = email_notifier {
        GLOBAL_EMAIL_NOTIFIER
            .set(std::sync::Arc::new(notifier.clone()))
            .ok();
        app_state.email_notifier = Some(std::sync::Arc::new(notifier.clone()));
    }

    // Start auto-backup task
    let backup_state = app_state.clone();
    tokio::spawn(async move {
        routes::backup::start_auto_backup(backup_state).await;
    });

    // Build routes
    //
    // Chat routes get a stricter rate limiter in addition to the global one.
    let chat_routes = Router::new()
        .route("/api/chat", post(routes::chat::send_message))
        .route("/api/chat/stream", post(routes::chat::stream_message))
        .route_layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::rate_limit::rate_limit_chat,
        ));

    let api_routes = Router::new()
        // Health
        .route("/api/health", get(routes::health::health_check))
        .route("/api/info", get(routes::health::system_info))
        // Metrics (Prometheus)
        .route("/api/metrics", get(routes::metrics::metrics))
        // Auth
        .route("/api/auth/status", get(routes::auth::auth_status))
        .route("/api/auth/login", post(routes::auth::auth_login))
        // Config
        .route("/api/config", get(routes::config_api::get_all))
        .route("/api/config", put(routes::config_api::update_all))
        .route("/api/config/{key}", get(routes::config_api::get_one))
        .route("/api/config/{key}", put(routes::config_api::set_one))
        // Sessions
        .route("/api/sessions", get(routes::session::list))
        .route("/api/sessions", post(routes::session::create))
        .route("/api/sessions/{id}", get(routes::session::get_one))
        .route("/api/sessions/{id}", put(routes::session::update))
        .route("/api/sessions/{id}", delete(routes::session::delete))
        .route(
            "/api/sessions/{id}/messages",
            get(routes::session::messages),
        )
        // Search
        .route("/api/search", get(routes::search::search))
        // Chat (with its own rate-limiter layer)
        .merge(chat_routes)
        // Channels
        .route("/api/channels", get(routes::channel::list))
        .route("/api/channels", post(routes::channel::create))
        .route("/api/channels/{id}", put(routes::channel::update))
        .route("/api/channels/{id}", delete(routes::channel::delete))
        .route("/api/channels/{id}/test", post(routes::channel::test))
        .route(
            "/api/channels/feishu/callback",
            post(routes::channel::feishu_callback),
        )
        .route(
            "/api/channels/wechat_work/callback",
            get(routes::channel::wechat_work_verify),
        )
        .route(
            "/api/channels/wechat_work/callback",
            post(routes::channel::wechat_work_callback),
        )
        .route(
            "/api/channels/webhook/{path}",
            post(routes::channel::webhook_callback),
        )
        // Workflows
        .route("/api/workflows", get(routes::workflow::list))
        .route("/api/workflows", post(routes::workflow::create))
        .route("/api/workflows/{id}", get(routes::workflow::get_one))
        .route("/api/workflows/{id}", put(routes::workflow::update))
        .route("/api/workflows/{id}", delete(routes::workflow::delete))
        .route("/api/workflows/{id}/run", post(routes::workflow::run))
        .route("/api/workflows/{id}/runs", get(routes::workflow::runs))
        // Tasks
        .route("/api/tasks", get(routes::task::list))
        .route("/api/tasks", post(routes::task::create))
        .route("/api/tasks/{id}", get(routes::task::get_one))
        .route("/api/tasks/{id}", put(routes::task::update))
        .route("/api/tasks/{id}", delete(routes::task::delete))
        .route("/api/tasks/{id}/run", post(routes::task::run_now))
        .route("/api/tasks/{id}/logs", get(routes::task::logs))
        // Monitor
        .route("/api/monitor", get(routes::monitor::monitor))
        .route(
            "/api/monitor/reset",
            axum::routing::post(routes::monitor::monitor_reset),
        )
        .route(
            "/api/monitor/timeseries",
            get(routes::monitor::monitor_timeseries),
        )
        // WebSocket real-time events
        .route("/api/ws", get(routes::ws::ws_handler))
        // Backup
        .route("/api/backup", get(routes::backup::backup))
        .route("/api/backup/restore", post(routes::backup::restore))
        .route("/api/backup/list", get(routes::backup::list_backups))
        // Export
        .route(
            "/api/export/session/{id}",
            get(routes::export::export_session),
        )
        .route(
            "/api/export/workflow/{id}/runs",
            get(routes::export::export_workflow_runs),
        )
        .route("/api/export/bulk", post(routes::export::export_bulk))
        // Publish
        .route("/p/{publish_id}", get(routes::publish::get_published))
        .route(
            "/api/publish/{id}",
            delete(routes::publish::delete_published),
        )
        // Notifications
        .route(
            "/api/notifications/test-email",
            post(routes::notifications::test_email),
        )
        // OpenAPI / Docs
        .route("/api/openapi.json", get(routes::openapi::openapi_json))
        .route("/api/docs", get(routes::openapi::api_docs))
        // Apply auth middleware to all API routes (except public paths handled inside)
        .route_layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::authenticate,
        ));

    // Monitor dashboard (serves HTML at /monitor)
    let monitor_router = Router::new().route("/monitor", get(routes::monitor::monitor_dashboard));

    // Static file serving for SPA frontend with cache headers for assets
    let frontend_dir = std::path::Path::new(&config.frontend_dir);
    let serve_dir = if frontend_dir.exists() {
        info!("Serving frontend from: {}", frontend_dir.display());
        ServeDir::new(frontend_dir)
    } else {
        info!(
            "Frontend directory not found at: {}. Using embedded fallback.",
            frontend_dir.display()
        );
        // Fallback: serve an embedded HTML response
        ServeDir::new(".")
    };

    let serve_dir_with_cache = ServiceBuilder::new()
        .layer(axum_mw::from_fn(middleware::static_cache_headers))
        .service(serve_dir);

    // CORS: permissive during local development; restrict in production via
    // `CORS_ORIGIN` env var (comma-separated allowed origins).  When left
    // empty, falls back to permissive (Any) for ease of local testing.
    let cors_origins = std::env::var("CORS_ORIGIN").unwrap_or_default();
    let cors_layer = if cors_origins.is_empty() {
        CorsLayer::permissive()
    } else {
        CorsLayer::new()
            .allow_origin(
                cors_origins
                    .split(',')
                    .filter_map(|s| {
                        let trimmed = s.trim();
                        if trimmed.is_empty() {
                            return None;
                        }
                        match trimmed.parse() {
                            Ok(origin) => Some(origin),
                            Err(e) => {
                                tracing::warn!(
                                    "Ignoring invalid CORS_ORIGIN value {:?}: {}",
                                    trimmed,
                                    e
                                );
                                None
                            }
                        }
                    })
                    .collect::<Vec<_>>(),
            )
            .allow_methods(Any)
            .allow_headers(Any)
    };

    let app = Router::new()
        .merge(api_routes)
        .merge(monitor_router)
        .fallback_service(serve_dir_with_cache)
        // Rate limit — enforce before we do any real work.
        // Placed innermost (first .layer()) so the layers below wrap it
        // and still receive short-circuit 429 responses.
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::rate_limit::rate_limit_global,
        ))
        // Monitoring — request counting and breakdown.
        // Wraps rate_limit so 429 responses are counted.
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::track_requests,
        ))
        // Security headers — applied outside rate-limit and monitoring so
        // every response (including 429, 401, 413) carries security headers.
        .layer(axum_mw::from_fn(middleware::security_headers))
        // Body size limit (10 MB)
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024))
        // Request timeout (60 s, configurable via env).
        // Custom Layer wraps every request in tokio::time::timeout; returns
        // HTTP 408 on timeout so the error type stays Infallible.
        .layer(RequestTimeoutLayer::from_env_or(60))
        // Compression — gzip/brotli for text responses
        .layer(CompressionLayer::new())
        // CORS
        .layer(cors_layer)
        // Tracing — outermost so every request is logged
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    // Parse bind address
    let listener = tokio::net::TcpListener::bind(&config.bind_address).await?;
    info!("Server listening on http://{}", config.bind_address);
    info!("Web UI: http://{}", config.bind_address);
    info!("API: http://{}/api/health", config.bind_address);

    // Graceful shutdown.
    // Use into_make_service_with_connect_info so ConnectInfo<SocketAddr> is
    // available in request extensions for rate limiting and logging.
    let shutdown_notify = llm_shutdown_notify.clone();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal(shutdown_notify))
    .await?;

    info!("Server shut down gracefully");
    Ok(())
}

async fn shutdown_signal(llm_shutdown: Arc<tokio::sync::Notify>) {
    let ctrl_c = async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {}
            Err(e) => {
                tracing::error!("Failed to install Ctrl+C handler: {}", e);
                // Fall back to a simple pending future so the server still runs;
                // the user can kill it with SIGKILL / Task Manager.
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::error!("Failed to install SIGTERM handler: {}", e);
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            info!("Ctrl+C received, shutting down...");
        },
        () = terminate => {
            info!("SIGTERM received, shutting down...");
        },
    }

    // Signal all in-flight LLM streaming tasks to cancel gracefully.
    llm_shutdown.notify_waiters();
    info!("Cancelled in-flight LLM streaming requests");
}
