//! System tray icon and menu for ClipSync Personal Desktop.
//!
//! Provides a tray icon with status display and quick actions:
//! - Status line (Synced / Paused / Disconnected)
//! - Pause sync toggle
//! - Show window
//! - Quit

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};
use tracing::{info, warn};

/// Global pause state shared with other modules.
static SYNC_PAUSED: AtomicBool = AtomicBool::new(false);

/// Check whether sync is currently paused.
pub fn is_paused() -> bool {
    SYNC_PAUSED.load(Ordering::Relaxed)
}

/// Toggle the pause state, returning the new value.
pub fn toggle_pause() -> bool {
    let was = SYNC_PAUSED.load(Ordering::Relaxed);
    SYNC_PAUSED.store(!was, Ordering::Relaxed);
    !was
}

/// Sync status summary.
#[derive(Debug, Clone, serde::Serialize)]
pub enum SyncStatus {
    Synced,
    Paused,
    Disconnected,
}

/// Initialise the system tray icon and menu.
///
/// Should be called once during app setup (after `Builder::build`).
pub fn init_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let status_item = MenuItemBuilder::with_id("status", "Status: Synced")
        .enabled(false)
        .build(app)?;

    let pause_item = MenuItemBuilder::with_id("pause", "Pause sync").build(app)?;

    let show_item = MenuItemBuilder::with_id("show", "Show window").build(app)?;

    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&status_item)
        .separator()
        .item(&pause_item)
        .item(&show_item)
        .separator()
        .item(&quit_item)
        .build()?;

    let icon = Image::from_path("icons/32x32.png").unwrap_or_else(|_| {
        // Fallback: use included bytes at compile time.
        Image::from_bytes(include_bytes!("../icons/32x32.png")).expect("failed to load tray icon")
    });

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("ClipSync")
        .on_menu_event(move |app, event| {
            match event.id().as_ref() {
                "pause" => {
                    let now_paused = toggle_pause();
                    info!(paused = now_paused, "sync pause toggled");
                    // Update menu text.
                    if let Some(item) = menu.get("pause") {
                        if let Some(mi) = item.as_menuitem() {
                            let label = if now_paused {
                                "Resume sync"
                            } else {
                                "Pause sync"
                            };
                            let _ = mi.set_text(label);
                        }
                    }
                    // Update status line.
                    if let Some(item) = menu.get("status") {
                        if let Some(mi) = item.as_menuitem() {
                            let label = if now_paused {
                                "Status: Paused"
                            } else {
                                "Status: Synced"
                            };
                            let _ = mi.set_text(label);
                        }
                    }
                }
                "show" => {
                    if let Some(win) = app.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
                "quit" => {
                    info!("quit requested from tray");
                    app.exit(0);
                }
                other => {
                    warn!(id = %other, "unknown tray menu event");
                }
            }
        })
        .build(app)?;

    info!("system tray initialised");
    Ok(())
}

/// Tauri command: get current sync status.
#[tauri::command]
pub fn get_sync_status() -> String {
    if is_paused() {
        "paused".to_string()
    } else {
        "synced".to_string()
    }
}

/// Tauri command: toggle pause/resume sync.
#[tauri::command]
pub fn cmd_toggle_pause() -> bool {
    toggle_pause()
}

/// Tauri command: show and focus the main window.
#[tauri::command]
pub fn show_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}
