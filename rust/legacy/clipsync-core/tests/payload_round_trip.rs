use clipsync_core::protocol::{ClipPayload, ClipType};

/// Test that golden text payload round-trips through serde unchanged.
#[test]
fn golden_text_round_trip() {
    let golden = include_str!("../../../tests/golden/clip_payload_text.json");
    let payload: ClipPayload = serde_json::from_str(golden).unwrap();

    assert_eq!(payload.clip_type, ClipType::Text);
    assert_eq!(payload.mime, "text/plain");
    assert_eq!(payload.ts, 1_714_000_000_000);
    assert_eq!(payload.nonce, "550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(payload.name, None);

    // Decoded data should be "Hello World"
    let decoded = payload.decode_data().unwrap();
    assert_eq!(String::from_utf8(decoded).unwrap(), "Hello World");

    // Re-serialized form should match original exactly
    let reserialized = serde_json::to_string(&payload).unwrap();
    assert_eq!(reserialized, golden.trim());
}

/// Test that golden image payload round-trips through serde unchanged.
#[test]
fn golden_image_round_trip() {
    let golden = include_str!("../../../tests/golden/clip_payload_image.json");
    let payload: ClipPayload = serde_json::from_str(golden).unwrap();

    assert_eq!(payload.clip_type, ClipType::Image);
    assert_eq!(payload.mime, "image/png");
    assert_eq!(payload.ts, 1_714_000_001_000);
    assert_eq!(payload.name, None);

    // Data should decode as valid base64 (it's a real 1x1 PNG)
    let decoded = payload.decode_data().unwrap();
    assert!(!decoded.is_empty());
    // PNG magic bytes
    assert_eq!(&decoded[..4], &[0x89, 0x50, 0x4E, 0x47]);

    let reserialized = serde_json::to_string(&payload).unwrap();
    assert_eq!(reserialized, golden.trim());
}

/// Test file type payload with name field present.
#[test]
fn file_payload_with_name() {
    let json = r#"{"type":"file","mime":"application/pdf","data":"JVBER","ts":1714000002,"nonce":"test-nonce","name":"doc.pdf"}"#;
    let payload: ClipPayload = serde_json::from_str(json).unwrap();

    assert_eq!(payload.clip_type, ClipType::File);
    assert_eq!(payload.name, Some("doc.pdf".to_string()));

    let reserialized = serde_json::to_string(&payload).unwrap();
    assert_eq!(reserialized, json);
}

/// Test that name=null is preserved (not omitted).
#[test]
fn null_name_preserved() {
    let payload = ClipPayload::text("test", 0);
    let json = serde_json::to_string(&payload).unwrap();
    assert!(
        json.contains(r#""name":null"#),
        "name:null must be explicit in JSON, not omitted"
    );
}

/// Test digest consistency across different payloads with same data.
#[test]
fn digest_echo_detection() {
    let p1 = ClipPayload::text("Hello World", 1000);
    let p2 = ClipPayload::text("Hello World", 2000);

    assert_eq!(
        p1.digest(),
        p2.digest(),
        "Same text content should produce same digest regardless of timestamp"
    );

    let p3 = ClipPayload::text("Different", 1000);
    assert_ne!(
        p1.digest(),
        p3.digest(),
        "Different content should produce different digest"
    );
}
