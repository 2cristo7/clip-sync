//! ClipSync Personal Desktop — Tauri 2 library crate.

pub mod discovery;
pub mod mesh_hub;
pub mod notifications;
pub mod pairing;
pub mod peer_link;
pub mod settings;
pub mod tray;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use uuid::Uuid;

/// Return the path to the config directory, creating it if needed.
pub fn config_dir() -> PathBuf {
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clipsync");
    fs::create_dir_all(&base).expect("failed to create config directory");
    base
}

/// Load or generate a persistent device ID (UUID v4).
pub fn load_or_create_device_id(config_path: &Path) -> String {
    let id_file = config_path.join("device_id");
    if let Ok(id) = fs::read_to_string(&id_file) {
        let id = id.trim().to_string();
        if !id.is_empty() {
            return id;
        }
    }
    let id = Uuid::new_v4().to_string();
    fs::write(&id_file, &id).expect("failed to write device_id");
    id
}

/// Get the local hostname for mDNS advertisement.
pub fn local_hostname() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "unknown-host".to_string())
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! ClipSync is ready.", name)
}

pub fn run() {
    let cfg_path = config_dir();
    let device_id = load_or_create_device_id(&cfg_path);
    let hostname = local_hostname();

    let pairing_mgr = Arc::new(pairing::PairingManager::new(
        device_id,
        hostname,
        &cfg_path,
    ));

    let settings_state = settings::init_settings_state();

    tauri::Builder::default()
        .manage(pairing_mgr as pairing::PairingState_)
        .manage(settings_state)
        .invoke_handler(tauri::generate_handler![
            greet,
            tray::get_sync_status,
            tray::cmd_toggle_pause,
            tray::show_window,
            pairing::get_discovered_peers,
            pairing::initiate_pairing,
            pairing::confirm_pairing,
            pairing::get_paired_peers,
            pairing::add_manual_peer,
            settings::get_settings,
            settings::update_setting,
            settings::reset_all,
            settings::get_debug_log,
        ])
        .setup(|app| {
            tray::init_tray(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
