//! Integration tests for `/pair` error body shape.
//!
//! Phase 1.5 standardizes 401 responses on `/pair` so they carry the
//! body `{"error": "<code>"}` where `<code>` is one of:
//!
//! * `invalid` — wrong code
//! * `expired` — TTL elapsed
//! * `consumed` — already used
//! * `notStarted` — pairing not initiated
//!
//! These codes mirror the Mac Swift `PairingError` case names so cross-
//! platform clients (Android `PairingApi.kt`, future Tauri client) can
//! parse responses identically regardless of which server implementation
//! they talk to.
//!
//! NOTE: the body shape is intentionally `{error}` — there is **no**
//! `message` field on `/pair` 401s. `/inject` uses `{error, message}`
//! (see `errors.rs`); the two shapes are kept separate on purpose.

use std::net::IpAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use clipsync_core::pairing::PairingManager;
use clipsync_core::tls::TlsIdentity;
use clipsync_core::token_store::TokenStore;
use tokio::sync::RwLock;

use clipsync_server::routes::build_router;
use clipsync_server::ws_hub::WsHub;
use clipsync_server::AppState;

/// Mirror of `tests/server_tests.rs::test_state` — kept here so this
/// integration test file is self-contained.
fn test_state() -> Arc<AppState> {
    let data_dir = std::env::temp_dir().join(format!("clipsync-test-{}", uuid::Uuid::new_v4()));
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

/// Drive `GET /pair?code=...` and decode the response body as JSON.
async fn pair_request(state: Arc<AppState>, code: &str) -> (StatusCode, serde_json::Value) {
    let app = build_router(state);
    let req = Request::builder()
        .uri(format!("/pair?code={code}"))
        .header("X-ClipSync-Device", "test-device")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body)
        .unwrap_or_else(|e| panic!("body was not JSON ({e}): {body:?}"));
    (status, json)
}

/// Wrong code while a code is active -> 401 `{"error":"invalid"}`.
#[tokio::test]
async fn pairing_invalid_code_returns_401_invalid() {
    let state = test_state();

    let code = {
        let mut pm = state.pairing_manager.write().await;
        pm.generate_code().to_string()
    };
    let wrong = if code == "000000" { "111111" } else { "000000" };

    let (status, json) = pair_request(state, wrong).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"], "invalid");
    assert!(
        json.get("message").is_none(),
        "/pair 401 must not include a `message` field"
    );
}

/// Expired code (TTL elapsed) -> 401 `{"error":"expired"}`.
///
/// We use the `pre_expire_for_tests` seam on `PairingManager` to force
/// the active code into an expired state without waiting the real TTL
/// (5 minutes).
#[tokio::test]
async fn pairing_expired_code_returns_401_expired() {
    let state = test_state();

    let code = {
        let mut pm = state.pairing_manager.write().await;
        let c = pm.generate_code().to_string();
        pm.pre_expire_for_tests();
        c
    };

    // The submitted value must match the active code: we want to assert
    // the manager surfaces `expired` *before* checking the code value
    // (matching the Mac order of checks: expired -> consumed -> match).
    let (status, json) = pair_request(state, &code).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"], "expired");
    assert!(json.get("message").is_none());
}

/// Already-consumed code -> 401 `{"error":"consumed"}`.
#[tokio::test]
async fn pairing_consumed_code_returns_401_consumed() {
    let state = test_state();

    let code = {
        let mut pm = state.pairing_manager.write().await;
        pm.generate_code().to_string()
    };

    // First call consumes the code.
    let (status, _) = pair_request(state.clone(), &code).await;
    assert_eq!(status, StatusCode::OK);

    // Second call with the same code -> consumed.
    let (status, json) = pair_request(state, &code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"], "consumed");
    assert!(json.get("message").is_none());
}

/// No code generated yet -> 401 `{"error":"notStarted"}`.
#[tokio::test]
async fn pairing_not_started_returns_401_not_started() {
    let state = test_state();

    let (status, json) = pair_request(state, "000000").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"], "notStarted");
    assert!(json.get("message").is_none());
}
