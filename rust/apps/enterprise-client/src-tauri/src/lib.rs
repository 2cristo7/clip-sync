use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipItem {
    pub text: String,
    pub timestamp: u64,
}

pub struct AppState {
    pub connection_status: Mutex<String>,
    pub policy_mode: Mutex<String>,
    pub sync_paused: Mutex<bool>,
    pub recent_clips: Mutex<Vec<ClipItem>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            connection_status: Mutex::new("disconnected".to_string()),
            policy_mode: Mutex::new("ReadWrite".to_string()),
            sync_paused: Mutex::new(false),
            recent_clips: Mutex::new(Vec::new()),
        }
    }
}

#[tauri::command]
fn get_connection_status(state: tauri::State<AppState>) -> String {
    state
        .connection_status
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

#[tauri::command]
fn get_policy_mode(state: tauri::State<AppState>) -> String {
    state
        .policy_mode
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

#[tauri::command]
fn get_sync_paused(state: tauri::State<AppState>) -> bool {
    *state
        .sync_paused
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[tauri::command]
fn toggle_sync(state: tauri::State<AppState>) -> bool {
    let mut paused = state
        .sync_paused
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *paused = !*paused;
    *paused
}

#[tauri::command]
fn get_recent_clips(state: tauri::State<AppState>) -> Vec<ClipItem> {
    state
        .recent_clips
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn build_tray_menu(app: &tauri::App) -> tauri::Result<()> {
    let status_item = MenuItemBuilder::with_id("status", "Status: Disconnected")
        .enabled(false)
        .build(app)?;
    let policy_item = MenuItemBuilder::with_id("policy", "Policy: ReadWrite")
        .enabled(false)
        .build(app)?;
    let separator1 = tauri::menu::PredefinedMenuItem::separator(app)?;
    let show_clips = MenuItemBuilder::with_id("show_clips", "Show recent clips").build(app)?;
    let pause_sync = MenuItemBuilder::with_id("pause_sync", "Pause sync").build(app)?;
    let separator2 = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&status_item)
        .item(&policy_item)
        .item(&separator1)
        .item(&show_clips)
        .item(&pause_sync)
        .item(&separator2)
        .item(&quit)
        .build()?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(move |app_handle, event| match event.id().as_ref() {
            "show_clips" => {
                if let Some(window) = app_handle.get_webview_window("clips") {
                    let _ = window.show();
                    let _ = window.set_focus();
                } else {
                    let _window = tauri::WebviewWindowBuilder::new(
                        app_handle,
                        "clips",
                        tauri::WebviewUrl::App("index.html".into()),
                    )
                    .title("Recent Clips")
                    .inner_size(400.0, 500.0)
                    .resizable(true)
                    .visible(true)
                    .build();
                }
            }
            "pause_sync" => {
                let state = app_handle.state::<AppState>();
                let mut paused = state
                    .sync_paused
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                *paused = !*paused;
                // Rebuild the menu to reflect new state will happen on next open
            }
            "quit" => {
                app_handle.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_connection_status,
            get_policy_mode,
            get_sync_paused,
            toggle_sync,
            get_recent_clips,
        ])
        .setup(|app| {
            build_tray_menu(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
