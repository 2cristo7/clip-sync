use clipsync_core::config;
use clipsync_core::protocol::{ClipPayload, ClipType};

/// Verify protocol constants match the spec.
#[test]
fn protocol_constants() {
    assert_eq!(config::PORT, 7010);
    assert_eq!(config::VERSION, "0.1.0");
    assert_eq!(config::MAX_PAYLOAD_BYTES, 20 * 1024 * 1024);
    assert_eq!(config::HMAC_MAX_SKEW_SECS, 60);
    assert_eq!(config::PAIRING_CODE_TTL_SECS, 120);
    assert_eq!(config::TLS_CERT_VALIDITY_DAYS, 365);
    assert_eq!(config::MDNS_SERVICE_TYPE, "_clipsync._tcp.local.");
}

/// Verify health response format.
#[test]
fn health_response_format() {
    let golden = include_str!("../../../tests/golden/health_response.json");
    let v: serde_json::Value = serde_json::from_str(golden).unwrap();

    assert_eq!(v["ok"], true);
    assert_eq!(v["version"], "0.1.0");
    assert!(v["platform"].is_string());
}

/// Verify pair response structure matches golden file.
#[test]
fn pair_response_structure() {
    let golden = include_str!("../../../tests/golden/pair_response.json");
    let resp: clipsync_core::pairing::PairResponse = serde_json::from_str(golden).unwrap();

    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;

    // All fields must decode as valid base64
    let token_bytes = BASE64.decode(&resp.token).unwrap();
    assert!(!token_bytes.is_empty(), "Token must not be empty");

    let secret_bytes = BASE64.decode(&resp.secret).unwrap();
    assert!(!secret_bytes.is_empty(), "Secret must not be empty");

    // Sig is HMAC-SHA256 output = always 32 bytes
    let sig_bytes = BASE64.decode(&resp.sig).unwrap();
    assert_eq!(
        sig_bytes.len(),
        32,
        "Signature must be 32 bytes (HMAC-SHA256)"
    );
}

/// Verify ClipType serialization matches wire format exactly.
#[test]
fn clip_type_wire_format() {
    // type must serialize as lowercase string
    let types = vec![
        (ClipType::Text, "\"text\""),
        (ClipType::Image, "\"image\""),
        (ClipType::File, "\"file\""),
    ];

    for (clip_type, expected) in types {
        let json = serde_json::to_string(&clip_type).unwrap();
        assert_eq!(
            json, expected,
            "ClipType::{:?} must serialize to {}",
            clip_type, expected
        );
    }
}

/// Cross-language compatibility golden vector — must decode and produce a
/// payload whose `ts` is in the **milliseconds** range. Mirrors the wire
/// format the Mac/Android peers emit.
///
/// ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
#[test]
fn compat_payload_v1_decodes_with_ms_timestamp() {
    let raw = include_str!("../../../tests/compat/payload_v1.json");
    let payload: ClipPayload = serde_json::from_str(raw).unwrap();

    assert_eq!(payload.ts, 1_714_000_000_000);
    // Sanity: the value lies in the ms range (>10^12), not seconds (~10^9).
    assert!(payload.ts > 1_000_000_000_000);

    // Round-trips through serde unchanged.
    let reserialized = serde_json::to_string(&payload).unwrap();
    assert_eq!(reserialized, raw.trim());

    // Validates as fresh when "now" is at the same ms.
    payload.validate(payload.ts as i64).unwrap();
}

/// Phase 1.8: explicitly-named ms-timestamp vector. Same wire bytes as
/// `payload_v1.json`; this vector is the canonical name introduced to make
/// the unit (milliseconds) unambiguous in the file name itself.
#[test]
fn compat_payload_v1_ms_decodes_with_ms_timestamp() {
    let raw = include_str!("../../../tests/compat/payload_v1_ms.json");
    let payload: ClipPayload = serde_json::from_str(raw).unwrap();

    assert_eq!(payload.ts, 1_714_000_000_000);
    assert!(payload.ts > 1_000_000_000_000);

    let reserialized = serde_json::to_string(&payload).unwrap();
    assert_eq!(reserialized, raw.trim());

    payload.validate(payload.ts as i64).unwrap();

    // Byte-equality with the legacy filename — the explicit "_ms" name is
    // a pure alias for clarity; bytes MUST stay identical.
    let legacy = include_str!("../../../tests/compat/payload_v1.json");
    assert_eq!(raw, legacy);
}

/// Phase 1.8: 401 body returned by `/pair?code=<wrong>`.
///
/// Wire shape (Phase 1.5): `{"error":"invalid"}` — NO `message` field.
/// This vector is fully deterministic, so we assert byte-equality after
/// JSON normalisation.
#[test]
fn compat_pairing_error_invalid_shape() {
    let raw = include_str!("../../../tests/compat/pairing_error_invalid.json");
    let v: serde_json::Value = serde_json::from_str(raw).unwrap();

    assert_eq!(v["error"], "invalid");
    assert!(
        v.get("message").is_none(),
        "/pair 401 body MUST NOT include a `message` field"
    );

    // Body has exactly one key.
    let obj = v.as_object().unwrap();
    assert_eq!(
        obj.len(),
        1,
        "/pair 401 body must have exactly one key (`error`)"
    );

    // Round-trip identity: serializing the deserialized form yields the
    // same canonical bytes axum produces on the wire.
    let reserialized = serde_json::to_string(&v).unwrap();
    assert_eq!(reserialized, raw.trim());
}

/// Phase 1.8: 400 body returned by `POST /inject` when the body fails to
/// decode as a `ClipPayload`.
///
/// Wire shape (Phase 1.3): `{"error":"<code>","message":"<text>"}`.
/// The `error` code is part of the contract; the exact `message` text is
/// not — it carries the underlying serde detail, which can vary across
/// `serde_json` versions. We therefore assert shape (keys + error code +
/// non-empty message), not byte-equality.
#[test]
fn compat_inject_400_decode_shape() {
    let raw = include_str!("../../../tests/compat/inject_400_decode.json");
    let v: serde_json::Value = serde_json::from_str(raw).unwrap();

    assert_eq!(v["error"], "decode_error");
    assert!(v["message"].is_string(), "`message` must be a string");
    assert!(
        !v["message"].as_str().unwrap().is_empty(),
        "`message` must be non-empty"
    );

    // Body has exactly two keys: error + message.
    let obj = v.as_object().unwrap();
    assert_eq!(
        obj.len(),
        2,
        "/inject 4xx body must have exactly `error` + `message`"
    );
    assert!(obj.contains_key("error"));
    assert!(obj.contains_key("message"));
}

/// Verify ts is Unix **milliseconds** (not seconds).
///
/// ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
#[test]
fn timestamp_is_unix_milliseconds() {
    let golden = include_str!("../../../tests/golden/clip_payload_text.json");
    let payload: ClipPayload = serde_json::from_str(golden).unwrap();

    // 1714000000000 is approximately April 2024 in Unix milliseconds.
    // If it were seconds (~1.7e9), the wire format would mismatch the
    // Mac/Android peers which always emit milliseconds.
    assert!(
        payload.ts > 1_000_000_000_000,
        "ts must be in Unix milliseconds, not seconds"
    );
    assert!(
        payload.ts < 9_999_999_999_999,
        "ts must fit a sane millisecond range"
    );
}

/// Verify data field uses standard base64 (not base64url).
#[test]
fn data_uses_standard_base64() {
    let golden = include_str!("../../../tests/golden/clip_payload_text.json");
    let payload: ClipPayload = serde_json::from_str(golden).unwrap();

    // Standard base64 uses + and /, with = padding
    // base64url uses - and _
    // "SGVsbG8gV29ybGQ=" uses standard alphabet
    assert!(
        !payload.data.contains('-') && !payload.data.contains('_'),
        "data field should use standard base64, not base64url"
    );

    // Should decode successfully with standard base64
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;
    assert!(BASE64.decode(&payload.data).is_ok());
}

/// Verify fingerprint format: base64url WITHOUT padding.
#[test]
fn fingerprint_format() {
    let identity = clipsync_core::tls::TlsIdentity::generate(&[], &[]).unwrap();
    let fp = clipsync_core::fingerprint::spki_sha256(&identity.cert_der).unwrap();

    // Must be base64url (no + or /) and no padding (no =)
    assert!(
        !fp.contains('+'),
        "fingerprint must use base64url, not standard base64"
    );
    assert!(
        !fp.contains('/'),
        "fingerprint must use base64url, not standard base64"
    );
    assert!(!fp.contains('='), "fingerprint must not have padding");

    // SHA-256 = 32 bytes → 43 base64url chars without padding
    assert_eq!(fp.len(), 43);
}
