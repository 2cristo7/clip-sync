use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum HmacError {
    #[error("invalid signature header format")]
    InvalidFormat,
    #[error("missing timestamp in header")]
    MissingTimestamp,
    #[error("missing v1 signature in header")]
    MissingSignature,
    #[error("timestamp skew exceeds {max_skew}s (delta={delta}s)")]
    TimestampSkew { delta: i64, max_skew: i64 },
    #[error("signature mismatch")]
    SignatureMismatch,
    #[error("invalid timestamp value")]
    InvalidTimestamp,
}

/// Sign a request body with HMAC-SHA256.
///
/// Returns a header value like: `t=1714000000, v1=abcdef0123456789...`
///
/// The signing message is: `"<timestamp>.<body_bytes>"`.
///
/// `timestamp` MUST be a Unix timestamp in **seconds** — this is independent of
/// `ClipPayload.ts` (which is in milliseconds).
///
/// HMAC t= header is in SECONDS. See CLAUDE.md §"Wire Protocol Invariants".
pub fn sign(secret: &[u8], timestamp: u64, body: &[u8]) -> String {
    let message = format!("{}.", timestamp);
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC can accept any key length");
    mac.update(message.as_bytes());
    mac.update(body);
    let result = mac.finalize();
    let hex_sig = hex::encode(result.into_bytes());
    format!("t={}, v1={}", timestamp, hex_sig)
}

/// Verify an HMAC signature header against a body.
///
/// `header` should be in format: `t=<unix_seconds>, v1=<hex>`
/// `now` is the current Unix timestamp in seconds.
/// `max_skew` is the maximum allowed time difference in seconds (typically 60).
///
/// HMAC t= header is in SECONDS. See CLAUDE.md §"Wire Protocol Invariants".
pub fn verify(
    secret: &[u8],
    header: &str,
    body: &[u8],
    now: u64,
    max_skew: i64,
) -> Result<(), HmacError> {
    let (ts, sig) = parse_header(header)?;

    // Check timestamp skew
    let delta = (now as i64) - (ts as i64);
    if delta.abs() > max_skew {
        return Err(HmacError::TimestampSkew { delta, max_skew });
    }

    // Recompute expected signature
    let expected = sign(secret, ts, body);
    let (_, expected_sig) = parse_header(&expected)?;

    // Constant-time comparison
    if !constant_time_eq(sig.as_bytes(), expected_sig.as_bytes()) {
        return Err(HmacError::SignatureMismatch);
    }

    Ok(())
}

/// Parse a header string like `t=1714000000, v1=abcdef...` into (timestamp, hex_signature).
fn parse_header(header: &str) -> Result<(u64, String), HmacError> {
    let mut ts: Option<u64> = None;
    let mut sig: Option<String> = None;

    for part in header.split(", ") {
        let part = part.trim();
        if let Some(val) = part.strip_prefix("t=") {
            ts = Some(val.parse().map_err(|_| HmacError::InvalidTimestamp)?);
        } else if let Some(val) = part.strip_prefix("v1=") {
            sig = Some(val.to_string());
        }
    }

    let ts = ts.ok_or(HmacError::MissingTimestamp)?;
    let sig = sig.ok_or(HmacError::MissingSignature)?;
    Ok((ts, sig))
}

/// Constant-time byte comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_produces_correct_format() {
        let sig = sign(b"secret", 1714000000, b"body");
        assert!(sig.starts_with("t=1714000000, v1="));
        // v1 value should be 64 hex chars (SHA-256)
        let hex_part = sig.strip_prefix("t=1714000000, v1=").unwrap();
        assert_eq!(hex_part.len(), 64);
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn verify_accepts_valid_signature() {
        let secret = b"mysecret";
        let body = b"some body content";
        let ts = 1714000000u64;
        let header = sign(secret, ts, body);
        assert!(verify(secret, &header, body, ts, 60).is_ok());
    }

    #[test]
    fn verify_rejects_wrong_secret() {
        let body = b"some body content";
        let ts = 1714000000u64;
        let header = sign(b"secret1", ts, body);
        let result = verify(b"secret2", &header, body, ts, 60);
        assert!(matches!(result, Err(HmacError::SignatureMismatch)));
    }

    #[test]
    fn verify_rejects_wrong_body() {
        let secret = b"mysecret";
        let ts = 1714000000u64;
        let header = sign(secret, ts, b"original");
        let result = verify(secret, &header, b"tampered", ts, 60);
        assert!(matches!(result, Err(HmacError::SignatureMismatch)));
    }

    #[test]
    fn verify_rejects_skewed_timestamp() {
        let secret = b"mysecret";
        let body = b"body";
        let header = sign(secret, 1714000000, body);
        // now is 120 seconds later, max skew is 60
        let result = verify(secret, &header, body, 1714000120, 60);
        assert!(matches!(result, Err(HmacError::TimestampSkew { .. })));
    }

    #[test]
    fn verify_accepts_within_skew() {
        let secret = b"mysecret";
        let body = b"body";
        let header = sign(secret, 1714000000, body);
        // 30 seconds later, within 60s skew
        assert!(verify(secret, &header, body, 1714000030, 60).is_ok());
        // 60 seconds later, at boundary
        assert!(verify(secret, &header, body, 1714000060, 60).is_ok());
    }

    #[test]
    fn golden_hmac_vector() {
        let golden_str = include_str!("../../../tests/golden/hmac_vector.json");
        let golden: serde_json::Value = serde_json::from_str(golden_str).unwrap();

        let secret_hex = golden["secret_hex"].as_str().unwrap();
        let secret = hex::decode(secret_hex).unwrap();
        let timestamp = golden["timestamp"].as_u64().unwrap();
        let body = golden["body"].as_str().unwrap();
        let expected_header = golden["expected_header"].as_str().unwrap();

        let computed = sign(&secret, timestamp, body.as_bytes());
        assert_eq!(computed, expected_header);

        // Also verify it passes verification
        assert!(verify(&secret, &computed, body.as_bytes(), timestamp, 60).is_ok());
    }

    #[test]
    fn parse_header_rejects_garbage() {
        let result = parse_header("garbage");
        assert!(result.is_err());
    }

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
    }
}
