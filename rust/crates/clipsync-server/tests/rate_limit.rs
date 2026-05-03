//! Rate-limit tests for `/inject` (Phase 1.4 — port of mac e2cb5451).
//!
//! The rate limiter runs **before** the auth middleware, so an attacker
//! spamming bad Bearer tokens hits 401 for the first `INJECT_RATE_LIMIT_MAX`
//! requests within the 60-second window — the next one hits 429.
//!
//! These tests exercise:
//!   1. Throttling kicks in **at** request 31 with an invalid token,
//!      returning `429 Too Many Requests` (not 401) and a `Retry-After`
//!      header (Phase 1.4 success criterion #1).
//!   2. Buckets are per-IP — two distinct simulated clients each get
//!      their own 30-request budget (criterion #2). We use
//!      `X-Forwarded-For` to differentiate clients in the test harness;
//!      the same header is honored by the production middleware (see
//!      `extract_client_ip`).

use std::net::IpAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use clipsync_core::pairing::PairingManager;
use clipsync_core::tls::TlsIdentity;
use clipsync_core::token_store::TokenStore;
use tokio::sync::RwLock;

use clipsync_server::rate_limit::{RateLimiter, INJECT_RATE_LIMIT_MAX};
use clipsync_server::routes::build_router_with_limiter;
use clipsync_server::ws_hub::WsHub;
use clipsync_server::AppState;

// ─── helpers ─────────────────────────────────────────────────────

fn test_state() -> Arc<AppState> {
    let data_dir =
        std::env::temp_dir().join(format!("clipsync-ratelimit-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&data_dir).unwrap();

    let hostnames = vec!["localhost".to_string()];
    let ips: Vec<IpAddr> = vec!["127.0.0.1".parse().unwrap()];
    let tls_identity = TlsIdentity::generate(&hostnames, &ips).unwrap();

    Arc::new(AppState {
        token_store: RwLock::new(TokenStore::new()),
        pairing_manager: RwLock::new(PairingManager::new()),
        ws_hub: WsHub::new(),
        tls_identity,
        data_dir,
    })
}

/// Build a `/inject` request that will fail auth (no valid token), so we
/// can drive the rate limiter independently of the auth middleware.
fn make_unauth_inject(client_ip: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .header("Authorization", "Bearer invalid-token")
        .header("X-ClipSync-Signature", "t=0,sig=00")
        .header("X-Forwarded-For", client_ip)
        .body(Body::from(b"{}".to_vec()))
        .unwrap()
}

// ─── criterion #1: 31st request returns 429 (not 401) ────────────

/// Send `INJECT_RATE_LIMIT_MAX + 1` unauthenticated requests inside the
/// window. The first `INJECT_RATE_LIMIT_MAX` must return `401`, the
/// next must return `429` with a `Retry-After` header.
#[tokio::test]
async fn inject_31st_request_is_throttled_to_429() {
    let state = test_state();
    let limiter = RateLimiter::new();

    // Verify the constant is what the plan promises (30/min).
    assert_eq!(INJECT_RATE_LIMIT_MAX, 30);

    let mut last_status = None;
    for i in 1..=INJECT_RATE_LIMIT_MAX {
        let app = build_router_with_limiter(state.clone(), limiter.clone());
        let resp = app.oneshot(make_unauth_inject("10.0.0.1")).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "request #{i} expected 401 (rate-limit not yet hit), got {}",
            resp.status()
        );
        last_status = Some(resp.status());
    }
    assert_eq!(last_status, Some(StatusCode::UNAUTHORIZED));

    // The 31st request must hit the limiter, not auth.
    let app = build_router_with_limiter(state, limiter);
    let resp = app.oneshot(make_unauth_inject("10.0.0.1")).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "request #{} must be 429 (limit pre-auth), got {}",
        INJECT_RATE_LIMIT_MAX + 1,
        resp.status()
    );

    let retry_after = resp
        .headers()
        .get("Retry-After")
        .expect("429 must include Retry-After header")
        .to_str()
        .expect("Retry-After must be ASCII");
    let secs: u64 = retry_after
        .parse()
        .expect("Retry-After must be a numeric seconds value");
    assert!(
        (1..=60).contains(&secs),
        "Retry-After should be within (0, 60] seconds, got {secs}"
    );
}

// ─── criterion #2: per-IP buckets ────────────────────────────────

/// Two distinct client IPs each consume their own quota — one IP being
/// throttled does NOT throttle the other.
#[tokio::test]
async fn rate_limit_is_per_ip() {
    let state = test_state();
    let limiter = RateLimiter::new();

    // Exhaust IP A's quota.
    for _ in 0..INJECT_RATE_LIMIT_MAX {
        let app = build_router_with_limiter(state.clone(), limiter.clone());
        let resp = app.oneshot(make_unauth_inject("10.0.0.1")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // IP A's next request is throttled.
    {
        let app = build_router_with_limiter(state.clone(), limiter.clone());
        let resp = app.oneshot(make_unauth_inject("10.0.0.1")).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "IP A should be throttled after exhausting its quota"
        );
    }

    // IP B is a fresh bucket — its first request must NOT be throttled.
    // It still fails auth (401) because the token is invalid, but the
    // important assertion is that it is not 429.
    let app = build_router_with_limiter(state, limiter);
    let resp = app.oneshot(make_unauth_inject("10.0.0.2")).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "IP B must reach auth (401), not be throttled (429) — got {}",
        resp.status()
    );
}

// ─── non-/inject paths are not affected ──────────────────────────

#[tokio::test]
async fn other_paths_are_not_rate_limited() {
    let state = test_state();
    let limiter = RateLimiter::new();

    // Hit /health far more than the limit — none should 429.
    for _ in 0..(INJECT_RATE_LIMIT_MAX + 5) {
        let app = build_router_with_limiter(state.clone(), limiter.clone());
        let req = Request::builder()
            .uri("/health")
            .header("X-Forwarded-For", "10.0.0.1")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "/health must never be rate limited"
        );
    }
}
