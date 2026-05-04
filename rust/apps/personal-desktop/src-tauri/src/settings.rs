//! Settings persistence and Tauri commands for the Advanced panel.
//!
//! Settings are stored as TOML at `~/.config/clipsync-personal/settings.toml`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::info;

// ── Settings types ───────────────────────────────────────────────

/// Per-device sync direction preference.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncDirection {
    #[default]
    Both,
    SendOnly,
    ReceiveOnly,
}

/// Settings for a specific paired device.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviceSettings {
    pub direction: SyncDirection,
}

/// Clipboard kind toggles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardKinds {
    pub text: bool,
    pub image: bool,
    pub files: bool,
}

impl Default for ClipboardKinds {
    fn default() -> Self {
        Self {
            text: true,
            image: true,
            files: true,
        }
    }
}

/// Notification preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub toast_on_receive: bool,
    pub sound: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            toast_on_receive: true,
            sound: false,
        }
    }
}

/// Network-related settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkSettings {
    /// Optional Tailscale fallback hostname.
    pub tailscale_hostname: Option<String>,
}

/// Root settings object.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub clipboard: ClipboardKinds,
    #[serde(default)]
    pub notifications: NotificationSettings,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub network: NetworkSettings,
    /// Per-device settings keyed by device ID.
    #[serde(default)]
    pub devices: HashMap<String, DeviceSettings>,
}

// ── Persistence ──────────────────────────────────────────────────

/// Return the path to `settings.toml`.
fn settings_path() -> PathBuf {
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clipsync-personal");
    fs::create_dir_all(&base).ok();
    base.join("settings.toml")
}

/// Load settings from disk, returning defaults if file doesn't exist.
pub fn load_settings() -> Settings {
    load_settings_from(&settings_path())
}

/// Load settings from a specific path (useful for testing).
pub fn load_settings_from(path: &Path) -> Settings {
    match fs::read_to_string(path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

/// Save settings to disk.
pub fn save_settings(settings: &Settings) {
    save_settings_to(settings, &settings_path());
}

/// Save settings to a specific path (useful for testing).
pub fn save_settings_to(settings: &Settings, path: &Path) {
    let contents = toml::to_string_pretty(settings).expect("failed to serialize settings");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(path, contents).expect("failed to write settings file");
}

// ── Managed state ────────────────────────────────────────────────

/// Thread-safe settings state managed by Tauri.
pub type SettingsState = Arc<Mutex<Settings>>;

/// Create the initial settings state.
pub fn init_settings_state() -> SettingsState {
    Arc::new(Mutex::new(load_settings()))
}

// ── Tauri commands ───────────────────────────────────────────────

/// Return the full settings object.
#[tauri::command]
pub async fn get_settings(state: tauri::State<'_, SettingsState>) -> Result<Settings, String> {
    let settings = state.lock().await;
    Ok(settings.clone())
}

/// Update a single setting by key path and JSON value.
///
/// Supported keys:
/// - `clipboard.text`, `clipboard.image`, `clipboard.files`
/// - `notifications.toast_on_receive`, `notifications.sound`
/// - `autostart`
/// - `network.tailscale_hostname`
/// - `devices.<device_id>.direction`
#[tauri::command]
pub async fn update_setting(
    key: String,
    value: String,
    state: tauri::State<'_, SettingsState>,
) -> Result<(), String> {
    let mut settings = state.lock().await;

    match key.as_str() {
        "clipboard.text" => {
            settings.clipboard.text = parse_bool(&value)?;
        }
        "clipboard.image" => {
            settings.clipboard.image = parse_bool(&value)?;
        }
        "clipboard.files" => {
            settings.clipboard.files = parse_bool(&value)?;
        }
        "notifications.toast_on_receive" => {
            settings.notifications.toast_on_receive = parse_bool(&value)?;
        }
        "notifications.sound" => {
            settings.notifications.sound = parse_bool(&value)?;
        }
        "autostart" => {
            settings.autostart = parse_bool(&value)?;
        }
        "network.tailscale_hostname" => {
            let trimmed = value.trim().to_string();
            settings.network.tailscale_hostname = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
        }
        k if k.starts_with("devices.") => {
            // Format: devices.<device_id>.direction
            let parts: Vec<&str> = k.splitn(3, '.').collect();
            if parts.len() == 3 && parts[2] == "direction" {
                let device_id = parts[1].to_string();
                let direction: SyncDirection =
                    serde_json::from_str(&format!("\"{}\"", value)).map_err(|e| e.to_string())?;
                let entry = settings.devices.entry(device_id).or_default();
                entry.direction = direction;
            } else {
                return Err(format!("unknown setting key: {}", key));
            }
        }
        _ => return Err(format!("unknown setting key: {}", key)),
    }

    save_settings(&settings);
    info!("setting updated: {} = {}", key, value);
    Ok(())
}

/// Reset all settings and delete paired peers.
#[tauri::command]
pub async fn reset_all(state: tauri::State<'_, SettingsState>) -> Result<(), String> {
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clipsync-personal");

    // Remove settings file
    let settings_file = base.join("settings.toml");
    if settings_file.exists() {
        fs::remove_file(&settings_file).map_err(|e| e.to_string())?;
    }

    // Remove peers file (from pairing module)
    let peers_file = base.join("peers.toml");
    if peers_file.exists() {
        fs::remove_file(&peers_file).map_err(|e| e.to_string())?;
    }

    // Remove device_id so a fresh one is generated on next launch
    let device_id_path = base.join("device_id");
    if device_id_path.exists() {
        fs::remove_file(&device_id_path).ok();
    }
    let legacy_base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clipsync");
    let legacy_id = legacy_base.join("device_id");
    if legacy_id.exists() {
        fs::remove_file(&legacy_id).ok();
    }

    // Reset in-memory state
    let mut settings = state.lock().await;
    *settings = Settings::default();

    info!("all settings and peers reset");
    Ok(())
}

/// Return the last 200 lines from the log file.
#[tauri::command]
pub async fn get_debug_log() -> Result<Vec<String>, String> {
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clipsync-personal");
    let log_path = base.join("clipsync.log");

    if !log_path.exists() {
        return Ok(vec!["No log file found.".to_string()]);
    }

    let content = fs::read_to_string(&log_path).map_err(|e| e.to_string())?;
    let lines: Vec<String> = content.lines().map(String::from).collect();
    let start = lines.len().saturating_sub(200);
    Ok(lines[start..].to_vec())
}

// ── Helpers ──────────────────────────────────────────────────────

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(format!("expected boolean, got: {}", value)),
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_settings() {
        let s = Settings::default();
        assert!(s.clipboard.text);
        assert!(s.clipboard.image);
        assert!(s.clipboard.files);
        assert!(s.notifications.toast_on_receive);
        assert!(!s.notifications.sound);
        assert!(!s.autostart);
        assert!(s.network.tailscale_hostname.is_none());
        assert!(s.devices.is_empty());
    }

    #[test]
    fn test_save_and_load() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();

        let mut settings = Settings::default();
        settings.clipboard.image = false;
        settings.autostart = true;
        settings.network.tailscale_hostname = Some("my-host".to_string());

        save_settings_to(&settings, &path);
        let loaded = load_settings_from(&path);

        assert!(!loaded.clipboard.image);
        assert!(loaded.autostart);
        assert_eq!(
            loaded.network.tailscale_hostname,
            Some("my-host".to_string())
        );
    }

    #[test]
    fn test_load_missing_file() {
        let path = PathBuf::from("/tmp/nonexistent-clipsync-settings.toml");
        let s = load_settings_from(&path);
        assert!(s.clipboard.text); // defaults
    }

    #[test]
    fn test_device_settings_default() {
        let d = DeviceSettings::default();
        assert_eq!(d.direction, SyncDirection::Both);
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_bool("true"), Ok(true));
        assert_eq!(parse_bool("false"), Ok(false));
        assert_eq!(parse_bool("1"), Ok(true));
        assert_eq!(parse_bool("0"), Ok(false));
        assert!(parse_bool("maybe").is_err());
    }

    #[test]
    fn test_roundtrip_toml() {
        let mut settings = Settings::default();
        settings.devices.insert(
            "device-abc".to_string(),
            DeviceSettings {
                direction: SyncDirection::SendOnly,
            },
        );

        let serialized = toml::to_string_pretty(&settings).unwrap();
        let deserialized: Settings = toml::from_str(&serialized).unwrap();
        assert_eq!(
            deserialized.devices["device-abc"].direction,
            SyncDirection::SendOnly
        );
    }
}
