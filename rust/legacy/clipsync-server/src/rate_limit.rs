//! Per-IP rate limiter for `/inject`.
//!
//! Runs **before** [`crate::auth::auth_layer`] so an attacker spamming
//! invalid Bearer tokens is throttled before reaching auth — otherwise
//! bad-token storms would just produce a flood of 401s and the limiter
//! never fires (the original Mac bug; see commit `e2cb5451`).
//!
//! Tower layer order is bottom-up: the layer added LAST runs FIRST.
//! See `routes::build_router` for the wiring.
//!
//! Limits: 30 requests per 60 seconds, per remote IP, only for `/inject`.
//! All other paths pass through untouched. On exceedance we return
//! `429 Too Many Requests` with a `Retry-After` header (seconds until
//! the IP's window resets).

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tokio::sync::Mutex;

/// Maximum number of `/inject` requests allowed per IP within the window.
pub const INJECT_RATE_LIMIT_MAX: u32 = 30;

/// Length of the rate-limit window.
pub const INJECT_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

/// Rolling-window counter, one per remote IP.
///
/// `window_start` is the `Instant` when the current window opened.
/// `count` is the number of requests observed inside that window.
/// When a request arrives after `window_start + INJECT_RATE_LIMIT_WINDOW`,
/// the window is reset.
#[derive(Clone, Copy)]
struct WindowState {
    window_start: Instant,
    count: u32,
}

/// Shared rate-limiter state. Cheap to clone — wraps `Arc<Mutex<…>>`.
#[derive(Clone, Default)]
pub struct RateLimiter {
    inner: Arc<Mutex<HashMap<IpAddr, WindowState>>>,
}

impl RateLimiter {
    /// Build an empty limiter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Try to record one request for `ip`. Returns:
    /// - `Ok(())` if the request is within the per-IP budget
    /// - `Err(retry_after)` with the number of seconds the caller should
    ///   wait before the window resets, when the limit is exceeded
    pub async fn check(&self, ip: IpAddr) -> Result<(), u64> {
        let now = Instant::now();
        let mut map = self.inner.lock().await;

        let state = map.entry(ip).or_insert(WindowState {
            window_start: now,
            count: 0,
        });

        // Reset the window if it expired.
        if now.duration_since(state.window_start) >= INJECT_RATE_LIMIT_WINDOW {
            state.window_start = now;
            state.count = 0;
        }

        if state.count >= INJECT_RATE_LIMIT_MAX {
            // Window has not yet reset. Compute remaining seconds, ceiling
            // to at least 1 so clients don't busy-loop on 0.
            let elapsed = now.duration_since(state.window_start);
            let remaining = INJECT_RATE_LIMIT_WINDOW
                .saturating_sub(elapsed)
                .as_secs()
                .max(1);
            return Err(remaining);
        }

        state.count += 1;
        Ok(())
    }
}

/// Extract the remote IP for the current request.
///
/// Order of resolution:
/// 1. `X-Forwarded-For` header (first entry) — used in tests and behind
///    proxies. We do NOT trust this in production deployments without
///    a proxy in front, but this server is currently designed for
///    direct LAN/Tailscale exposure where there's no proxy, and tests
///    rely on this to simulate distinct clients.
/// 2. `axum::extract::ConnectInfo<SocketAddr>` request extension, set by
///    the connection acceptor in `main.rs`.
/// 3. Fallback: `0.0.0.0`. Treated as a single bucket — better to
///    over-throttle one anonymous bucket than to skip rate limiting.
fn extract_client_ip(req: &Request<Body>) -> IpAddr {
    if let Some(xff) = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(first) = xff.split(',').next() {
            if let Ok(ip) = first.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }

    if let Some(connect_info) = req
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
    {
        return connect_info.0.ip();
    }

    IpAddr::from([0, 0, 0, 0])
}

/// Axum middleware: enforce the per-IP rate limit on `/inject`.
///
/// Other paths pass through unchanged (no bookkeeping cost).
pub async fn rate_limit_layer(
    axum::extract::State(limiter): axum::extract::State<RateLimiter>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if req.uri().path() != "/inject" {
        return next.run(req).await;
    }

    let ip = extract_client_ip(&req);
    match limiter.check(ip).await {
        Ok(()) => next.run(req).await,
        Err(retry_after) => {
            let mut resp = (StatusCode::TOO_MANY_REQUESTS, "Too Many Requests").into_response();
            if let Ok(value) = HeaderValue::from_str(&retry_after.to_string()) {
                resp.headers_mut().insert("Retry-After", value);
            }
            resp
        }
    }
}
