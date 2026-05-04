//! File broadcast module for the ClipSync personal mesh.
//!
//! Provides Tauri commands for sending files to peers via drag-drop,
//! receiving files, and managing the received files cache.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

/// Maximum file size allowed for broadcast (50 MB).
pub const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024;

/// How long to keep received files before cleanup (24 hours).
const RECEIVED_FILE_TTL_SECS: u64 = 24 * 60 * 60;

/// A file broadcast frame sent over the mesh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastFile {
    /// Unique ID for this file transfer.
    pub id: String,
    /// Original file name.
    pub name: String,
    /// MIME type (best effort).
    pub mime: String,
    /// File size in bytes.
    pub size: u64,
    /// Base64-encoded file data.
    pub data_b64: String,
    /// Device ID of the sender.
    pub sender_device_id: String,
    /// Sender device name (for display).
    pub sender_device_name: String,
    /// Timestamp (ms since epoch).
    pub timestamp: u64,
}

/// Metadata for a received file (without the full data in memory).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivedFileInfo {
    /// Unique ID.
    pub id: String,
    /// Original file name.
    pub name: String,
    /// MIME type.
    pub mime: String,
    /// File size in bytes.
    pub size: u64,
    /// Sender device name.
    pub sender_device_name: String,
    /// Timestamp received (ms since epoch).
    pub received_at: u64,
    /// Path where the file is stored temporarily.
    pub stored_path: String,
}

/// Errors from broadcast operations.
#[derive(Debug, thiserror::Error)]
pub enum BroadcastError {
    #[error("file too large: {0} bytes exceeds 50 MB limit")]
    FileTooLarge(u64),
    #[error("file not found: {0}")]
    FileNotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("file ID not found: {0}")]
    ReceivedFileNotFound(String),
}

impl Serialize for BroadcastError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Manages received files storage and cleanup.
pub struct FileReceiver {
    /// Directory where received files are stored.
    storage_dir: PathBuf,
    /// Index of received files.
    files: Arc<Mutex<HashMap<String, ReceivedFileInfo>>>,
}

impl FileReceiver {
    /// Create a new file receiver with the given storage directory.
    pub fn new(storage_dir: PathBuf) -> Self {
        fs::create_dir_all(&storage_dir).ok();
        Self {
            storage_dir,
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Store a received broadcast file to disk.
    pub async fn store(&self, file: &BroadcastFile) -> Result<ReceivedFileInfo, BroadcastError> {
        let dest = self.storage_dir.join(&file.id).join(&file.name);
        fs::create_dir_all(dest.parent().unwrap())?;

        let data = base64::engine::general_purpose::STANDARD
            .decode(&file.data_b64)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        fs::write(&dest, &data)?;

        let info = ReceivedFileInfo {
            id: file.id.clone(),
            name: file.name.clone(),
            mime: file.mime.clone(),
            size: file.size,
            sender_device_name: file.sender_device_name.clone(),
            received_at: now_ms(),
            stored_path: dest.to_string_lossy().to_string(),
        };

        self.files
            .lock()
            .await
            .insert(file.id.clone(), info.clone());
        info!(file_id = %file.id, name = %file.name, "file received and stored");

        Ok(info)
    }

    /// List all received files.
    pub async fn list(&self) -> Vec<ReceivedFileInfo> {
        let files = self.files.lock().await;
        let mut list: Vec<_> = files.values().cloned().collect();
        list.sort_by_key(|f| std::cmp::Reverse(f.received_at));
        list
    }

    /// Get info for a specific file.
    pub async fn get(&self, file_id: &str) -> Option<ReceivedFileInfo> {
        self.files.lock().await.get(file_id).cloned()
    }

    /// Save a received file to a user-chosen destination.
    pub async fn save_to(&self, file_id: &str, destination: &Path) -> Result<(), BroadcastError> {
        let info = self
            .files
            .lock()
            .await
            .get(file_id)
            .cloned()
            .ok_or_else(|| BroadcastError::ReceivedFileNotFound(file_id.to_string()))?;

        let source = PathBuf::from(&info.stored_path);
        if !source.exists() {
            return Err(BroadcastError::FileNotFound(info.stored_path));
        }

        fs::copy(&source, destination)?;
        info!(file_id = %file_id, dest = %destination.display(), "file saved to destination");
        Ok(())
    }

    /// Remove files older than 24 hours.
    pub async fn cleanup_stale(&self) {
        let cutoff = now_ms().saturating_sub(RECEIVED_FILE_TTL_SECS * 1000);
        let mut files = self.files.lock().await;

        let stale_ids: Vec<String> = files
            .iter()
            .filter(|(_, info)| info.received_at < cutoff)
            .map(|(id, _)| id.clone())
            .collect();

        for id in stale_ids {
            if let Some(info) = files.remove(&id) {
                let path = PathBuf::from(&info.stored_path);
                if path.exists() {
                    fs::remove_file(&path).ok();
                }
                // Try removing the parent dir (the file-id dir)
                if let Some(parent) = path.parent() {
                    fs::remove_dir(parent).ok();
                }
                info!(file_id = %id, "stale received file cleaned up");
            }
        }
    }
}

/// Validate file size against the 50 MB limit.
pub fn validate_file_size(size: u64) -> Result<(), BroadcastError> {
    if size > MAX_FILE_SIZE {
        return Err(BroadcastError::FileTooLarge(size));
    }
    Ok(())
}

/// Read a file and create a broadcast frame.
pub fn prepare_broadcast(
    file_path: &Path,
    sender_device_id: &str,
    sender_device_name: &str,
) -> Result<BroadcastFile, BroadcastError> {
    if !file_path.exists() {
        return Err(BroadcastError::FileNotFound(
            file_path.to_string_lossy().to_string(),
        ));
    }

    let metadata = fs::metadata(file_path)?;
    validate_file_size(metadata.len())?;

    let data = fs::read(file_path)?;
    let data_b64 = base64::engine::general_purpose::STANDARD.encode(&data);

    let name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let mime = mime_from_extension(file_path);

    Ok(BroadcastFile {
        id: Uuid::new_v4().to_string(),
        name,
        mime,
        size: metadata.len(),
        data_b64,
        sender_device_id: sender_device_id.to_string(),
        sender_device_name: sender_device_name.to_string(),
        timestamp: now_ms(),
    })
}

/// Simple MIME type detection from file extension.
fn mime_from_extension(path: &Path) -> String {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "txt" => "text/plain",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "zip" => "application/zip",
        "json" => "application/json",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        "doc" | "docx" => "application/msword",
        "xls" | "xlsx" => "application/vnd.ms-excel",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Current time in milliseconds since epoch.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Received files storage directory.
pub fn received_files_dir() -> PathBuf {
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clipsync-personal")
        .join("received");
    fs::create_dir_all(&base).ok();
    base
}

// ── Tauri Commands ──────────────────────────────────────────────────────

/// Tauri command: send a file to selected peers.
#[tauri::command]
pub async fn send_file(file_path: String, peer_ids: Vec<String>) -> Result<String, String> {
    let path = PathBuf::from(&file_path);

    let broadcast = prepare_broadcast(&path, "local", "this-device").map_err(|e| e.to_string())?;

    let file_id = broadcast.id.clone();
    let payload = serde_json::to_string(&broadcast).map_err(|e| e.to_string())?;

    info!(
        file = %broadcast.name,
        size = broadcast.size,
        peers = ?peer_ids,
        "file broadcast prepared"
    );

    // In production, this would send via MeshHub to selected peers.
    // For now, we validate and return the file ID.
    let _ = peer_ids;
    let _ = payload;

    Ok(file_id)
}

/// Tauri command: get list of recently received files.
#[tauri::command]
pub async fn get_received_files() -> Result<Vec<ReceivedFileInfo>, String> {
    let receiver = FileReceiver::new(received_files_dir());
    Ok(receiver.list().await)
}

/// Tauri command: reveal a file in the system file manager.
#[tauri::command]
pub async fn reveal_file(file_path: String) -> Result<(), String> {
    let path = PathBuf::from(&file_path);
    if !path.exists() {
        return Err("File not found".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg("/select,")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path.parent().unwrap_or(&path))
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Tauri command: save a received file to a user-chosen destination.
#[tauri::command]
pub async fn save_file(file_id: String, destination: String) -> Result<(), String> {
    let receiver = FileReceiver::new(received_files_dir());
    let dest = PathBuf::from(&destination);
    receiver
        .save_to(&file_id, &dest)
        .await
        .map_err(|e| e.to_string())
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn validate_file_size_within_limit() {
        assert!(validate_file_size(1024).is_ok());
        assert!(validate_file_size(MAX_FILE_SIZE).is_ok());
    }

    #[test]
    fn validate_file_size_exceeds_limit() {
        let result = validate_file_size(MAX_FILE_SIZE + 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, BroadcastError::FileTooLarge(_)));
    }

    #[test]
    fn prepare_broadcast_small_file() {
        let mut tmp = NamedTempFile::new().unwrap();
        write!(tmp, "hello world").unwrap();

        let result = prepare_broadcast(tmp.path(), "dev-1", "My Mac");
        assert!(result.is_ok());

        let frame = result.unwrap();
        assert_eq!(frame.size, 11);
        assert_eq!(frame.sender_device_id, "dev-1");
        assert_eq!(frame.sender_device_name, "My Mac");
        assert!(!frame.id.is_empty());
        assert!(!frame.data_b64.is_empty());
    }

    #[test]
    fn prepare_broadcast_file_not_found() {
        let result = prepare_broadcast(Path::new("/nonexistent/file.txt"), "dev-1", "Mac");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BroadcastError::FileNotFound(_)
        ));
    }

    #[test]
    fn prepare_broadcast_too_large() {
        // We don't actually create a 50MB+ file — just test validate_file_size
        let err = validate_file_size(60 * 1024 * 1024);
        assert!(err.is_err());
    }

    #[test]
    fn mime_detection() {
        assert_eq!(mime_from_extension(Path::new("photo.png")), "image/png");
        assert_eq!(mime_from_extension(Path::new("doc.pdf")), "application/pdf");
        assert_eq!(
            mime_from_extension(Path::new("data.xyz")),
            "application/octet-stream"
        );
    }

    #[test]
    fn broadcast_frame_serde_round_trip() {
        let frame = BroadcastFile {
            id: "test-id".to_string(),
            name: "file.txt".to_string(),
            mime: "text/plain".to_string(),
            size: 5,
            data_b64: base64::engine::general_purpose::STANDARD.encode(b"hello"),
            sender_device_id: "dev-1".to_string(),
            sender_device_name: "My Mac".to_string(),
            timestamp: 1_000_000,
        };

        let json = serde_json::to_string(&frame).unwrap();
        let parsed: BroadcastFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-id");
        assert_eq!(parsed.name, "file.txt");
        assert_eq!(parsed.size, 5);
    }

    #[tokio::test]
    async fn file_receiver_store_and_list() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let receiver = FileReceiver::new(tmp_dir.path().to_path_buf());

        let frame = BroadcastFile {
            id: "recv-1".to_string(),
            name: "test.txt".to_string(),
            mime: "text/plain".to_string(),
            size: 5,
            data_b64: base64::engine::general_purpose::STANDARD.encode(b"hello"),
            sender_device_id: "peer-1".to_string(),
            sender_device_name: "Peer Mac".to_string(),
            timestamp: 1_000_000,
        };

        let info = receiver.store(&frame).await.unwrap();
        assert_eq!(info.name, "test.txt");
        assert!(PathBuf::from(&info.stored_path).exists());

        let list = receiver.list().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "recv-1");
    }

    #[tokio::test]
    async fn file_receiver_save_to() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let receiver = FileReceiver::new(tmp_dir.path().to_path_buf());

        let frame = BroadcastFile {
            id: "recv-2".to_string(),
            name: "data.bin".to_string(),
            mime: "application/octet-stream".to_string(),
            size: 3,
            data_b64: base64::engine::general_purpose::STANDARD.encode(b"abc"),
            sender_device_id: "peer-2".to_string(),
            sender_device_name: "Peer Win".to_string(),
            timestamp: 2_000_000,
        };

        receiver.store(&frame).await.unwrap();

        let dest = tmp_dir.path().join("saved_data.bin");
        receiver.save_to("recv-2", &dest).await.unwrap();
        assert!(dest.exists());
        assert_eq!(fs::read(&dest).unwrap(), b"abc");
    }

    #[tokio::test]
    async fn file_receiver_not_found() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let receiver = FileReceiver::new(tmp_dir.path().to_path_buf());

        let result = receiver.save_to("nonexistent", Path::new("/tmp/out")).await;
        assert!(result.is_err());
    }
}
