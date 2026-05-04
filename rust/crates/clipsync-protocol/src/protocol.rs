use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Maximum allowed clock skew between sender and receiver for `ClipPayload.ts`,
/// expressed in **milliseconds** (5 minutes).
///
/// Mirrors the Mac/Android invariant `abs(now_ms - ts) < 5*60*1000`.
/// See CLAUDE.md §"Wire Protocol Invariants".
pub const PAYLOAD_TS_MAX_SKEW_MS: i64 = 5 * 60 * 1000;

/// Errors returned by [`ClipPayload::validate`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PayloadValidationError {
    /// Payload `ts` deviates from server clock by more than [`PAYLOAD_TS_MAX_SKEW_MS`].
    #[error("timestamp_out_of_range: payload ts deviates by {delta_ms}ms (max {max_ms}ms)")]
    TimestampOutOfRange { delta_ms: i64, max_ms: i64 },
}

/// Current Unix timestamp in **milliseconds**.
///
/// ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
pub fn unix_millis() -> u64 {
    // ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// The type of clipboard content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipType {
    Text,
    Image,
    File,
}

/// Logical role of the device emitting / receiving a payload.
///
/// Optional metadata used by enterprise deployments that route based on
/// who-is-who in a session (e.g. "server" canonical store vs. "client"
/// endpoints, or symmetric "peer" mesh nodes). Personal builds leave
/// payloads with `origin_role = None`.
///
/// Phase 1.10 (extensible protocol): see
/// `docs/plans/master-plan-rust-fork.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceRole {
    Server,
    Client,
    Peer,
}

/// Optional broadcast / redaction hints attached to a [`ClipPayload`].
///
/// Reserved for enterprise filtering: `redact = true` asks the receiver
/// to obfuscate sensitive content in audit logs; `broadcast_scope`
/// narrows fan-out (e.g. `"team"`, `"user"`, `"device"`). Personal
/// builds always leave this `None` and decoders MUST tolerate unknown
/// scope strings.
///
/// Phase 1.10 (extensible protocol): see
/// `docs/plans/master-plan-rust-fork.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyHints {
    /// If `true`, audit/log surfaces should redact `data`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redact: Option<bool>,
    /// Broadcast scope hint, e.g. `"team"`, `"user"`, `"device"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broadcast_scope: Option<String>,
}

/// Wire-format payload for clipboard synchronization.
///
/// JSON example:
/// ```json
/// {"type":"text","mime":"text/plain","data":"SGVsbG8=","ts":1714000000000,"nonce":"...","name":null}
/// ```
///
/// ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipPayload {
    #[serde(rename = "type")]
    pub clip_type: ClipType,
    pub mime: String,
    /// Base64-encoded (standard alphabet, with padding) content.
    pub data: String,
    /// Unix timestamp in **milliseconds**.
    ///
    /// ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
    pub ts: u64,
    /// UUID v4 nonce.
    pub nonce: String,
    /// File name, or `null` when not a file.
    pub name: Option<String>,
    /// Optional broadcast / redaction hints. Personal builds leave this
    /// `None`; enterprise builds may populate it.
    ///
    /// Backward-compatible: omitted when serializing if `None`, defaults
    /// to `None` when missing on the wire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<PolicyHints>,
    /// Optional logical role of the originating device.
    ///
    /// Backward-compatible: omitted when serializing if `None`, defaults
    /// to `None` when missing on the wire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_role: Option<DeviceRole>,
}

impl ClipPayload {
    /// Returns the SHA-256 hex digest of the `data` field (the base64 string itself).
    /// Used for echo detection: if the digest matches the last-written payload, skip it.
    pub fn digest(&self) -> String {
        let hash = Sha256::digest(self.data.as_bytes());
        hex::encode(hash)
    }

    /// Decode the `data` field from base64 into raw bytes.
    pub fn decode_data(&self) -> Result<Vec<u8>, base64::DecodeError> {
        BASE64.decode(&self.data)
    }

    /// Create a text payload with the given content.
    pub fn text(content: &str, ts: u64) -> Self {
        Self {
            clip_type: ClipType::Text,
            mime: "text/plain".to_string(),
            data: BASE64.encode(content.as_bytes()),
            ts,
            nonce: uuid::Uuid::new_v4().to_string(),
            name: None,
            policy: None,
            origin_role: None,
        }
    }

    /// Create an image payload with raw PNG bytes.
    pub fn image(png_bytes: &[u8], ts: u64) -> Self {
        Self {
            clip_type: ClipType::Image,
            mime: "image/png".to_string(),
            data: BASE64.encode(png_bytes),
            ts,
            nonce: uuid::Uuid::new_v4().to_string(),
            name: None,
            policy: None,
            origin_role: None,
        }
    }

    /// Validate the payload against a server-side now timestamp (milliseconds).
    ///
    /// Rejects payloads whose `ts` deviates by more than
    /// [`PAYLOAD_TS_MAX_SKEW_MS`] from `now_ms`. Mirrors the Mac/Android
    /// invariant `abs(now_ms - ts) < 5*60*1000`.
    ///
    /// ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
    pub fn validate(&self, now_ms: i64) -> Result<(), PayloadValidationError> {
        let delta = now_ms - (self.ts as i64);
        if delta.abs() >= PAYLOAD_TS_MAX_SKEW_MS {
            return Err(PayloadValidationError::TimestampOutOfRange {
                delta_ms: delta,
                max_ms: PAYLOAD_TS_MAX_SKEW_MS,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_text() {
        let payload = ClipPayload {
            clip_type: ClipType::Text,
            mime: "text/plain".to_string(),
            data: BASE64.encode(b"Hello World"),
            ts: 1714000000,
            nonce: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            name: None,
            policy: None,
            origin_role: None,
        };

        let json = serde_json::to_string(&payload).unwrap();
        let parsed: ClipPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(payload, parsed);
    }

    #[test]
    fn round_trip_image() {
        let payload = ClipPayload {
            clip_type: ClipType::Image,
            mime: "image/png".to_string(),
            data: "iVBORw0KGgoAAAANSUhEUg==".to_string(),
            ts: 1714000001,
            nonce: "660e8400-e29b-41d4-a716-446655440001".to_string(),
            name: None,
            policy: None,
            origin_role: None,
        };

        let json = serde_json::to_string(&payload).unwrap();
        let parsed: ClipPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(payload, parsed);
    }

    #[test]
    fn round_trip_file() {
        let payload = ClipPayload {
            clip_type: ClipType::File,
            mime: "application/pdf".to_string(),
            data: BASE64.encode(b"fake pdf content"),
            ts: 1714000002,
            nonce: "770e8400-e29b-41d4-a716-446655440002".to_string(),
            name: Some("document.pdf".to_string()),
            policy: None,
            origin_role: None,
        };

        let json = serde_json::to_string(&payload).unwrap();
        let parsed: ClipPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(payload, parsed);
        assert_eq!(parsed.name, Some("document.pdf".to_string()));
    }

    #[test]
    fn type_serializes_lowercase() {
        let payload = ClipPayload {
            clip_type: ClipType::Text,
            mime: "text/plain".to_string(),
            data: "dGVzdA==".to_string(),
            ts: 0,
            nonce: "test".to_string(),
            name: None,
            policy: None,
            origin_role: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains(r#""type":"text""#));
    }

    #[test]
    fn name_serializes_as_null() {
        let payload = ClipPayload {
            clip_type: ClipType::Text,
            mime: "text/plain".to_string(),
            data: "dGVzdA==".to_string(),
            ts: 0,
            nonce: "test".to_string(),
            name: None,
            policy: None,
            origin_role: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains(r#""name":null"#));
    }

    #[test]
    fn golden_text_payload() {
        let golden = include_str!("../../../tests/golden/clip_payload_text.json");
        let payload: ClipPayload = serde_json::from_str(golden).unwrap();
        assert_eq!(payload.clip_type, ClipType::Text);
        assert_eq!(payload.mime, "text/plain");
        // ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
        assert_eq!(payload.ts, 1_714_000_000_000);
        assert_eq!(payload.nonce, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(payload.name, None);

        // Verify data decodes to "Hello World"
        let decoded = payload.decode_data().unwrap();
        assert_eq!(decoded, b"Hello World");

        // Re-serialize and compare (compact, no trailing newline)
        let reserialized = serde_json::to_string(&payload).unwrap();
        assert_eq!(reserialized, golden.trim());
    }

    #[test]
    fn golden_image_payload() {
        let golden = include_str!("../../../tests/golden/clip_payload_image.json");
        let payload: ClipPayload = serde_json::from_str(golden).unwrap();
        assert_eq!(payload.clip_type, ClipType::Image);
        assert_eq!(payload.mime, "image/png");
        // ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
        assert_eq!(payload.ts, 1_714_000_001_000);
        assert_eq!(payload.name, None);

        // Re-serialize and compare
        let reserialized = serde_json::to_string(&payload).unwrap();
        assert_eq!(reserialized, golden.trim());
    }

    #[test]
    fn digest_is_consistent() {
        let p1 = ClipPayload {
            clip_type: ClipType::Text,
            mime: "text/plain".to_string(),
            data: "SGVsbG8=".to_string(),
            ts: 0,
            nonce: "a".to_string(),
            name: None,
            policy: None,
            origin_role: None,
        };
        let p2 = ClipPayload {
            clip_type: ClipType::Image,
            mime: "image/png".to_string(),
            data: "SGVsbG8=".to_string(), // same data
            ts: 999,
            nonce: "b".to_string(),
            name: None,
            policy: None,
            origin_role: None,
        };
        // Same data field → same digest
        assert_eq!(p1.digest(), p2.digest());
    }

    // ─── ts unit semantics: ms vs s ──────────────────────────────────

    #[test]
    fn validate_rejects_ts_in_seconds() {
        // A ts value < 10^11 looks like seconds (year 5138 in seconds, 1973 in ms).
        // Treated as ms it lies far in the past, so validation must reject it.
        let payload = ClipPayload::text("hello", 1_714_000_000); // unix seconds-shaped
        let now_ms = 1_714_000_000_000_i64; // current time in ms (≈ 2024-04-25)
        let err = payload.validate(now_ms).unwrap_err();
        assert!(matches!(
            err,
            PayloadValidationError::TimestampOutOfRange { .. }
        ));
    }

    #[test]
    fn validate_accepts_fresh_unix_millis_payload() {
        let now = unix_millis();
        let payload = ClipPayload::text("hello", now);
        assert!(payload.validate(now as i64).is_ok());
        // Within the skew window (1 second drift).
        assert!(payload.validate(now as i64 + 1_000).is_ok());
        assert!(payload.validate(now as i64 - 1_000).is_ok());
    }

    #[test]
    fn validate_rejects_out_of_window_future() {
        let payload = ClipPayload::text("hello", 1_000_000_000_000); // ms-shaped
                                                                     // 5 minutes + 1 second in the future of payload.ts
        let now_ms = 1_000_000_000_000 + (5 * 60 * 1000) + 1_000;
        assert!(payload.validate(now_ms).is_err());
    }

    #[test]
    fn validate_accepts_within_window_boundary() {
        let payload = ClipPayload::text("hello", 1_000_000_000_000);
        // 4 minutes 59 seconds — inside the window
        let now_ms = 1_000_000_000_000_i64 + (4 * 60 * 1000) + 59_000;
        assert!(payload.validate(now_ms).is_ok());
    }

    #[test]
    fn unix_millis_returns_milliseconds_not_seconds() {
        let now = unix_millis();
        // Any reasonable real time is way past 10^12 ms (year 2001+).
        assert!(now > 1_000_000_000_000, "unix_millis must return ms");
    }
}
