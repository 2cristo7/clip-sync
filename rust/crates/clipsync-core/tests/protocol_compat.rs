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

/// Verify ts is Unix seconds (not milliseconds).
#[test]
fn timestamp_is_unix_seconds() {
    let golden = include_str!("../../../tests/golden/clip_payload_text.json");
    let payload: ClipPayload = serde_json::from_str(golden).unwrap();

    // 1714000000 is approximately April 2024 in Unix seconds
    // If it were milliseconds, it would be year ~56000
    assert!(payload.ts > 1_000_000_000, "ts should be in Unix seconds");
    assert!(
        payload.ts < 2_000_000_000,
        "ts should be in Unix seconds, not milliseconds"
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
