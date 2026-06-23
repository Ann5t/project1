//! Sliding-window rate limiter middleware.
//!
//! Tracks per-IP request counts using a 60-second sliding window stored in a
//! `HashMap<String, Vec<Instant>>` behind a `tokio::sync::RwLock`. Old entries
//! are pruned on every check (lazy cleanup).
//!
//! Three limit tiers are enforced:
//!
//! | Middleware            | Scope            | Default limit          |
//! |-----------------------|------------------|------------------------|
//! | `rate_limit_global`   | All routes       | 100 requests/min per IP|
//! | `rate_limit_chat`     | `/api/chat*`     | 10 requests/min per IP |
//! | (within chat, stream) | SSE concurrency  | 5 concurrent per IP    |
//!
//! When a limit is exceeded the middleware returns HTTP 429 with a JSON body
//! `{"error":"Rate limited","retry_after":N}` and a `Retry-After` header.
//! Successful responses include `X-RateLimit-Limit`, `X-RateLimit-Remaining`,
//! and `X-RateLimit-Reset` headers.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use tokio::sync::RwLock;

use crate::state::AppState;

// ── RateLimiter ────────────────────────────────────────────────────────────

/// Shared sliding-window rate limiter stored in [`AppState`].
///
/// All fields are behind `Arc` so the struct is cheap to clone while
/// sharing the same backing data.
#[derive(Clone)]
pub struct RateLimiter {
    /// Per-IP request timestamps for the global (all-routes) bucket.
    global_buckets: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
    /// Per-IP request timestamps for the chat-specific bucket.
    chat_buckets: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
    /// Per-IP active SSE connection count (concurrent, not rate).
    sse_active: Arc<RwLock<HashMap<String, u32>>>,
    /// Global requests-per-minute limit.
    pub global_rpm: u32,
    /// Chat requests-per-minute limit.
    pub chat_rpm: u32,
    /// Max concurrent SSE connections per IP.
    pub sse_max_concurrent: u32,
    /// Master on/off switch.
    pub enabled: bool,
    /// Sliding-window duration (60 seconds).
    window: Duration,
}

impl RateLimiter {
    /// Create a new rate limiter with the given limits.
    pub fn new(global_rpm: u32, chat_rpm: u32, sse_max_concurrent: u32, enabled: bool) -> Self {
        Self {
            global_buckets: Arc::new(RwLock::new(HashMap::new())),
            chat_buckets: Arc::new(RwLock::new(HashMap::new())),
            sse_active: Arc::new(RwLock::new(HashMap::new())),
            global_rpm,
            chat_rpm,
            sse_max_concurrent,
            enabled,
            window: Duration::from_secs(60),
        }
    }

    // ── Global limit ───────────────────────────────────────────────────

    /// Check whether `ip` is allowed under the global RPM limit.
    ///
    /// Returns `Ok((remaining, reset_secs))` when allowed, or
    /// `Err((retry_after_secs, limit))` when the limit is exceeded.
    pub async fn check_global(&self, ip: &str) -> Result<(u32, u64), (u64, u32)> {
        self.check_bucket(&self.global_buckets, ip, self.global_rpm)
            .await
    }

    // ── Chat limit ─────────────────────────────────────────────────────

    /// Check whether `ip` is allowed under the chat RPM limit.
    ///
    /// Same return convention as [`check_global`].
    pub async fn check_chat(&self, ip: &str) -> Result<(u32, u64), (u64, u32)> {
        self.check_bucket(&self.chat_buckets, ip, self.chat_rpm).await
    }

    // ── SSE concurrency ────────────────────────────────────────────────

    /// Check whether `ip` can open another SSE stream.
    ///
    /// Returns `Ok(remaining)` when a slot is available, or
    /// `Err(limit)` when the per-IP concurrent limit is reached.
    pub async fn check_sse(&self, ip: &str) -> Result<u32, u32> {
        let active = self.sse_active.read().await;
        let count = active.get(ip).copied().unwrap_or(0);
        if count >= self.sse_max_concurrent {
            Err(self.sse_max_concurrent)
        } else {
            Ok(self.sse_max_concurrent.saturating_sub(count))
        }
    }

    /// Register a new SSE connection for `ip` (increment the counter).
    pub async fn sse_connected(&self, ip: &str) {
        let mut active = self.sse_active.write().await;
        *active.entry(ip.to_string()).or_insert(0) += 1;
    }

    /// Unregister an SSE connection for `ip` (decrement the counter).
    /// Removes the entry when the count reaches zero.
    pub async fn sse_disconnected(&self, ip: &str) {
        let mut active = self.sse_active.write().await;
        if let Some(count) = active.get_mut(ip) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                active.remove(ip);
            }
        }
    }

    // ── Internal helpers ───────────────────────────────────────────────

    /// Core sliding-window check shared by `check_global` and `check_chat`.
    ///
    /// 1. Acquires a write lock on the bucket map.
    /// 2. Prunes timestamps older than `window`.
    /// 3. If the remaining count reaches `limit`, records this request and
    ///    returns `Ok((remaining, reset_secs))`.
    /// 4. Otherwise returns `Err((retry_after_secs, limit))`.
    async fn check_bucket(
        &self,
        buckets: &RwLock<HashMap<String, Vec<Instant>>>,
        ip: &str,
        limit: u32,
    ) -> Result<(u32, u64), (u64, u32)> {
        let now = Instant::now();
        let cutoff = now - self.window;

        let mut map = buckets.write().await;
        let entry = map.entry(ip.to_string()).or_default();

        // Slide the window — drop timestamps older than 60 s.
        entry.retain(|t| *t > cutoff);

        if entry.len() >= limit as usize {
            // Compute retry-after: when the oldest entry expires.
            let oldest = entry.first().copied().unwrap_or(now);
            let expires_at = oldest + self.window;
            let retry_after = expires_at
                .duration_since(now)
                .as_secs()
                .max(1);
            Err((retry_after, limit))
        } else {
            entry.push(now);
            let remaining = limit.saturating_sub(entry.len() as u32);
            // Reset: seconds until the oldest entry in this window falls off.
            let reset = entry
                .first()
                .map(|oldest| (*oldest + self.window).duration_since(now).as_secs())
                .unwrap_or(self.window.as_secs());
            Ok((remaining, reset))
        }
    }
}

// ── Middleware entry points ────────────────────────────────────────────────

/// Global rate limiter applied to all routes.
///
/// Enforces `rate_limit_global_rpm` per IP with a 60-second sliding window.
/// When rate limiting is disabled (`rate_limit_enabled = false`) this is a
/// no-op.
pub async fn rate_limit_global(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if !state.rate_limiter.enabled {
        return next.run(request).await;
    }

    let ip = extract_client_ip(&request);

    match state.rate_limiter.check_global(&ip).await {
        Ok((remaining, reset)) => {
            let limit = state.rate_limiter.global_rpm;
            let mut response = next.run(request).await;
            add_rate_limit_headers(&mut response, limit, remaining, reset);
            response
        }
        Err((retry_after, limit)) => {
            state.increment_rate_limit();
            build_429_response(retry_after, limit)
        }
    }
}

/// Chat-specific rate limiter applied to `/api/chat` and `/api/chat/stream`.
///
/// Enforces two constraints:
/// 1. Chat RPM (`rate_limit_chat_rpm`) per IP.
/// 2. For the `/stream` variant, a concurrent SSE connection cap
///    (`sse_max_concurrent`) per IP.
///
/// The SSE counter is incremented before the handler runs and decremented
/// after the response completes (which, for SSE streams, means after the
/// client disconnects).
pub async fn rate_limit_chat(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if !state.rate_limiter.enabled {
        return next.run(request).await;
    }

    let ip = extract_client_ip(&request);
    let is_stream = request.uri().path().ends_with("/stream");

    // Step 1 — Check chat RPM (fail-fast, no cleanup needed).
    let (remaining, reset) = match state.rate_limiter.check_chat(&ip).await {
        Ok(v) => v,
        Err((retry_after, limit)) => {
            state.increment_rate_limit();
            return build_429_response(retry_after, limit);
        }
    };

    // Step 2 — For stream endpoints, check SSE concurrency.
    if is_stream {
        match state.rate_limiter.check_sse(&ip).await {
            Ok(_) => {
                state.rate_limiter.sse_connected(&ip).await;
            }
            Err(limit) => {
                state.increment_rate_limit();
                return build_429_response(1, limit);
            }
        }
    }

    // Step 3 — Run the handler.
    let mut response = next.run(request).await;

    // Step 4 — Cleanup SSE counter (no-op for non-stream endpoints).
    if is_stream {
        state.rate_limiter.sse_disconnected(&ip).await;
    }

    let limit = state.rate_limiter.chat_rpm;
    add_rate_limit_headers(&mut response, limit, remaining, reset);
    response
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Extract the client IP address from the request.
///
/// Checks `X-Forwarded-For` first (taking the leftmost address when behind a
/// proxy), then falls back to the socket peer address via `ConnectInfo`.
/// Returns `"unknown"` only when neither source is available (e.g. Unix
/// domain sockets).
fn extract_client_ip(request: &Request) -> String {
    // 1. X-Forwarded-For header (common when behind nginx / reverse proxy)
    if let Some(xff) = request.headers().get("x-forwarded-for") {
        if let Ok(val) = xff.to_str() {
            // The leftmost address is the original client.
            if let Some(addr) = val.split(',').next() {
                let trimmed = addr.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }

    // 2. Socket peer address via ConnectInfo extension (set by
    //    `into_make_service_with_connect_info`).
    if let Some(connect_info) = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
    {
        return connect_info.0.ip().to_string();
    }

    // 3. Last resort — no identifying information available.
    "unknown".to_string()
}

/// Build an HTTP 429 (Too Many Requests) response with `Retry-After` header
/// and a JSON error body.
fn build_429_response(retry_after: u64, limit: u32) -> Response {
    let body = serde_json::json!({
        "error": "Rate limited",
        "retry_after": retry_after,
    });

    let mut response = Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header("content-type", "application/json")
        .header("Retry-After", retry_after.to_string())
        .body(Body::from(serde_json::to_vec(&body).unwrap_or_default()))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .expect("Failed to build rate limit error response")
        });

    add_rate_limit_headers(&mut response, limit, 0, retry_after);
    response
}

/// Add `X-RateLimit-*` informational headers to an existing response.
fn add_rate_limit_headers(response: &mut Response, limit: u32, remaining: u32, reset: u64) {
    let headers = response.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&limit.to_string()) {
        headers.insert("X-RateLimit-Limit", v);
    }
    if let Ok(v) = HeaderValue::from_str(&remaining.to_string()) {
        headers.insert("X-RateLimit-Remaining", v);
    }
    if let Ok(v) = HeaderValue::from_str(&reset.to_string()) {
        headers.insert("X-RateLimit-Reset", v);
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build an HTTP request with optional X-Forwarded-For header.
    fn make_request(xff: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder().uri("https://example.com/api/test");
        if let Some(val) = xff {
            builder = builder.header("x-forwarded-for", val);
        }
        builder.body(Body::empty()).unwrap()
    }

    // ── X-Forwarded-For extraction ──

    #[tokio::test]
    async fn extract_client_ip_single_xff() {
        let req = make_request(Some("203.0.113.42"));
        let ip = extract_client_ip(&req);
        assert_eq!(ip, "203.0.113.42");
    }

    #[tokio::test]
    async fn extract_client_ip_multiple_xff_takes_leftmost() {
        // When behind multiple proxies: client, proxy1, proxy2
        let req = make_request(Some("10.0.0.1, 172.16.0.2, 192.168.0.3"));
        let ip = extract_client_ip(&req);
        assert_eq!(ip, "10.0.0.1", "Should extract the leftmost (original client) IP");
    }

    #[tokio::test]
    async fn extract_client_ip_multiple_xff_with_whitespace() {
        let req = make_request(Some("  192.168.1.100  , 10.0.0.5 , 172.16.0.1 "));
        let ip = extract_client_ip(&req);
        assert_eq!(ip, "192.168.1.100");
    }

    #[tokio::test]
    async fn extract_client_ip_no_header_returns_unknown() {
        let req = make_request(None);
        let ip = extract_client_ip(&req);
        assert_eq!(ip, "unknown");
    }

    #[tokio::test]
    async fn extract_client_ip_empty_xff_returns_unknown() {
        let req = make_request(Some(""));
        let ip = extract_client_ip(&req);
        assert_eq!(ip, "unknown");
    }

    #[tokio::test]
    async fn extract_client_ip_ipv6_address() {
        let req = make_request(Some("2001:db8::1"));
        let ip = extract_client_ip(&req);
        assert_eq!(ip, "2001:db8::1");
    }

    #[tokio::test]
    async fn extract_client_ip_multiple_with_ipv6() {
        let req = make_request(Some("2001:db8::1, 10.0.0.1, 192.168.1.1"));
        let ip = extract_client_ip(&req);
        assert_eq!(ip, "2001:db8::1");
    }

    // ── Rate limiter sliding window ──

    #[tokio::test]
    async fn rate_limiter_allows_under_limit() {
        let rl = RateLimiter::new(5, 2, 2, true);
        for _ in 0..5 {
            let result = rl.check_global("test-ip-1").await;
            assert!(result.is_ok(), "Should allow {} requests under the limit", 5);
        }
    }

    #[tokio::test]
    async fn rate_limiter_blocks_over_limit() {
        let rl = RateLimiter::new(3, 2, 2, true);
        // Exhaust the limit
        for _ in 0..3 {
            let _ = rl.check_global("test-ip-2").await;
        }
        // Next request should be blocked
        let result = rl.check_global("test-ip-2").await;
        assert!(result.is_err(), "4th request should be rate limited");
        if let Err((retry_after, limit)) = result {
            assert_eq!(limit, 3);
            assert!(retry_after > 0, "retry_after should be positive");
        }
    }

    #[tokio::test]
    async fn rate_limiter_burst_at_boundary() {
        let rl = RateLimiter::new(10, 5, 2, true);
        let ip = "burst-ip";

        // Send exactly limit requests
        for _ in 0..10 {
            let result = rl.check_global(ip).await;
            assert!(result.is_ok(), "Request under limit should succeed");
        }

        // The very next request (boundary) should be blocked
        let result = rl.check_global(ip).await;
        assert!(result.is_err(), "Request exactly at boundary should be rate limited");
    }

    #[tokio::test]
    async fn rate_limiter_different_ips_independent() {
        let rl = RateLimiter::new(2, 1, 1, true);

        // Exhaust IP 1
        let _ = rl.check_global("ip-a").await;
        let _ = rl.check_global("ip-a").await;
        assert!(rl.check_global("ip-a").await.is_err());

        // IP 2 should still be allowed
        let result = rl.check_global("ip-b").await;
        assert!(result.is_ok(), "Different IP should have independent limits");
    }

    #[tokio::test]
    async fn rate_limiter_remaining_count_decreases() {
        let rl = RateLimiter::new(5, 2, 2, true);
        let ip = "remaining-test";

        let (rem1, _) = rl.check_global(ip).await.unwrap();
        assert_eq!(rem1, 4, "After 1 request, 4 remaining");
        let (rem2, _) = rl.check_global(ip).await.unwrap();
        assert_eq!(rem2, 3, "After 2 requests, 3 remaining");
        let (rem3, _) = rl.check_global(ip).await.unwrap();
        assert_eq!(rem3, 2, "After 3 requests, 2 remaining");
    }

    #[tokio::test]
    async fn rate_limiter_returns_reset_time() {
        let rl = RateLimiter::new(10, 5, 2, true);
        let ip = "reset-test";

        let (_, reset) = rl.check_global(ip).await.unwrap();
        assert!(reset > 0, "Reset time should be positive");
        assert!(reset <= 60, "Reset time should be within the window (60s)");
    }

    #[tokio::test]
    async fn rate_limiter_retry_after_is_at_least_one_second() {
        let rl = RateLimiter::new(1, 1, 1, true);
        let ip = "retry-test";

        let _ = rl.check_global(ip).await.unwrap(); // Use the single slot
        let result = rl.check_global(ip).await;
        assert!(result.is_err());
        if let Err((retry_after, _)) = result {
            assert!(retry_after >= 1, "retry_after should be at least 1 second, got {}", retry_after);
        }
    }

    #[tokio::test]
    async fn rate_limiter_disabled_flag_respected() {
        let rl = RateLimiter::new(1, 1, 1, false); // disabled
        assert!(!rl.enabled, "Enabled flag should be false when disabled");

        let rl_enabled = RateLimiter::new(1, 1, 1, true); // enabled
        assert!(rl_enabled.enabled, "Enabled flag should be true when enabled");
    }

    #[tokio::test]
    async fn rate_limiter_chat_limit_separate_from_global() {
        let rl = RateLimiter::new(100, 3, 5, true);
        let ip = "chat-separate";

        // Exhaust chat limit
        for _ in 0..3 {
            assert!(rl.check_chat(ip).await.is_ok());
        }
        assert!(rl.check_chat(ip).await.is_err(), "Chat limit should be exhausted");

        // Global should still have capacity
        let result = rl.check_global(ip).await;
        assert!(result.is_ok(), "Global limit should be independent of chat limit");
    }

    #[tokio::test]
    async fn sse_concurrency_tracking() {
        let rl = RateLimiter::new(100, 10, 3, true);
        let ip = "sse-test";

        assert!(rl.check_sse(ip).await.is_ok());
        rl.sse_connected(ip).await;
        assert!(rl.check_sse(ip).await.is_ok());
        rl.sse_connected(ip).await;
        assert!(rl.check_sse(ip).await.is_ok());
        rl.sse_connected(ip).await;

        // Now at limit (3)
        assert!(rl.check_sse(ip).await.is_err(), "4th SSE should be blocked");

        // Disconnect one
        rl.sse_disconnected(ip).await;
        assert!(rl.check_sse(ip).await.is_ok(), "After disconnect, slot should be free");
    }

    #[tokio::test]
    async fn sse_disconnect_removes_entry() {
        let rl = RateLimiter::new(100, 10, 3, true);
        let ip = "sse-cleanup";

        rl.sse_connected(ip).await;
        rl.sse_disconnected(ip).await;

        // After disconnect, should be back to 0, and next connect should work
        rl.sse_connected(ip).await;
        rl.sse_connected(ip).await;
        rl.sse_connected(ip).await;

        // All 3 slots should be used
        assert!(rl.check_sse(ip).await.is_err());
    }

    // ── Build 429 response tests ──

    #[test]
    fn build_429_response_has_correct_status() {
        let response = build_429_response(30, 100);
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn build_429_response_has_retry_after_header() {
        let response = build_429_response(45, 50);
        let retry_after = response.headers().get("Retry-After").unwrap();
        assert_eq!(retry_after.to_str().unwrap(), "45");
    }

    #[test]
    fn build_429_response_has_rate_limit_headers() {
        let response = build_429_response(15, 20);
        assert!(response.headers().get("X-RateLimit-Limit").is_some());
        assert!(response.headers().get("X-RateLimit-Remaining").is_some());
        assert!(response.headers().get("X-RateLimit-Reset").is_some());
    }

    // ── add_rate_limit_headers tests ──

    #[tokio::test]
    async fn rate_limit_headers_added_to_response() {
        let mut response = Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())
            .unwrap();

        add_rate_limit_headers(&mut response, 100, 42, 58);

        let headers = response.headers();
        assert_eq!(headers.get("X-RateLimit-Limit").unwrap().to_str().unwrap(), "100");
        assert_eq!(headers.get("X-RateLimit-Remaining").unwrap().to_str().unwrap(), "42");
        assert_eq!(headers.get("X-RateLimit-Reset").unwrap().to_str().unwrap(), "58");
    }
}
