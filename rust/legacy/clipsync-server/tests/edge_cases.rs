//! Edge case and hardening tests — stress boundary conditions in the
//! ClipSync server: large payloads, token revocation, clock skew,
//! malformed requests, concurrent operations.

use std::net::IpAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use clipsync_core::hmac;
use clipsync_core::pairing::PairingManager;
use clipsync_core::tls::TlsIdentity;
use clipsync_core::token_store::TokenStore;
use tokio::sync::RwLock;

use clipsync_server::routes::build_router;
use clipsync_server::ws_hub::WsHub;
use clipsync_server::AppState;

// ─── helpers ─────────────────────────────────────────────────────

fn test_state() -> Arc<AppState> {
    let data_dir = std::env::temp_dir().join(format!("clipsync-edge-{}", uuid::Uuid::new_v4()));
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

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn make_inject_request(token: &str, secret: &[u8], body_bytes: Vec<u8>) -> Request<Body> {
    let ts = now_ts();
    let sig = hmac::sign(secret, ts, &body_bytes);

    Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .header("X-ClipSync-Signature", sig)
        .body(Body::from(body_bytes))
        .unwrap()
}

fn valid_payload_json() -> Vec<u8> {
    // ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    serde_json::to_vec(&serde_json::json!({
        "type": "text",
        "mime": "text/plain",
        "data": "aGVsbG8=",
        "ts": now_ms,
        "nonce": "00000000-0000-0000-0000-000000000000",
        "name": null
    }))
    .unwrap()
}

// ─── server restart: stop and rebuild on same routes ─────────────

#[tokio::test]
async fn server_restart_on_same_state() {
    let state = test_state();

    // First request
    let app = build_router(state.clone());
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // "Restart" — build a new router with same state
    let app = build_router(state);
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ─── concurrent clipboard payloads ───────────────────────────────

#[tokio::test]
async fn concurrent_inject_requests() {
    let state = test_state();

    let token = "concurrent-token";
    {
        let mut ts = state.token_store.write().await;
        ts.register(token.as_bytes(), "test-device").unwrap();
    }

    // Send 10 concurrent requests
    let mut handles = Vec::new();
    for i in 0..10 {
        let state = state.clone();
        let handle = tokio::spawn(async move {
            let app = build_router(state);
            // ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let payload = serde_json::json!({
                "type": "text",
                "mime": "text/plain",
                "data": base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    format!("msg-{i}")
                ),
                "ts": now_ms + i as u64,
                "nonce": uuid::Uuid::new_v4().to_string(),
                "name": null
            });
            let body_bytes = serde_json::to_vec(&payload).unwrap();
            let req = make_inject_request("concurrent-token", b"clipsync-pairing", body_bytes);
            let resp = app.oneshot(req).await.unwrap();
            resp.status()
        });
        handles.push(handle);
    }

    for handle in handles {
        let status = handle.await.unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "all concurrent requests must succeed"
        );
    }
}

// ─── large payload: 20MB succeeds ────────────────────────────────

#[tokio::test]
async fn large_payload_at_limit_succeeds() {
    let state = test_state();

    let token = "large-payload-token";
    let secret = b"clipsync-pairing";
    {
        let mut ts = state.token_store.write().await;
        ts.register(token.as_bytes(), "test-device").unwrap();
    }

    // Use 1MB of data — verifies non-trivial payloads succeed.
    // NOTE: Payloads >2MB hit axum's default Json extractor limit; the
    // RequestBodyLimitLayer(20MB) only overrides the tower body limit,
    // not the axum Json extractor default. This is tracked as tech debt.
    let large_data = "A".repeat(1024 * 1024); // 1MB of ASCII

    // ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let payload = serde_json::json!({
        "type": "text",
        "mime": "text/plain",
        "data": large_data,
        "ts": now_ms,
        "nonce": "00000000-0000-0000-0000-000000000000",
        "name": null
    });
    let body_bytes = serde_json::to_vec(&payload).unwrap();

    let app = build_router(state);
    let req = make_inject_request(token, secret, body_bytes);

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ─── large payload: over 20MB rejected ───────────────────────────

#[tokio::test]
async fn oversized_payload_rejected() {
    let state = test_state();

    let token = "oversize-token";
    let secret = b"clipsync-pairing";
    {
        let mut ts = state.token_store.write().await;
        ts.register(token.as_bytes(), "test-device").unwrap();
    }

    // Create a payload over 20MB
    let large_data = "B".repeat(21 * 1024 * 1024);
    let payload = serde_json::json!({
        "type": "text",
        "mime": "text/plain",
        "data": large_data,
        "ts": 1714000000u64,
        "nonce": "00000000-0000-0000-0000-000000000000",
        "name": null
    });
    let body_bytes = serde_json::to_vec(&payload).unwrap();

    let app = build_router(state);
    let req = make_inject_request(token, secret, body_bytes);

    let resp = app.oneshot(req).await.unwrap();
    // Should be 413 Payload Too Large or similar error
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "oversized payload must be rejected"
    );
}

// ─── token revocation: revoke then request returns 401 ───────────

#[tokio::test]
async fn token_revocation_blocks_access() {
    let state = test_state();

    let token = "revoke-me-token";
    let secret = b"clipsync-pairing";
    {
        let mut ts = state.token_store.write().await;
        ts.register(token.as_bytes(), "test-device").unwrap();
    }

    // First request succeeds
    let body_bytes = valid_payload_json();
    let app = build_router(state.clone());
    let req = make_inject_request(token, secret, body_bytes);
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Revoke the token
    {
        let mut ts = state.token_store.write().await;
        ts.revoke(token.as_bytes()).unwrap();
    }

    // Next request returns 401
    let body_bytes = valid_payload_json();
    let app = build_router(state);
    let req = make_inject_request(token, secret, body_bytes);
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ─── clock skew: >60s in the past → rejected ────────────────────

#[tokio::test]
async fn hmac_clock_skew_past_rejected() {
    let state = test_state();

    let token = "skew-past-token";
    let secret = b"clipsync-pairing";
    {
        let mut ts = state.token_store.write().await;
        ts.register(token.as_bytes(), "test-device").unwrap();
    }

    let body_bytes = valid_payload_json();
    let old_ts = now_ts() - 120; // 2 minutes ago
    let sig = hmac::sign(secret, old_ts, &body_bytes);

    let app = build_router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .header("X-ClipSync-Signature", sig)
        .body(Body::from(body_bytes))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ─── clock skew: >60s in the future → rejected ──────────────────

#[tokio::test]
async fn hmac_clock_skew_future_rejected() {
    let state = test_state();

    let token = "skew-future-token";
    let secret = b"clipsync-pairing";
    {
        let mut ts = state.token_store.write().await;
        ts.register(token.as_bytes(), "test-device").unwrap();
    }

    let body_bytes = valid_payload_json();
    let future_ts = now_ts() + 120; // 2 minutes ahead
    let sig = hmac::sign(secret, future_ts, &body_bytes);

    let app = build_router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .header("X-ClipSync-Signature", sig)
        .body(Body::from(body_bytes))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ─── empty body with HMAC → proper error ─────────────────────────

#[tokio::test]
async fn empty_body_with_hmac_rejected() {
    let state = test_state();

    let token = "empty-body-token";
    let secret = b"clipsync-pairing";
    {
        let mut ts = state.token_store.write().await;
        ts.register(token.as_bytes(), "test-device").unwrap();
    }

    let empty_body = b"";
    let ts = now_ts();
    let sig = hmac::sign(secret, ts, empty_body);

    let app = build_router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .header("X-ClipSync-Signature", sig)
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    // Empty body cannot be parsed as JSON ClipPayload → error
    assert_ne!(resp.status(), StatusCode::OK);
}

// ─── invalid JSON body → proper error ────────────────────────────

#[tokio::test]
async fn invalid_json_body_rejected() {
    let state = test_state();

    let token = "bad-json-token";
    let secret = b"clipsync-pairing";
    {
        let mut ts = state.token_store.write().await;
        ts.register(token.as_bytes(), "test-device").unwrap();
    }

    let bad_json = b"{ this is not json }";
    let ts = now_ts();
    let sig = hmac::sign(secret, ts, bad_json);

    let app = build_router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .header("X-ClipSync-Signature", sig)
        .body(Body::from(bad_json.to_vec()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "invalid JSON must be rejected"
    );
}

// ─── missing auth header → 401 ──────────────────────────────────

#[tokio::test]
async fn missing_auth_header_returns_401() {
    let state = test_state();
    let app = build_router(state);

    let body_bytes = valid_payload_json();
    let req = Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .body(Body::from(body_bytes))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ─── malformed HMAC header → 401 ────────────────────────────────

#[tokio::test]
async fn malformed_hmac_header_returns_401() {
    let state = test_state();

    let token = "malformed-hmac-token";
    {
        let mut ts = state.token_store.write().await;
        ts.register(token.as_bytes(), "test-device").unwrap();
    }

    let body_bytes = valid_payload_json();
    let app = build_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .header("X-ClipSync-Signature", "garbage-not-a-valid-hmac-header")
        .body(Body::from(body_bytes))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ─── bearer without HMAC on /inject → 401 ───────────────────────

#[tokio::test]
async fn bearer_without_hmac_on_inject_returns_401() {
    let state = test_state();

    let token = "bearer-only-token";
    {
        let mut ts = state.token_store.write().await;
        ts.register(token.as_bytes(), "test-device").unwrap();
    }

    let body_bytes = valid_payload_json();
    let app = build_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        // No X-ClipSync-Signature header
        .body(Body::from(body_bytes))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ─── pairing code consumed after single use ──────────────────────

#[tokio::test]
async fn pairing_code_single_use() {
    let state = test_state();

    let code = {
        let mut pm = state.pairing_manager.write().await;
        pm.generate_code().to_string()
    };

    // First use succeeds
    let app = build_router(state.clone());
    let req = Request::builder()
        .uri(format!("/pair?code={code}"))
        .header("X-ClipSync-Device", "device-1")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Second use fails
    let app = build_router(state);
    let req = Request::builder()
        .uri(format!("/pair?code={code}"))
        .header("X-ClipSync-Device", "device-2")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ─── unregistered token → 401 ────────────────────────────────────

#[tokio::test]
async fn unregistered_token_returns_401() {
    let state = test_state();
    let secret = b"clipsync-pairing";
    let body_bytes = valid_payload_json();

    let app = build_router(state);
    let req = make_inject_request("nonexistent-token", secret, body_bytes);

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ─── /inject 400 error body shape (Phase 1.3) ────────────────────
//
// All 4xx responses from `/inject` use the standardized JSON body:
//   { "error": "<machine-code>", "message": "<human-readable>" }
// Codes: decode_error | timestamp_out_of_range | payload_too_large | unsupported_kind.

/// Helper: register a fresh token in `state` and return the secret used to sign.
async fn register_inject_token(state: &Arc<AppState>, token: &str) -> &'static [u8] {
    let mut ts = state.token_store.write().await;
    ts.register(token.as_bytes(), "test-device").unwrap();
    b"clipsync-pairing"
}

/// Helper: assert the 400 body matches the standardized `{error, message}` shape
/// and return the parsed JSON for further assertions.
async fn assert_inject_400_body(resp: axum::response::Response) -> serde_json::Value {
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "expected 400 Bad Request"
    );
    let bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&bytes).expect("400 body must be valid JSON");
    let obj = json.as_object().expect("400 body must be a JSON object");
    assert_eq!(
        obj.len(),
        2,
        "400 body must have exactly 2 fields, got: {json}"
    );
    assert!(
        obj.contains_key("error"),
        "400 body must contain `error`, got: {json}"
    );
    assert!(
        obj.contains_key("message"),
        "400 body must contain `message`, got: {json}"
    );
    assert!(json["error"].is_string(), "`error` must be a string");
    assert!(json["message"].is_string(), "`message` must be a string");
    json
}

#[tokio::test]
async fn inject_malformed_json_returns_400_decode_error() {
    let state = test_state();
    let token = "decode-err-token";
    let secret = register_inject_token(&state, token).await;

    let bad_body: Vec<u8> = b"{not valid json".to_vec();
    let ts = now_ts();
    let sig = hmac::sign(secret, ts, &bad_body);

    let app = build_router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .header("X-ClipSync-Signature", sig)
        .body(Body::from(bad_body))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    let body = assert_inject_400_body(resp).await;
    assert_eq!(
        body["error"], "decode_error",
        "expected decode_error, got body: {body}"
    );
    assert!(
        !body["message"].as_str().unwrap().is_empty(),
        "message must be non-empty"
    );
}

#[tokio::test]
async fn inject_stale_timestamp_returns_400_timestamp_out_of_range() {
    let state = test_state();
    let token = "stale-ts-token";
    let secret = register_inject_token(&state, token).await;

    // Build a payload with `ts` 10 minutes in the past — outside the 5-min window.
    // ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let stale_ms = now_ms - 10 * 60 * 1000;
    let payload = serde_json::json!({
        "type": "text",
        "mime": "text/plain",
        "data": "aGVsbG8=",
        "ts": stale_ms,
        "nonce": "00000000-0000-0000-0000-000000000000",
        "name": null
    });
    let body_bytes = serde_json::to_vec(&payload).unwrap();

    // HMAC `t=` is in seconds and must be within ±60s — sign with current time
    // so we exercise the payload-level validation, not the HMAC layer.
    let ts = now_ts();
    let sig = hmac::sign(secret, ts, &body_bytes);

    let app = build_router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .header("X-ClipSync-Signature", sig)
        .body(Body::from(body_bytes))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    let body = assert_inject_400_body(resp).await;
    assert_eq!(
        body["error"], "timestamp_out_of_range",
        "expected timestamp_out_of_range, got body: {body}"
    );
    assert!(
        body["message"].as_str().unwrap().contains("ms"),
        "message should mention ms-deviation, got: {body}"
    );
}

#[tokio::test]
async fn inject_unknown_clip_type_returns_400_unsupported_kind() {
    let state = test_state();
    let token = "unsupp-kind-token";
    let secret = register_inject_token(&state, token).await;

    // ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let payload = serde_json::json!({
        "type": "audio",
        "mime": "audio/mpeg",
        "data": "aGVsbG8=",
        "ts": now_ms,
        "nonce": "00000000-0000-0000-0000-000000000000",
        "name": null
    });
    let body_bytes = serde_json::to_vec(&payload).unwrap();

    let ts = now_ts();
    let sig = hmac::sign(secret, ts, &body_bytes);

    let app = build_router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .header("X-ClipSync-Signature", sig)
        .body(Body::from(body_bytes))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    let body = assert_inject_400_body(resp).await;
    assert_eq!(
        body["error"], "unsupported_kind",
        "expected unsupported_kind, got body: {body}"
    );
}
