use clipsync_core::hmac;

/// Test HMAC signing and verification against the golden test vector.
#[test]
fn golden_hmac_vector_integration() {
    let golden_str = include_str!("../../../tests/golden/hmac_vector.json");
    let golden: serde_json::Value = serde_json::from_str(golden_str).unwrap();

    let secret_hex = golden["secret_hex"].as_str().unwrap();
    let secret = hex::decode(secret_hex).unwrap();
    let timestamp = golden["timestamp"].as_u64().unwrap();
    let body = golden["body"].as_str().unwrap();
    let expected_header = golden["expected_header"].as_str().unwrap();

    // Sign and verify the result matches
    let header = hmac::sign(&secret, timestamp, body.as_bytes());
    assert_eq!(
        header, expected_header,
        "HMAC signature does not match golden vector"
    );

    // Verification should succeed with same timestamp
    assert!(
        hmac::verify(&secret, &header, body.as_bytes(), timestamp, 60).is_ok(),
        "HMAC verification failed for valid golden vector"
    );
}

/// Test that a tampered body fails verification.
#[test]
fn tampered_body_fails_verification() {
    let golden_str = include_str!("../../../tests/golden/hmac_vector.json");
    let golden: serde_json::Value = serde_json::from_str(golden_str).unwrap();

    let secret_hex = golden["secret_hex"].as_str().unwrap();
    let secret = hex::decode(secret_hex).unwrap();
    let timestamp = golden["timestamp"].as_u64().unwrap();
    let body = golden["body"].as_str().unwrap();

    let header = hmac::sign(&secret, timestamp, body.as_bytes());

    // Tamper with body (change the base64 data field)
    let tampered = body.replace("SGVsbG8gV29ybGQ=", "R29vZGJ5ZSBXb3JsZA==");
    let result = hmac::verify(&secret, &header, tampered.as_bytes(), timestamp, 60);
    assert!(result.is_err(), "Tampered body should fail verification");
}

/// Test replay attack: old timestamp beyond skew window.
#[test]
fn replay_beyond_skew_rejected() {
    let secret = b"test-secret";
    let body = b"request-body";
    let old_ts = 1714000000u64;

    let header = hmac::sign(secret, old_ts, body);

    // 120 seconds later, beyond 60s skew
    let result = hmac::verify(secret, &header, body, old_ts + 120, 60);
    assert!(
        result.is_err(),
        "Replay beyond skew window should be rejected"
    );
}
