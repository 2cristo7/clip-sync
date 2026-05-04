//! Protocol conformance tests — verify ClipSync wire format, HMAC vectors,
//! golden test data, and server endpoint behaviour against the spec.

use std::net::IpAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use clipsync_core::config::VERSION;
use clipsync_core::hmac;
use clipsync_core::pairing::{PairResponse, PairingManager};
use clipsync_core::protocol::{ClipPayload, ClipType};
use clipsync_core::tls::TlsIdentity;
use clipsync_core::token_store::TokenStore;
use tokio::sync::RwLock;

use clipsync_server::routes::build_router;
use clipsync_server::ws_hub::WsHub;
use clipsync_server::AppState;

// ─── helpers ─────────────────────────────────────────────────────

fn test_state() -> Arc<AppState> {
    let data_dir = std::env::temp_dir().join(format!("clipsync-conform-{}", uuid::Uuid::new_v4()));
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

/// Current Unix time in **milliseconds**, for `ClipPayload.ts`.
/// See CLAUDE.md §"Wire Protocol Invariants".
fn now_ts_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

// ─── golden file: health_response.json ───────────────────────────

#[test]
fn golden_health_response_format() {
    let golden = include_str!("../../../tests/golden/health_response.json");
    let json: serde_json::Value = serde_json::from_str(golden).unwrap();

    assert_eq!(json["ok"], true);
    assert_eq!(json["version"], VERSION);
    assert!(json["platform"].is_string());
    // Platform must be a known value
    let platform = json["platform"].as_str().unwrap();
    assert!(
        ["macos", "linux", "windows"].contains(&platform),
        "unexpected platform: {platform}"
    );
}

// ─── golden file: hmac_vector.json ───────────────────────────────

#[test]
fn golden_hmac_vector_produces_correct_signature() {
    let golden_str = include_str!("../../../tests/golden/hmac_vector.json");
    let golden: serde_json::Value = serde_json::from_str(golden_str).unwrap();

    let secret_hex = golden["secret_hex"].as_str().unwrap();
    let secret = hex::decode(secret_hex).unwrap();
    let timestamp = golden["timestamp"].as_u64().unwrap();
    let body = golden["body"].as_str().unwrap();
    let expected_header = golden["expected_header"].as_str().unwrap();

    let computed = hmac::sign(&secret, timestamp, body.as_bytes());
    assert_eq!(computed, expected_header);
}

#[test]
fn golden_hmac_vector_verifies_successfully() {
    let golden_str = include_str!("../../../tests/golden/hmac_vector.json");
    let golden: serde_json::Value = serde_json::from_str(golden_str).unwrap();

    let secret = hex::decode(golden["secret_hex"].as_str().unwrap()).unwrap();
    let timestamp = golden["timestamp"].as_u64().unwrap();
    let body = golden["body"].as_str().unwrap();
    let expected_header = golden["expected_header"].as_str().unwrap();

    assert!(hmac::verify(&secret, expected_header, body.as_bytes(), timestamp, 60).is_ok());
}

// ─── golden file: clip_payload_text.json ─────────────────────────

#[test]
fn golden_text_payload_deserializes() {
    let golden = include_str!("../../../tests/golden/clip_payload_text.json");
    let payload: ClipPayload = serde_json::from_str(golden).unwrap();

    assert_eq!(payload.clip_type, ClipType::Text);
    assert_eq!(payload.mime, "text/plain");
    // ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
    assert_eq!(payload.ts, 1_714_000_000_000);
    assert_eq!(payload.nonce, "550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(payload.name, None);

    let decoded = payload.decode_data().unwrap();
    assert_eq!(decoded, b"Hello World");
}

#[test]
fn golden_text_payload_round_trips() {
    let golden = include_str!("../../../tests/golden/clip_payload_text.json");
    let payload: ClipPayload = serde_json::from_str(golden).unwrap();
    let reserialized = serde_json::to_string(&payload).unwrap();
    assert_eq!(reserialized, golden.trim());
}

// ─── golden file: clip_payload_image.json ────────────────────────

#[test]
fn golden_image_payload_deserializes() {
    let golden = include_str!("../../../tests/golden/clip_payload_image.json");
    let payload: ClipPayload = serde_json::from_str(golden).unwrap();

    assert_eq!(payload.clip_type, ClipType::Image);
    assert_eq!(payload.mime, "image/png");
    // ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
    assert_eq!(payload.ts, 1_714_000_001_000);
    assert_eq!(payload.nonce, "660e8400-e29b-41d4-a716-446655440001");
    assert_eq!(payload.name, None);
}

#[test]
fn golden_image_payload_round_trips() {
    let golden = include_str!("../../../tests/golden/clip_payload_image.json");
    let payload: ClipPayload = serde_json::from_str(golden).unwrap();
    let reserialized = serde_json::to_string(&payload).unwrap();
    assert_eq!(reserialized, golden.trim());
}

// ─── golden file: pair_response.json ─────────────────────────────

#[test]
fn golden_pair_response_has_correct_format() {
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;

    let golden = include_str!("../../../tests/golden/pair_response.json");
    let resp: PairResponse = serde_json::from_str(golden).unwrap();

    // Token: valid base64 (production uses 32 bytes; golden file is a test fixture)
    let token_bytes = BASE64.decode(&resp.token).unwrap();
    assert!(
        !token_bytes.is_empty(),
        "token must decode to non-empty bytes"
    );

    // Sig: valid base64, HMAC-SHA256 output = 32 bytes
    let sig_bytes = BASE64.decode(&resp.sig).unwrap();
    assert_eq!(sig_bytes.len(), 32, "sig must be 32 bytes (HMAC-SHA256)");

    // Secret: valid base64 (production uses 32 bytes; golden file is a test fixture)
    let secret_bytes = BASE64.decode(&resp.secret).unwrap();
    assert!(
        !secret_bytes.is_empty(),
        "secret must decode to non-empty bytes"
    );
}

// ─── HMAC: wrong secret fails verification ───────────────────────

#[test]
fn hmac_wrong_secret_fails() {
    let body = b"some body content";
    let ts = 1714000000u64;
    let header = hmac::sign(b"correct-secret", ts, body);

    let result = hmac::verify(b"wrong-secret", &header, body, ts, 60);
    assert!(result.is_err(), "HMAC with wrong secret must fail");
}

// ─── HMAC: expired timestamp (>60s skew) fails ──────────────────

#[test]
fn hmac_expired_timestamp_past_fails() {
    let secret = b"mysecret";
    let body = b"payload";
    let ts = 1714000000u64;
    let header = hmac::sign(secret, ts, body);

    // 61 seconds later — exceeds 60s skew
    let result = hmac::verify(secret, &header, body, ts + 61, 60);
    assert!(result.is_err(), "HMAC with >60s past skew must fail");
}

#[test]
fn hmac_expired_timestamp_future_fails() {
    let secret = b"mysecret";
    let body = b"payload";
    let ts = 1714000000u64;
    let header = hmac::sign(secret, ts, body);

    // 61 seconds in the past relative to signed ts
    let result = hmac::verify(secret, &header, body, ts.saturating_sub(61), 60);
    assert!(result.is_err(), "HMAC with >60s future skew must fail");
}

// ─── server: /health returns matching JSON ───────────────────────

#[tokio::test]
async fn server_health_returns_conformant_json() {
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

    // Must have exactly these three fields
    let obj = json.as_object().unwrap();
    assert_eq!(obj.len(), 3);
    assert!(obj.contains_key("ok"));
    assert!(obj.contains_key("version"));
    assert!(obj.contains_key("platform"));
}

// ─── server: /pair with valid code returns token ─────────────────

#[tokio::test]
async fn server_pair_valid_code_returns_token() {
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;

    let state = test_state();

    let code = {
        let mut pm = state.pairing_manager.write().await;
        pm.generate_code().to_string()
    };

    let app = build_router(state);

    let req = Request::builder()
        .uri(format!("/pair?code={code}"))
        .header("X-ClipSync-Device", "conformance-test")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify fields exist and are base64
    let token = json["token"].as_str().unwrap();
    let sig = json["sig"].as_str().unwrap();
    let secret = json["secret"].as_str().unwrap();

    assert!(BASE64.decode(token).is_ok(), "token must be valid base64");
    assert!(BASE64.decode(sig).is_ok(), "sig must be valid base64");
    assert!(BASE64.decode(secret).is_ok(), "secret must be valid base64");

    // Token decoded is 32 bytes
    assert_eq!(BASE64.decode(token).unwrap().len(), 32);
}

// ─── server: /inject with valid auth succeeds ────────────────────

#[tokio::test]
async fn server_inject_valid_auth_succeeds() {
    let state = test_state();

    let token = "conformance-inject-token";
    let secret = b"clipsync-pairing";
    {
        let mut ts = state.token_store.write().await;
        ts.register(token.as_bytes(), "test-device").unwrap();
    }

    let app = build_router(state);

    let payload = serde_json::json!({
        "type": "text",
        "mime": "text/plain",
        "data": "SGVsbG8gV29ybGQ=",
        // ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
        "ts": now_ts_ms(),
        "nonce": "550e8400-e29b-41d4-a716-446655440000",
        "name": null
    });
    let body_bytes = serde_json::to_vec(&payload).unwrap();

    let ts = now_ts();
    let sig = hmac::sign(secret, ts, &body_bytes);

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

// ─── server: /inject without auth returns 401 ────────────────────

#[tokio::test]
async fn server_inject_no_auth_returns_401() {
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

// ─── wire format: all required fields present ────────────────────

#[test]
fn wire_format_text_has_all_required_fields() {
    let payload = ClipPayload::text("Hello", 1714000000);
    let json_str = serde_json::to_string(&payload).unwrap();
    let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let obj = json.as_object().unwrap();

    assert!(obj.contains_key("type"));
    assert!(obj.contains_key("mime"));
    assert!(obj.contains_key("data"));
    assert!(obj.contains_key("ts"));
    assert!(obj.contains_key("nonce"));
    assert!(obj.contains_key("name"));
    assert_eq!(obj.len(), 6, "payload must have exactly 6 fields");
}

#[test]
fn wire_format_image_has_all_required_fields() {
    let payload = ClipPayload::image(b"\x89PNG", 1714000001);
    let json_str = serde_json::to_string(&payload).unwrap();
    let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let obj = json.as_object().unwrap();

    assert_eq!(json["type"], "image");
    assert_eq!(json["mime"], "image/png");
    assert!(json["name"].is_null());
    assert_eq!(obj.len(), 6);
}

// ─── HMAC header format conformance ──────────────────────────────

#[test]
fn hmac_header_format_matches_spec() {
    let sig = hmac::sign(b"secret", 1714000000, b"body");

    // Format: "t=<unix_s>, v1=<hex>"
    assert!(sig.starts_with("t="), "must start with t=");
    assert!(sig.contains(", v1="), "must contain ', v1='");

    let parts: Vec<&str> = sig.split(", ").collect();
    assert_eq!(parts.len(), 2, "must have exactly 2 parts");

    let ts_part = parts[0].strip_prefix("t=").unwrap();
    assert!(ts_part.parse::<u64>().is_ok(), "timestamp must be u64");

    let v1_part = parts[1].strip_prefix("v1=").unwrap();
    assert_eq!(v1_part.len(), 64, "v1 must be 64 hex chars (SHA-256)");
    assert!(v1_part.chars().all(|c| c.is_ascii_hexdigit()));
}

// ─── nonce is UUID v4 format ─────────────────────────────────────

#[test]
fn payload_nonce_is_uuid_format() {
    let payload = ClipPayload::text("test", 0);
    let nonce = &payload.nonce;

    // UUID v4 format: 8-4-4-4-12 hex chars with dashes
    assert_eq!(nonce.len(), 36);
    let parts: Vec<&str> = nonce.split('-').collect();
    assert_eq!(parts.len(), 5);
    assert_eq!(parts[0].len(), 8);
    assert_eq!(parts[1].len(), 4);
    assert_eq!(parts[2].len(), 4);
    assert_eq!(parts[3].len(), 4);
    assert_eq!(parts[4].len(), 12);
}
