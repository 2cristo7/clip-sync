use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The type of clipboard content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipType {
    Text,
    Image,
    File,
}

/// Wire-format payload for clipboard synchronization.
///
/// JSON example:
/// ```json
/// {"type":"text","mime":"text/plain","data":"SGVsbG8=","ts":1714000000,"nonce":"...","name":null}
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipPayload {
    #[serde(rename = "type")]
    pub clip_type: ClipType,
    pub mime: String,
    /// Base64-encoded (standard alphabet, with padding) content.
    pub data: String,
    /// Unix timestamp in seconds.
    pub ts: u64,
    /// UUID v4 nonce.
    pub nonce: String,
    /// File name, or `null` when not a file.
    pub name: Option<String>,
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
        }
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
        assert_eq!(payload.ts, 1714000000);
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
        assert_eq!(payload.ts, 1714000001);
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
        };
        let p2 = ClipPayload {
            clip_type: ClipType::Image,
            mime: "image/png".to_string(),
            data: "SGVsbG8=".to_string(), // same data
            ts: 999,
            nonce: "b".to_string(),
            name: None,
        };
        // Same data field → same digest
        assert_eq!(p1.digest(), p2.digest());
    }
}
