//! Phase 1.10 — extensible protocol round-trip & backward-compat tests.
//!
//! Covers:
//! 1. Old-shape payloads (no `policy`, no `origin_role`) round-trip
//!    byte-equal.
//! 2. New extended payloads with `policy = Some(...)` and
//!    `origin_role = Some(Client)` round-trip equal.
//! 3. The new `Handshake` struct round-trips equal.
//! 4. Existing Mac/Android golden vector
//!    (`tests/compat/payload_v1_ms.json`) still decodes successfully and
//!    the new optional fields default to `None`.

use clipsync_protocol::handshake::Handshake;
use clipsync_protocol::protocol::{ClipPayload, ClipType, DeviceRole, PolicyHints};

/// Phase 1.8 golden vector covering the post-fix wire shape. Path is
/// relative to this file under
/// `rust/crates/clipsync-protocol/tests/extensions.rs`, so we walk up
/// to `rust/tests/compat/`.
const COMPAT_V1_MS_JSON: &str = include_str!("../../../tests/compat/payload_v1_ms.json");

#[test]
fn payload_v1_no_extensions_round_trips_unchanged() {
    // Old-shape payload: no `policy`, no `origin_role`. With
    // `skip_serializing_if = "Option::is_none"` the serialized form must
    // contain neither field, and the bytes must match a freshly
    // composed canonical encoding.
    let raw = r#"{"type":"text","mime":"text/plain","data":"SGVsbG8gV29ybGQ=","ts":1714000000000,"nonce":"550e8400-e29b-41d4-a716-446655440000","name":null}"#;
    let parsed: ClipPayload = serde_json::from_str(raw).unwrap();

    // Defaults must be None when fields are absent.
    assert_eq!(parsed.policy, None);
    assert_eq!(parsed.origin_role, None);

    let reserialized = serde_json::to_string(&parsed).unwrap();
    assert_eq!(
        reserialized, raw,
        "old-shape payload must round-trip byte-equal"
    );

    // Sanity: no extension fields leaked into the output.
    assert!(!reserialized.contains("policy"));
    assert!(!reserialized.contains("origin_role"));
}

#[test]
fn payload_with_extensions_round_trips() {
    let payload = ClipPayload {
        clip_type: ClipType::Text,
        mime: "text/plain".to_string(),
        data: "SGVsbG8=".to_string(),
        ts: 1_714_000_000_000,
        nonce: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        name: None,
        policy: Some(PolicyHints {
            redact: Some(true),
            broadcast_scope: Some("team".to_string()),
        }),
        origin_role: Some(DeviceRole::Client),
    };

    let json = serde_json::to_string(&payload).unwrap();
    let parsed: ClipPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(payload, parsed);

    // Spot-check that the extensions are actually serialized.
    assert!(json.contains(r#""policy""#));
    assert!(json.contains(r#""origin_role":"client""#));
    assert!(json.contains(r#""broadcast_scope":"team""#));
    assert!(json.contains(r#""redact":true"#));
}

#[test]
fn handshake_round_trips() {
    let hs = Handshake {
        device_id: "abc-1234".to_string(),
        role: DeviceRole::Peer,
        capabilities: vec![
            "broadcast".to_string(),
            "policy".to_string(),
            "audit".to_string(),
        ],
    };
    let json = serde_json::to_string(&hs).unwrap();
    let parsed: Handshake = serde_json::from_str(&json).unwrap();
    assert_eq!(hs, parsed);

    // Role is rendered lowercase on the wire.
    assert!(json.contains(r#""role":"peer""#));
}

#[test]
fn compat_v1_ms_golden_decodes_with_defaults() {
    // The Phase 1.8 compat vector predates Phase 1.10 extensions; the
    // optional fields must default to `None` and the byte form must
    // still round-trip unchanged.
    let parsed: ClipPayload = serde_json::from_str(COMPAT_V1_MS_JSON).unwrap();
    assert_eq!(parsed.policy, None);
    assert_eq!(parsed.origin_role, None);
    assert_eq!(parsed.ts, 1_714_000_000_000);
    assert_eq!(parsed.clip_type, ClipType::Text);

    let reserialized = serde_json::to_string(&parsed).unwrap();
    assert_eq!(reserialized, COMPAT_V1_MS_JSON.trim());
}

#[test]
fn policy_hints_omits_none_fields() {
    let hints = PolicyHints {
        redact: None,
        broadcast_scope: Some("user".to_string()),
    };
    let json = serde_json::to_string(&hints).unwrap();
    assert!(!json.contains("redact"));
    assert!(json.contains(r#""broadcast_scope":"user""#));
}

#[test]
fn device_role_serializes_lowercase() {
    assert_eq!(
        serde_json::to_string(&DeviceRole::Server).unwrap(),
        r#""server""#
    );
    assert_eq!(
        serde_json::to_string(&DeviceRole::Client).unwrap(),
        r#""client""#
    );
    assert_eq!(
        serde_json::to_string(&DeviceRole::Peer).unwrap(),
        r#""peer""#
    );
}
