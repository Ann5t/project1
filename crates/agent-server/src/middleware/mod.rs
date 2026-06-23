//! HTTP middleware for monitoring (request counting), rate limiting, and authentication.
//!
//! [`track_requests`] increments the global request counter on every HTTP request.
//! [`authenticate`] checks the `Authorization: Bearer <token>` header (or
//! `?token=<token>` query parameter for WebSocket upgrades) against the stored
//! `admin_token` when `auth_enabled` is true. Public paths like `/api/health`,
//! channel callbacks, and `/p/*` published pages are excluded.
//! [`rate_limit_global`] and [`rate_limit_chat`] enforce per-IP sliding-window
//! rate limits using configurable thresholds.
//! [`static_cache_headers`] adds Cache-Control headers for static asset paths
//! (/css/*, /js/*, /assets/*) to enable browser caching.

pub mod rate_limit;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;

use crate::state::AppState;

/// Middleware that increments the request counter on every HTTP request,
/// and records a breakdown by (method, path, status) for Prometheus metrics.
pub async fn track_requests(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    state.increment_request_count();

    // Capture method and path before the request is consumed.
    let method = request.method().to_string();
    let path = request.uri().path().to_string();

    let response = next.run(request).await;

    let status = response.status().as_u16();
    state
        .increment_request_breakdown(&method, &path, status)
        .await;

    response
}

/// Authentication middleware.
///
/// When `auth_enabled` is `true` in the application state, this middleware
/// requires a valid Bearer token for every request that is not on a public
/// path.  If auth is disabled the middleware is a no-op.
///
/// # Token extraction order
///
/// 1. `Authorization: Bearer <token>` header (preferred for REST calls)
/// 2. `?token=<token>` query parameter (fallback, useful for WebSocket /
///    EventSource connections)
pub async fn authenticate(State(state): State<AppState>, request: Request, next: Next) -> Response {
    // If auth is not enabled, skip entirely
    if !state.is_auth_enabled() {
        return next.run(request).await;
    }

    // Public paths that never require authentication
    if is_public_path(request.uri().path()) {
        return next.run(request).await;
    }

    // Extract the token from the request
    let token = extract_token(&request);
    let admin_token = state.get_admin_token().await;

    match token {
        Some(t) if t == admin_token => next.run(request).await,
        _ => {
            let body = serde_json::json!({
                "error": "Unauthorized — invalid or missing authentication token"
            });
            Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap_or_default()))
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .expect("Failed to build auth error response")
                })
        }
    }
}

/// Returns `true` when the given path does not require authentication.
fn is_public_path(path: &str) -> bool {
    path == "/api/health"
        || path == "/api/info"
        || path == "/api/metrics"
        || path == "/api/auth/status"
        || path == "/api/auth/login"
        || path.starts_with("/p/")
        || path == "/api/channels/feishu/callback"
        || path == "/api/channels/wechat_work/callback"
        || path.starts_with("/api/channels/webhook/")
        || path == "/api/openapi.json"
        || path == "/api/docs"
}

/// Extract the bearer token from the request.
///
/// Checks the `Authorization` header first, then falls back to the `token`
/// query parameter (useful for WebSocket / EventSource connections where
/// custom headers cannot be set).
fn extract_token(request: &Request) -> Option<String> {
    // 1. Authorization: Bearer <token> header
    if let Some(auth) = request.headers().get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.trim().to_string());
            }
        }
    }

    // 2. ?token=<token> query parameter
    if let Some(query) = request.uri().query() {
        for pair in query.split('&') {
            let mut parts = pair.splitn(2, '=');
            if let Some("token") = parts.next() {
                return parts.next().map(|v| v.to_string());
            }
        }
    }

    None
}

/// Middleware that adds `Cache-Control: public, max-age=3600` to responses
/// for static asset paths (/css/*, /js/*, /assets/*).
///
/// Apply this as a layer on the static file fallback service so browser
/// caching reduces repeated downloads of unchanged assets.
pub async fn static_cache_headers(request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let should_cache =
        path.starts_with("/css/") || path.starts_with("/js/") || path.starts_with("/assets/");

    let mut response = next.run(request).await;

    if should_cache {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static("cache-control"),
            axum::http::HeaderValue::from_static("public, max-age=3600"),
        );
    }

    response
}

/// Middleware that injects security-related HTTP response headers.
///
/// Applies a baseline set of OWASP-recommended headers to every response:
///
/// | Header | Value | Purpose |
/// |--------|-------|---------|
/// | `X-Content-Type-Options` | `nosniff` | Prevent MIME-type sniffing |
/// | `X-Frame-Options` | `DENY` | Prevent clickjacking |
/// | `Referrer-Policy` | `strict-origin-when-cross-origin` | Limit referrer info |
/// | `X-XSS-Protection` | `1; mode=block` | Legacy anti-XSS for older browsers |
/// | `X-DNS-Prefetch-Control` | `off` | Disable DNS prefetch for privacy |
/// | `Content-Security-Policy` | (see below) | Restrict resource loading |
///
/// The CSP allows same-origin resources and is suitable for a local SPA.
/// Tune `CSP_POLICY` env var in production if you embed external content
/// (maps, analytics, CDN-hosted assets, etc.).
///
/// `Strict-Transport-Security` is omitted because this server may run
/// without HTTPS (behind a reverse proxy). Add HSTS at the reverse-proxy
/// level instead.
///
/// Apply as a router-level layer so every response receives these headers.
pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        axum::http::HeaderName::from_static("x-content-type-options"),
        axum::http::HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        axum::http::HeaderName::from_static("x-frame-options"),
        axum::http::HeaderValue::from_static("DENY"),
    );
    headers.insert(
        axum::http::HeaderName::from_static("referrer-policy"),
        axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        axum::http::HeaderName::from_static("x-xss-protection"),
        axum::http::HeaderValue::from_static("1; mode=block"),
    );
    headers.insert(
        axum::http::HeaderName::from_static("x-dns-prefetch-control"),
        axum::http::HeaderValue::from_static("off"),
    );

    // Content-Security-Policy: restrict resource loading to same-origin.
    // Override via CSP_POLICY env var when the SPA loads external resources.
    let csp = std::sync::LazyLock::new(|| {
        std::env::var("CSP_POLICY").unwrap_or_else(|_| {
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
             img-src 'self' data:; font-src 'self'; connect-src 'self'"
                .to_string()
        })
    });
    if let Ok(value) = axum::http::HeaderValue::from_str(&csp) {
        headers.insert(
            axum::http::HeaderName::from_static("content-security-policy"),
            value,
        );
    }

    response
}
