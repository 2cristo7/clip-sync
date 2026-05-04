use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

use clipsync_core::clipboard::{ClipboardError, ClipboardProvider};
use clipsync_core::config::PORT;
use clipsync_core::hmac;
use clipsync_core::protocol::{ClipPayload, ClipType};

use clipsync_client::credentials::ClientCredentials;

// ── Mock clipboard ──────────────────────────────────────────────────

struct MockClipboard {
    content: Arc<Mutex<Option<ClipPayload>>>,
    last_written_digest: Arc<Mutex<Option<String>>>,
}

impl MockClipboard {
    fn new() -> Self {
        Self {
            content: Arc::new(Mutex::new(None)),
            last_written_digest: Arc::new(Mutex::new(None)),
        }
    }

    fn set_content(&self, payload: ClipPayload) {
        *self.content.lock().unwrap() = Some(payload);
    }

    #[allow(dead_code)]
    fn get_last_written_digest(&self) -> Option<String> {
        self.last_written_digest.lock().unwrap().clone()
    }
}

impl ClipboardProvider for MockClipboard {
    fn read(&self) -> Result<Option<ClipPayload>, ClipboardError> {
        let content = self.content.lock().unwrap();
        if let Some(ref payload) = *content {
            let digest = payload.digest();
            let last = self.last_written_digest.lock().unwrap();
            if last.as_deref() == Some(&digest) {
                return Ok(None); // echo suppression
            }
            return Ok(Some(payload.clone()));
        }
        Ok(None)
    }

    fn write(&self, payload: &ClipPayload) -> Result<(), ClipboardError> {
        let digest = payload.digest();
        *self.last_written_digest.lock().unwrap() = Some(digest);
        *self.content.lock().unwrap() = Some(payload.clone());
        Ok(())
    }
}

// ── Credential tests ────────────────────────────────────────────────

#[test]
fn credentials_save_load_roundtrip() {
    let dir = std::env::temp_dir().join("clipsync_integ_creds");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let path = dir.join("client_creds.json");

    let creds = ClientCredentials {
        token: BASE64.encode(b"my-token-32-bytes-of-data-here!!"),
        secret: BASE64.encode(b"my-secret-32-bytes-data-here!!!"),
        host: "192.168.1.42".to_string(),
        port: PORT,
        fingerprint: "abcdef1234567890".to_string(),
        server_name: Some("TestMac".to_string()),
    };

    creds.save(&path).unwrap();
    assert!(path.exists());

    let loaded = ClientCredentials::load(&path).unwrap();
    assert_eq!(loaded.token, creds.token);
    assert_eq!(loaded.secret, creds.secret);
    assert_eq!(loaded.host, creds.host);
    assert_eq!(loaded.port, creds.port);
    assert_eq!(loaded.fingerprint, creds.fingerprint);
    assert_eq!(loaded.server_name, Some("TestMac".to_string()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn credentials_load_nonexistent_fails() {
    let path = PathBuf::from("/tmp/clipsync_nonexistent_creds.json");
    let _ = std::fs::remove_file(&path);
    assert!(ClientCredentials::load(&path).is_err());
}

#[test]
fn credentials_load_invalid_json_fails() {
    let dir = std::env::temp_dir().join("clipsync_integ_bad_creds");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let path = dir.join("client_creds.json");
    std::fs::write(&path, "not json").unwrap();
    assert!(ClientCredentials::load(&path).is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

// ── HMAC signing matches server validation ──────────────────────────

#[test]
fn hmac_sign_verify_roundtrip() {
    let secret = b"shared-secret-for-test";
    let body = br#"{"type":"text","mime":"text/plain","data":"SGVsbG8=","ts":1714000000,"nonce":"test","name":null}"#;
    let ts = 1714000000u64;

    let sig_header = hmac::sign(secret, ts, body);

    // Verify the signature passes validation within skew
    assert!(hmac::verify(secret, &sig_header, body, ts, 60).is_ok());
    assert!(hmac::verify(secret, &sig_header, body, ts + 30, 60).is_ok());

    // Verify it fails with wrong secret
    assert!(hmac::verify(b"wrong-secret", &sig_header, body, ts, 60).is_err());

    // Verify it fails with tampered body
    assert!(hmac::verify(secret, &sig_header, b"tampered", ts, 60).is_err());

    // Verify it fails outside skew
    assert!(hmac::verify(secret, &sig_header, body, ts + 120, 60).is_err());
}

#[test]
fn hmac_sign_format_matches_protocol() {
    let secret = b"test";
    let body = b"body";
    let ts = 1714000000u64;

    let header = hmac::sign(secret, ts, body);

    // Format: "t=<timestamp>, v1=<64 hex chars>"
    assert!(header.starts_with("t=1714000000, v1="));
    let hex_part = header.strip_prefix("t=1714000000, v1=").unwrap();
    assert_eq!(hex_part.len(), 64);
    assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
}

// ── Echo suppression ────────────────────────────────────────────────

#[test]
fn mock_clipboard_echo_suppression() {
    let clipboard = MockClipboard::new();
    let payload = ClipPayload::text("Hello from server", 1714000000);

    // Write (simulating receiving from server)
    clipboard.write(&payload).unwrap();

    // Read should return None (echo suppression)
    assert!(clipboard.read().unwrap().is_none());
}

#[test]
fn mock_clipboard_detects_new_content() {
    let clipboard = MockClipboard::new();

    // Set content externally (simulating user copy)
    let payload = ClipPayload::text("User copied text", 1714000000);
    clipboard.set_content(payload);

    // Read should return the content
    let read = clipboard.read().unwrap();
    assert!(read.is_some());
    let read = read.unwrap();
    assert_eq!(read.clip_type, ClipType::Text);
}

#[test]
fn echo_buffer_prevents_resend() {
    // Simulate the echo suppression buffer logic
    let mut recent_digests: VecDeque<String> = VecDeque::new();
    let buffer_size = 32;

    let payload = ClipPayload::text("Hello", 1714000000);
    let digest = payload.digest();

    // Add to recent digests (simulating receiving from server)
    if recent_digests.len() >= buffer_size {
        recent_digests.pop_front();
    }
    recent_digests.push_back(digest.clone());

    // Clipboard produces same content
    let clipboard_payload = ClipPayload::text("Hello", 1714000001);
    let clipboard_digest = clipboard_payload.digest();

    // The base64 of same text produces same digest
    assert_eq!(digest, clipboard_digest);

    // Should be suppressed
    assert!(recent_digests.iter().any(|d| d == &clipboard_digest));
}

// ── Reconnection backoff ────────────────────────────────────────────

#[test]
fn exponential_backoff_sequence() {
    // Verify the backoff sequence: 1, 2, 4, 8, 16, 30, 30, ...
    let delays: Vec<u64> = vec![1, 2, 4, 8, 16, 30, 30];
    let mut current = Duration::from_secs(1);
    let max = Duration::from_secs(30);

    for expected in delays {
        assert_eq!(current, Duration::from_secs(expected));
        current = (current * 2).min(max);
    }
}

// ── Payload creation ────────────────────────────────────────────────

#[test]
fn clip_payload_text_roundtrip() {
    let payload = ClipPayload::text("Hello World", 1714000000);
    let json = serde_json::to_string(&payload).unwrap();
    let parsed: ClipPayload = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.clip_type, ClipType::Text);
    assert_eq!(parsed.mime, "text/plain");
    let decoded = parsed.decode_data().unwrap();
    assert_eq!(decoded, b"Hello World");
}

#[test]
fn clip_payload_image_roundtrip() {
    let fake_png = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic bytes
    let payload = ClipPayload::image(&fake_png, 1714000001);
    let json = serde_json::to_string(&payload).unwrap();
    let parsed: ClipPayload = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.clip_type, ClipType::Image);
    assert_eq!(parsed.mime, "image/png");
    let decoded = parsed.decode_data().unwrap();
    assert_eq!(decoded, fake_png);
}

#[test]
fn clip_payload_digest_consistency() {
    let p1 = ClipPayload::text("same content", 100);
    let p2 = ClipPayload::text("same content", 200);

    // Same content → same digest (regardless of timestamp/nonce)
    assert_eq!(p1.digest(), p2.digest());

    let p3 = ClipPayload::text("different content", 100);
    assert_ne!(p1.digest(), p3.digest());
}
