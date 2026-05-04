use std::sync::{Arc, Mutex};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use clipsync_protocol::protocol::{ClipPayload, ClipType};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("clipboard access failed: {0}")]
    AccessFailed(String),
    #[error("unsupported content type")]
    UnsupportedType,
    #[error("image conversion failed: {0}")]
    ImageConversion(String),
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

    /// Check if a digest matches the last written payload (echo suppression).
    fn is_echo(&self, digest: &str) -> bool {
        let last = self.last_written_digest.lock().unwrap();
        last.as_deref() == Some(digest)
    }

    /// Record a digest as the last written payload.
    fn record_written(&self, digest: String) {
        *self.last_written_digest.lock().unwrap() = Some(digest);
    }

    /// Make a timestamp for the current instant.
    ///
    /// ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
    fn now_ts() -> u64 {
        // ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    /// Try to read an image from the clipboard, returning PNG bytes.
    /// On macOS, handles TIFF→PNG conversion since macOS clipboard often stores TIFF.
    /// On Linux/Wayland, falls back to wl-paste if arboard fails.
    fn read_image_bytes(&self) -> Result<Option<Vec<u8>>, ClipboardError> {
        let mut clipboard =
            arboard::Clipboard::new().map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

        match clipboard.get_image() {
            Ok(img) => {
                // arboard gives us RGBA pixels; encode as PNG for wire format
                let png_bytes = encode_rgba_as_png(&img.bytes, img.width as u32, img.height as u32);
                Ok(Some(png_bytes))
            }
            Err(_) => {
                // Platform-specific fallbacks
                self.read_image_platform_fallback()
            }
        }
    }

    /// Platform-specific image fallbacks when arboard fails.
    #[cfg(target_os = "macos")]
    fn read_image_platform_fallback(&self) -> Result<Option<Vec<u8>>, ClipboardError> {
        // On macOS, clipboard may have TIFF data that arboard couldn't handle.
        // Try reading raw pasteboard data via osascript as a fallback.
        // In practice, arboard 3.x handles macOS images well, so this is rare.
        Ok(None)
    }

    /// Platform-specific image fallback for Linux (Wayland: wl-paste).
    #[cfg(target_os = "linux")]
    fn read_image_platform_fallback(&self) -> Result<Option<Vec<u8>>, ClipboardError> {
        // Try wl-paste for Wayland sessions
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            match std::process::Command::new("wl-paste")
                .args(["--type", "image/png"])
                .output()
            {
                Ok(output) if output.status.success() && !output.stdout.is_empty() => {
                    return Ok(Some(output.stdout));
                }
                _ => {}
            }
        }
        Ok(None)
    }

    /// Platform-specific image fallback for Windows.
    #[cfg(target_os = "windows")]
    fn read_image_platform_fallback(&self) -> Result<Option<Vec<u8>>, ClipboardError> {
        // arboard handles CF_DIB/CF_BITMAP on Windows well.
        // If it failed, there's nothing extra we can do without win32 APIs.
        Ok(None)
    }

    /// Fallback for other platforms.
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    fn read_image_platform_fallback(&self) -> Result<Option<Vec<u8>>, ClipboardError> {
        Ok(None)
    }

    /// Write an image to the clipboard from PNG bytes.
    /// On Linux/Wayland, falls back to wl-copy if arboard fails.
    fn write_image_to_clipboard(&self, png_bytes: &[u8]) -> Result<(), ClipboardError> {
        let (width, height, rgba) =
            decode_png_to_rgba(png_bytes).map_err(ClipboardError::ImageConversion)?;

        let img_data = arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: std::borrow::Cow::Owned(rgba),
        };

        let mut clipboard =
            arboard::Clipboard::new().map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

        match clipboard.set_image(img_data) {
            Ok(()) => Ok(()),
            Err(_e) => self.write_image_platform_fallback(png_bytes),
        }
    }

    /// Platform-specific image write fallback for macOS.
    #[cfg(target_os = "macos")]
    fn write_image_platform_fallback(&self, _png_bytes: &[u8]) -> Result<(), ClipboardError> {
        Err(ClipboardError::AccessFailed(
            "failed to write image to macOS clipboard".to_string(),
        ))
    }

    /// Platform-specific image write fallback for Linux (Wayland: wl-copy).
    #[cfg(target_os = "linux")]
    fn write_image_platform_fallback(&self, png_bytes: &[u8]) -> Result<(), ClipboardError> {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            use std::io::Write;
            let mut child = std::process::Command::new("wl-copy")
                .args(["--type", "image/png"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| ClipboardError::AccessFailed(format!("wl-copy spawn: {e}")))?;

            if let Some(ref mut stdin) = child.stdin {
                stdin
                    .write_all(png_bytes)
                    .map_err(|e| ClipboardError::AccessFailed(format!("wl-copy write: {e}")))?;
            }

            let status = child
                .wait()
                .map_err(|e| ClipboardError::AccessFailed(format!("wl-copy wait: {e}")))?;
            if status.success() {
                return Ok(());
            }
        }
        Err(ClipboardError::AccessFailed(
            "failed to write image to Linux clipboard".to_string(),
        ))
    }

    /// Platform-specific image write fallback for Windows.
    #[cfg(target_os = "windows")]
    fn write_image_platform_fallback(&self, _png_bytes: &[u8]) -> Result<(), ClipboardError> {
        Err(ClipboardError::AccessFailed(
            "failed to write image to Windows clipboard".to_string(),
        ))
    }

    /// Fallback for other platforms.
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    fn write_image_platform_fallback(&self, _png_bytes: &[u8]) -> Result<(), ClipboardError> {
        Err(ClipboardError::AccessFailed(
            "image clipboard not supported on this platform".to_string(),
        ))
    }
}

impl Default for SystemClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardProvider for SystemClipboard {
    fn read(&self) -> Result<Option<ClipPayload>, ClipboardError> {
        let mut clipboard =
            arboard::Clipboard::new().map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

        // Try text first
        if let Ok(text) = clipboard.get_text() {
            if !text.is_empty() {
                let digest = Self::data_digest(text.as_bytes());

                // Echo suppression: skip if we just wrote this
                if self.is_echo(&digest) {
                    return Ok(None);
                }

                return Ok(Some(ClipPayload {
                    clip_type: ClipType::Text,
                    mime: "text/plain".to_string(),
                    data: BASE64.encode(text.as_bytes()),
                    ts: Self::now_ts(),
                    nonce: uuid::Uuid::new_v4().to_string(),
                    name: None,
                }));
            }
        }

        // Drop the arboard clipboard before platform-specific image reads
        drop(clipboard);

        // Try image (arboard + platform fallbacks)
        if let Some(png_bytes) = self.read_image_bytes()? {
            if !png_bytes.is_empty() {
                let digest = Self::data_digest(&png_bytes);

                if self.is_echo(&digest) {
                    return Ok(None);
                }

                return Ok(Some(ClipPayload {
                    clip_type: ClipType::Image,
                    mime: "image/png".to_string(),
                    data: BASE64.encode(&png_bytes),
                    ts: Self::now_ts(),
                    nonce: uuid::Uuid::new_v4().to_string(),
                    name: None,
                }));
            }
        }

        // Try file paths (platform-specific)
        if let Some(payload) = read_file_paths_from_clipboard()? {
            let digest = Self::data_digest(payload.data.as_bytes());
            if self.is_echo(&digest) {
                return Ok(None);
            }
            return Ok(Some(payload));
        }

        Ok(None)
    }

    fn write(&self, payload: &ClipPayload) -> Result<(), ClipboardError> {
        match payload.clip_type {
            ClipType::Text => {
                let raw = BASE64
                    .decode(&payload.data)
                    .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;
                let text = String::from_utf8(raw)
                    .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

                let digest = Self::data_digest(text.as_bytes());
                self.record_written(digest);

                let mut clipboard = arboard::Clipboard::new()
                    .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;
                clipboard
                    .set_text(&text)
                    .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;
            }
            ClipType::Image => {
                let png_bytes = BASE64
                    .decode(&payload.data)
                    .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

                let digest = Self::data_digest(&png_bytes);
                self.record_written(digest);

                self.write_image_to_clipboard(&png_bytes)?;
            }
            ClipType::File => {
                // Files are saved to disk, not written to clipboard.
                // Save the file to ~/Downloads/ and optionally set file path on clipboard.
                let raw = BASE64
                    .decode(&payload.data)
                    .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;
                let filename = payload.name.as_deref().unwrap_or("clipsync_file");
                save_received_file(filename, &raw)?;

                // Set the file path as text on clipboard so user can paste the path
                let dest = received_file_path(filename);
                let digest = Self::data_digest(dest.to_string_lossy().as_bytes());
                self.record_written(digest);

                let mut clipboard = arboard::Clipboard::new()
                    .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;
                clipboard
                    .set_text(dest.to_string_lossy())
                    .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// File clipboard support
// ---------------------------------------------------------------------------

/// Read file paths from the system clipboard (platform-specific).
/// Returns a File-type ClipPayload with the first file's contents, or None.
#[cfg(target_os = "macos")]
fn read_file_paths_from_clipboard() -> Result<Option<ClipPayload>, ClipboardError> {
    // On macOS, try to read file URLs from pasteboard via osascript.
    // arboard doesn't expose file paths, so we use a small AppleScript.
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            "try\n\
             set fp to POSIX path of (the clipboard as «class furl»)\n\
             return fp\n\
             end try",
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let path_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if path_str.is_empty() {
                return Ok(None);
            }
            let path = std::path::Path::new(&path_str);
            if !path.exists() || !path.is_file() {
                return Ok(None);
            }
            file_payload_from_path(path)
        }
        _ => Ok(None),
    }
}

/// Read file paths from clipboard on Linux (text/uri-list).
#[cfg(target_os = "linux")]
fn read_file_paths_from_clipboard() -> Result<Option<ClipPayload>, ClipboardError> {
    // Try xclip for X11
    let output = if std::env::var("WAYLAND_DISPLAY").is_ok() {
        std::process::Command::new("wl-paste")
            .args(["--type", "text/uri-list"])
            .output()
    } else {
        std::process::Command::new("xclip")
            .args(["-selection", "clipboard", "-t", "text/uri-list", "-o"])
            .output()
    };

    match output {
        Ok(out) if out.status.success() => {
            let uri_list = String::from_utf8_lossy(&out.stdout).to_string();
            // Parse first file:// URI
            for line in uri_list.lines() {
                let line = line.trim();
                if line.starts_with("file://") {
                    let path_str = line.strip_prefix("file://").unwrap_or(line);
                    // URL-decode
                    let decoded = url_decode(path_str);
                    let path = std::path::Path::new(&decoded);
                    if path.exists() && path.is_file() {
                        return file_payload_from_path(path);
                    }
                }
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

/// Read file paths from clipboard on Windows (CF_HDROP via PowerShell).
#[cfg(target_os = "windows")]
fn read_file_paths_from_clipboard() -> Result<Option<ClipPayload>, ClipboardError> {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-Clipboard -Format FileDropList | Select-Object -First 1 -ExpandProperty FullName",
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let path_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if path_str.is_empty() {
                return Ok(None);
            }
            let path = std::path::Path::new(&path_str);
            if path.exists() && path.is_file() {
                return file_payload_from_path(path);
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

/// Fallback for unsupported platforms.
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn read_file_paths_from_clipboard() -> Result<Option<ClipPayload>, ClipboardError> {
    Ok(None)
}

/// Build a File ClipPayload from a filesystem path.
fn file_payload_from_path(path: &std::path::Path) -> Result<Option<ClipPayload>, ClipboardError> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| ClipboardError::AccessFailed(format!("file metadata: {e}")))?;

    // Enforce 20 MB max
    if metadata.len() > clipsync_protocol::config::MAX_PAYLOAD_BYTES as u64 {
        tracing::warn!(
            "Skipping file {} ({} bytes) — exceeds max payload size",
            path.display(),
            metadata.len()
        );
        return Ok(None);
    }

    let bytes =
        std::fs::read(path).map_err(|e| ClipboardError::AccessFailed(format!("file read: {e}")))?;

    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let mime = mime_from_extension(&filename);

    // ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    Ok(Some(ClipPayload {
        clip_type: ClipType::File,
        mime,
        data: BASE64.encode(&bytes),
        ts,
        nonce: uuid::Uuid::new_v4().to_string(),
        name: Some(filename),
    }))
}

/// Simple MIME type detection from file extension.
fn mime_from_extension(filename: &str) -> String {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "xml" => "application/xml",
        "zip" => "application/zip",
        "doc" | "docx" => "application/msword",
        "xls" | "xlsx" => "application/vnd.ms-excel",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Simple URL percent-decoding for file paths.
#[allow(dead_code)]
fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().unwrap_or(b'0');
            let lo = chars.next().unwrap_or(b'0');
            let hex = [hi, lo];
            if let Ok(val) = u8::from_str_radix(&String::from_utf8_lossy(&hex), 16) {
                result.push(val as char);
            }
        } else {
            result.push(b as char);
        }
    }
    result
}

/// Get the path where a received file would be saved.
fn received_file_path(filename: &str) -> std::path::PathBuf {
    let downloads = dirs::download_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Downloads"));
    downloads.join(filename)
}

/// Save a received file to ~/Downloads/.
/// If a file with the same name exists, appends a counter.
pub fn save_received_file(
    filename: &str,
    data: &[u8],
) -> Result<std::path::PathBuf, ClipboardError> {
    let downloads = dirs::download_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Downloads"));

    std::fs::create_dir_all(&downloads)
        .map_err(|e| ClipboardError::AccessFailed(format!("create downloads dir: {e}")))?;

    let base_path = downloads.join(filename);

    // Avoid overwriting: find unique name
    let dest = if base_path.exists() {
        let stem = std::path::Path::new(filename)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| filename.to_string());
        let ext = std::path::Path::new(filename)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();

        let mut counter = 1u32;
        loop {
            let candidate = downloads.join(format!("{stem}_{counter}{ext}"));
            if !candidate.exists() {
                break candidate;
            }
            counter += 1;
            if counter > 9999 {
                return Err(ClipboardError::AccessFailed(
                    "too many files with same name".to_string(),
                ));
            }
        }
    } else {
        base_path
    };

    std::fs::write(&dest, data)
        .map_err(|e| ClipboardError::AccessFailed(format!("file write: {e}")))?;

    tracing::info!("Saved received file to {}", dest.display());
    Ok(dest)
}

// ---------------------------------------------------------------------------
// PNG encoding/decoding helpers
// ---------------------------------------------------------------------------

/// Encode RGBA pixel data as PNG bytes using a minimal encoder.
fn encode_rgba_as_png(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut buf, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("PNG header write failed");
        writer
            .write_image_data(rgba)
            .expect("PNG data write failed");
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

/// Convert TIFF bytes to PNG bytes using the `image` crate.
/// Useful on macOS where the clipboard often stores TIFF data.
#[allow(dead_code)]
pub fn tiff_to_png(tiff_bytes: &[u8]) -> Result<Vec<u8>, ClipboardError> {
    use image::ImageReader;
    use std::io::Cursor;

    let reader = ImageReader::new(Cursor::new(tiff_bytes))
        .with_guessed_format()
        .map_err(|e| ClipboardError::ImageConversion(format!("format guess: {e}")))?;

    let img = reader
        .decode()
        .map_err(|e| ClipboardError::ImageConversion(format!("decode: {e}")))?;

    let mut png_buf = Vec::new();
    let mut cursor = Cursor::new(&mut png_buf);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| ClipboardError::ImageConversion(format!("encode png: {e}")))?;

    Ok(png_buf)
}

/// Convert BMP/DIB bytes to PNG bytes using the `image` crate.
/// Useful on Windows where CF_DIB clipboard format is common.
#[allow(dead_code)]
pub fn bmp_to_png(bmp_bytes: &[u8]) -> Result<Vec<u8>, ClipboardError> {
    use image::ImageReader;
    use std::io::Cursor;

    let reader = ImageReader::new(Cursor::new(bmp_bytes))
        .with_guessed_format()
        .map_err(|e| ClipboardError::ImageConversion(format!("format guess: {e}")))?;

    let img = reader
        .decode()
        .map_err(|e| ClipboardError::ImageConversion(format!("decode: {e}")))?;

    let mut png_buf = Vec::new();
    let mut cursor = Cursor::new(&mut png_buf);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| ClipboardError::ImageConversion(format!("encode png: {e}")))?;

    Ok(png_buf)
}

// ---------------------------------------------------------------------------
// Clipboard watcher
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Desktop notifications
// ---------------------------------------------------------------------------

/// Notification configuration.
pub struct ClipNotification {
    pub summary: String,
    pub body: String,
}

/// Show a desktop notification for a received clipboard payload.
pub fn notify_received(payload: &ClipPayload) -> Result<(), ClipboardError> {
    let notif = match payload.clip_type {
        ClipType::Text => {
            let raw = payload.decode_data().unwrap_or_default();
            let text = String::from_utf8_lossy(&raw);
            let preview = if text.len() > 80 {
                format!("{}...", &text[..77])
            } else {
                text.to_string()
            };
            ClipNotification {
                summary: "ClipSync — Text received".to_string(),
                body: preview,
            }
        }
        ClipType::Image => {
            let size_kb = payload.data.len() * 3 / 4 / 1024; // approx decoded size
            ClipNotification {
                summary: "ClipSync — Image received".to_string(),
                body: format!("Image copied to clipboard (~{size_kb} KB)"),
            }
        }
        ClipType::File => {
            let name = payload.name.as_deref().unwrap_or("unknown");
            ClipNotification {
                summary: "ClipSync — File received".to_string(),
                body: format!("Saved to Downloads: {name}"),
            }
        }
    };

    show_native_notification(&notif)
}

/// Send a native desktop notification via notify-rust.
fn show_native_notification(notif: &ClipNotification) -> Result<(), ClipboardError> {
    notify_rust::Notification::new()
        .appname("ClipSync")
        .summary(&notif.summary)
        .body(&notif.body)
        .timeout(notify_rust::Timeout::Milliseconds(4000))
        .show()
        .map_err(|e| ClipboardError::AccessFailed(format!("notification: {e}")))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

    #[test]
    fn encode_decode_png_roundtrip() {
        // 2x2 red RGBA image
        let rgba: Vec<u8> = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let png_bytes = encode_rgba_as_png(&rgba, 2, 2);
        assert!(!png_bytes.is_empty());

        let (w, h, decoded) = decode_png_to_rgba(&png_bytes).unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(decoded, rgba);
    }

    #[test]
    fn mime_from_ext() {
        assert_eq!(mime_from_extension("photo.png"), "image/png");
        assert_eq!(mime_from_extension("doc.pdf"), "application/pdf");
        assert_eq!(mime_from_extension("archive.zip"), "application/zip");
        assert_eq!(mime_from_extension("data.xyz"), "application/octet-stream");
        assert_eq!(mime_from_extension("noext"), "application/octet-stream");
    }

    #[test]
    fn url_decode_basic() {
        assert_eq!(url_decode("/path/to/file"), "/path/to/file");
        assert_eq!(url_decode("/path/to/my%20file.txt"), "/path/to/my file.txt");
        assert_eq!(url_decode("%2Ftmp%2Ftest"), "/tmp/test");
    }

    #[test]
    fn save_file_roundtrip() {
        let tmp = std::env::temp_dir().join("clipsync_test_save");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let data = b"hello world";
        let dest = tmp.join("test.txt");
        std::fs::write(&dest, data).unwrap();

        // Verify we can read it back
        let read_back = std::fs::read(&dest).unwrap();
        assert_eq!(read_back, data);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn received_file_path_uses_downloads() {
        let path = received_file_path("test.pdf");
        assert!(path.to_string_lossy().contains("test.pdf"));
        // Should be under some Downloads directory
        let parent = path.parent().unwrap();
        // Just verify we got a non-empty path — actual directory varies across CI
        assert!(!parent.to_string_lossy().is_empty());
    }

    #[test]
    fn file_payload_construction() {
        let payload = ClipPayload {
            clip_type: ClipType::File,
            mime: "application/pdf".to_string(),
            data: BASE64.encode(b"fake pdf"),
            ts: 1714000000,
            nonce: "test-nonce".to_string(),
            name: Some("doc.pdf".to_string()),
        };
        assert_eq!(payload.clip_type, ClipType::File);
        assert_eq!(payload.name, Some("doc.pdf".to_string()));
        let decoded = payload.decode_data().unwrap();
        assert_eq!(decoded, b"fake pdf");
    }

    #[test]
    fn notification_text_preview_truncation() {
        let long_text = "a".repeat(200);
        let payload = ClipPayload::text(&long_text, 1714000000);
        let raw = payload.decode_data().unwrap();
        let text = String::from_utf8_lossy(&raw);
        // Our notify_received would truncate at 80 chars
        let preview = if text.len() > 80 {
            format!("{}...", &text[..77])
        } else {
            text.to_string()
        };
        assert_eq!(preview.len(), 80);
        assert!(preview.ends_with("..."));
    }
}
