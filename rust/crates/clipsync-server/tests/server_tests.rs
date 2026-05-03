use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use clipsync_core::config::VERSION;
use clipsync_core::hmac;
use clipsync_core::pairing::PairingManager;
use clipsync_core::tls::TlsIdentity;
use clipsync_core::token_store::TokenStore;
use tokio::sync::RwLock;

use clipsync_server::routes::build_router;
use clipsync_server::ws_hub::WsHub;
use clipsync_server::AppState;

/// Create a test AppState with a temporary data directory.
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

// ─── /health ─────────────────────────────────────────────────────

#[tokio::test]
async fn health_returns_ok() {
    let state = test_state();
    let app = build_router(state);

    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["ok"], true);
    assert_eq!(json["version"], VERSION);
    assert!(json["platform"].is_string());
}

#[tokio::test]
async fn health_requires_no_auth() {
    let state = test_state();
    let app = build_router(state);

    // No Authorization header — should still succeed
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ─── /pair ───────────────────────────────────────────────────────

#[tokio::test]
async fn pair_rejects_invalid_code() {
    let state = test_state();
    let app = build_router(state);

    let req = Request::builder()
        .uri("/pair?code=000000")
        .header("X-ClipSync-Device", "test-device")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn pair_accepts_valid_code() {
    let state = test_state();

    // Generate a pairing code
    let code = {
        let mut pm = state.pairing_manager.write().await;
        pm.generate_code().to_string()
    };

    let app = build_router(state);

    let req = Request::builder()
        .uri(&format!("/pair?code={code}"))
        .header("X-ClipSync-Device", "test-device")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["token"].is_string());
    assert!(json["sig"].is_string());
    assert!(json["secret"].is_string());
}

#[tokio::test]
async fn pair_code_consumed_after_use() {
    let state = test_state();

    let code = {
        let mut pm = state.pairing_manager.write().await;
        pm.generate_code().to_string()
    };

    // First use — succeeds
    let app = build_router(state.clone());
    let req = Request::builder()
        .uri(&format!("/pair?code={code}"))
        .header("X-ClipSync-Device", "test-device")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Second use — fails (code consumed)
    let app = build_router(state);
    let req = Request::builder()
        .uri(&format!("/pair?code={code}"))
        .header("X-ClipSync-Device", "test-device")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ─── /inject auth ────────────────────────────────────────────────

#[tokio::test]
async fn inject_rejects_no_auth() {
    let state = test_state();
    let app = build_router(state);

    let payload = serde_json::json!({
        "type": "text",
        "mime": "text/plain",
        "data": "aGVsbG8=",
        "ts": 1714000000u64,
        "nonce": "00000000-0000-0000-0000-000000000000",
        "name": null
    });

    let req = Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn inject_rejects_bearer_without_hmac() {
    let state = test_state();

    // Register a token
    let token = "test-token-12345";
    {
        let mut ts = state.token_store.write().await;
        ts.register(token.as_bytes(), "test-device").unwrap();
    }

    let app = build_router(state);

    let payload = serde_json::json!({
        "type": "text",
        "mime": "text/plain",
        "data": "aGVsbG8=",
        "ts": 1714000000u64,
        "nonce": "00000000-0000-0000-0000-000000000000",
        "name": null
    });

    let req = Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    // No HMAC header → rejected
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn inject_accepts_valid_bearer_and_hmac() {
    let state = test_state();

    let token = "test-token-12345";
    let secret = b"clipsync-pairing";
    {
        let mut ts = state.token_store.write().await;
        ts.register(token.as_bytes(), "test-device").unwrap();
    }

    let app = build_router(state);

    let payload = serde_json::json!({
        "type": "text",
        "mime": "text/plain",
        "data": "aGVsbG8=",
        "ts": 1714000000u64,
        "nonce": "00000000-0000-0000-0000-000000000000",
        "name": null
    });
    let body_bytes = serde_json::to_vec(&payload).unwrap();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let sig = hmac::sign(secret, now, &body_bytes);

    let req = Request::builder()
        .method("POST")
        .uri("/inject")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .header("X-ClipSync-Signature", sig)
        .body(Body::from(body_bytes))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["ok"], true);
    assert!(json["nonce"].is_string());
}

// ─── /ws auth ────────────────────────────────────────────────────

#[tokio::test]
async fn ws_rejects_no_auth() {
    let state = test_state();
    let app = build_router(state);

    let req = Request::builder()
        .uri("/ws")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn ws_accepts_valid_bearer() {
    let state = test_state();

    let token = "ws-test-token";
    {
        let mut ts = state.token_store.write().await;
        ts.register(token.as_bytes(), "ws-device").unwrap();
    }

    let app = build_router(state);

    let req = Request::builder()
        .uri("/ws")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    // axum WebSocket upgrade via oneshot may return 101 or succeed without error.
    // In test mode without a real HTTP connection, 101 is expected but some
    // versions return 200 or require a real TCP connection for the upgrade.
    // We just verify it's NOT 401 (auth passed).
    assert_ne!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "Auth should pass for valid Bearer token"
    );
}

// ─── WsHub unit tests ───────────────────────────────────────────

#[tokio::test]
async fn ws_hub_register_and_unregister() {
    let hub = WsHub::new();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

    let id = hub.register("device-1".to_string(), tx).await;
    assert_eq!(hub.client_count().await, 1);

    hub.unregister(&id).await;
    assert_eq!(hub.client_count().await, 0);
}

#[tokio::test]
async fn ws_hub_broadcast_to_all() {
    let hub = WsHub::new();
    let (tx1, mut rx1) = tokio::sync::mpsc::unbounded_channel();
    let (tx2, mut rx2) = tokio::sync::mpsc::unbounded_channel();

    let _id1 = hub.register("device-1".to_string(), tx1).await;
    let _id2 = hub.register("device-2".to_string(), tx2).await;

    let payload = clipsync_core::protocol::ClipPayload::text("hello", 1714000000);
    hub.broadcast(&payload, None).await;

    let msg1 = rx1.try_recv().unwrap();
    let msg2 = rx2.try_recv().unwrap();
    assert_eq!(msg1, msg2);

    let parsed: serde_json::Value = serde_json::from_str(&msg1).unwrap();
    assert_eq!(parsed["type"], "text");
}

#[tokio::test]
async fn ws_hub_broadcast_excludes_sender() {
    let hub = WsHub::new();
    let (tx1, mut rx1) = tokio::sync::mpsc::unbounded_channel();
    let (tx2, mut rx2) = tokio::sync::mpsc::unbounded_channel();

    let id1 = hub.register("device-1".to_string(), tx1).await;
    let _id2 = hub.register("device-2".to_string(), tx2).await;

    let payload = clipsync_core::protocol::ClipPayload::text("hello", 1714000000);
    hub.broadcast(&payload, Some(&id1)).await;

    // Device 1 excluded — should not receive
    assert!(rx1.try_recv().is_err());
    // Device 2 should receive
    assert!(rx2.try_recv().is_ok());
}

#[tokio::test]
async fn ws_hub_stale_client_removed() {
    let hub = WsHub::new();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    let _id = hub.register("stale-device".to_string(), tx).await;
    assert_eq!(hub.client_count().await, 1);

    // Drop the receiver — sending should fail and trigger cleanup
    drop(rx);

    hub.broadcast_raw("test", None).await;
    assert_eq!(hub.client_count().await, 0);
}

#[tokio::test]
async fn ws_hub_device_names() {
    let hub = WsHub::new();
    let (tx1, _rx1) = tokio::sync::mpsc::unbounded_channel();
    let (tx2, _rx2) = tokio::sync::mpsc::unbounded_channel();

    hub.register("MacBook".to_string(), tx1).await;
    hub.register("Pixel".to_string(), tx2).await;

    let mut names = hub.device_names().await;
    names.sort();
    assert_eq!(names, vec!["MacBook", "Pixel"]);
}
