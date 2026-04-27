use std::sync::{Arc, Mutex};

use crate::protocol::{ClipPayload, ClipType};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("clipboard access failed: {0}")]
    AccessFailed(String),
    #[error("unsupported content type")]
    UnsupportedType,
}

/// Trait abstracting clipboard operations for cross-platform support.
pub trait ClipboardProvider: Send + Sync {
    /// Read the current clipboard content, if any.
    fn read(&self) -> Result<Option<ClipPayload>, ClipboardError>;

    /// Write a payload to the system clipboard.
    fn write(&self, payload: &ClipPayload) -> Result<(), ClipboardError>;
}

/// Default clipboard implementation using the `arboard` crate.
/// Includes echo suppression: after writing, the next read with the same
/// data digest is suppressed to avoid sync loops.
pub struct SystemClipboard {
    last_written_digest: Arc<Mutex<Option<String>>>,
}

impl SystemClipboard {
    pub fn new() -> Self {
        Self {
            last_written_digest: Arc::new(Mutex::new(None)),
        }
    }

    /// Compute SHA-256 hex digest of raw bytes (for echo detection).
    fn data_digest(data: &[u8]) -> String {
        let hash = Sha256::digest(data);
        hex::encode(hash)
    }
}

impl Default for SystemClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardProvider for SystemClipboard {
    fn read(&self) -> Result<Option<ClipPayload>, ClipboardError> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

        // Try text first
        if let Ok(text) = clipboard.get_text() {
            if !text.is_empty() {
                let digest = Self::data_digest(text.as_bytes());

                // Echo suppression: skip if we just wrote this
                let last = self.last_written_digest.lock().unwrap();
                if last.as_deref() == Some(&digest) {
                    return Ok(None);
                }
                drop(last);

                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                return Ok(Some(ClipPayload {
                    clip_type: ClipType::Text,
                    mime: "text/plain".to_string(),
                    data: BASE64.encode(text.as_bytes()),
                    ts,
                    nonce: uuid::Uuid::new_v4().to_string(),
                    name: None,
                }));
            }
        }

        // Try image
        if let Ok(img) = clipboard.get_image() {
            // arboard gives us RGBA pixels; encode as PNG for wire format
            let png_bytes = encode_rgba_as_png(
                &img.bytes,
                img.width as u32,
                img.height as u32,
            );

            let digest = Self::data_digest(&png_bytes);

            let last = self.last_written_digest.lock().unwrap();
            if last.as_deref() == Some(&digest) {
                return Ok(None);
            }
            drop(last);

            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            return Ok(Some(ClipPayload {
                clip_type: ClipType::Image,
                mime: "image/png".to_string(),
                data: BASE64.encode(&png_bytes),
                ts,
                nonce: uuid::Uuid::new_v4().to_string(),
                name: None,
            }));
        }

        Ok(None)
    }

    fn write(&self, payload: &ClipPayload) -> Result<(), ClipboardError> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

        match payload.clip_type {
            ClipType::Text => {
                let raw = BASE64.decode(&payload.data)
                    .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;
                let text = String::from_utf8(raw)
                    .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

                // Record digest for echo suppression
                let digest = Self::data_digest(text.as_bytes());
                *self.last_written_digest.lock().unwrap() = Some(digest);

                clipboard.set_text(&text)
                    .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;
            }
            ClipType::Image => {
                let png_bytes = BASE64.decode(&payload.data)
                    .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

                // Record digest for echo suppression (digest of raw PNG bytes)
                let digest = Self::data_digest(&png_bytes);
                *self.last_written_digest.lock().unwrap() = Some(digest);

                let (width, height, rgba) = decode_png_to_rgba(&png_bytes)
                    .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

                let img_data = arboard::ImageData {
                    width: width as usize,
                    height: height as usize,
                    bytes: std::borrow::Cow::Owned(rgba),
                };

                clipboard.set_image(img_data)
                    .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;
            }
            ClipType::File => {
                return Err(ClipboardError::UnsupportedType);
            }
        }

        Ok(())
    }
}

/// Encode RGBA pixel data as PNG bytes using a minimal encoder.
fn encode_rgba_as_png(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    // Use a simple PNG encoder
    let mut buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut buf, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("PNG header write failed");
        writer.write_image_data(rgba).expect("PNG data write failed");
    }
    buf
}

/// Decode PNG bytes into (width, height, rgba_bytes).
fn decode_png_to_rgba(png_bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let cursor = std::io::Cursor::new(png_bytes);
    let decoder = png::Decoder::new(cursor);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());
    Ok((info.width, info.height, buf))
}

/// A watcher that polls the clipboard for changes and sends new payloads.
pub struct ClipboardWatcher<P: ClipboardProvider> {
    provider: P,
    last_digest: Option<String>,
    poll_interval_ms: u64,
}

impl<P: ClipboardProvider> ClipboardWatcher<P> {
    pub fn new(provider: P, poll_interval_ms: u64) -> Self {
        Self {
            provider,
            last_digest: None,
            poll_interval_ms,
        }
    }

    /// Poll once, returning a new payload if the clipboard changed.
    pub fn poll(&mut self) -> Result<Option<ClipPayload>, ClipboardError> {
        if let Some(payload) = self.provider.read()? {
            let digest = payload.digest();
            if self.last_digest.as_deref() != Some(&digest) {
                self.last_digest = Some(digest);
                return Ok(Some(payload));
            }
        }
        Ok(None)
    }

    /// Start a polling loop, sending changes to the provided channel.
    pub async fn watch(
        mut self,
        tx: tokio::sync::mpsc::Sender<ClipPayload>,
    ) -> Result<(), ClipboardError> {
        let interval = tokio::time::Duration::from_millis(self.poll_interval_ms);
        loop {
            if let Some(payload) = self.poll()? {
                if tx.send(payload).await.is_err() {
                    // Receiver dropped, stop watching
                    break;
                }
            }
            tokio::time::sleep(interval).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A mock clipboard for testing without system clipboard access.
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

    #[test]
    fn echo_suppression() {
        let mock = MockClipboard::new();
        let payload = ClipPayload::text("Hello", 1714000000);

        // Write to clipboard
        mock.write(&payload).unwrap();

        // Read should return None (echo suppression)
        let result = mock.read().unwrap();
        assert!(result.is_none(), "Should suppress echo after write");
    }

    #[test]
    fn reads_new_content() {
        let mock = MockClipboard::new();
        let payload = ClipPayload::text("Hello", 1714000000);
        mock.set_content(payload);

        let result = mock.read().unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().clip_type, ClipType::Text);
    }

    #[test]
    fn watcher_detects_changes() {
        let mock = MockClipboard::new();
        let mut watcher = ClipboardWatcher::new(mock, 100);

        // No content initially
        assert!(watcher.poll().unwrap().is_none());
    }

    #[test]
    fn watcher_skips_duplicate() {
        let mock = MockClipboard::new();
        let payload = ClipPayload::text("Hello", 1714000000);
        mock.set_content(payload);

        let mut watcher = ClipboardWatcher::new(mock, 100);

        // First poll gets the content
        let first = watcher.poll().unwrap();
        assert!(first.is_some());

        // Second poll with same content returns None
        let second = watcher.poll().unwrap();
        assert!(second.is_none());
    }
}
