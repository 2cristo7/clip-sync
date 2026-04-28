use std::sync::Arc;

use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use clipsync_core::config::VERSION;

use crate::AppState;

/// Menu item IDs for event handling.
mod ids {
    pub const PAIR: &str = "pair";
    pub const QUIT: &str = "quit";
}

/// Build the system tray icon and menu.
///
/// This must be called from the main thread on macOS.
/// Returns the TrayIcon guard (drop it to remove the icon).
pub fn build_tray(state: Arc<AppState>) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let menu = Menu::new();

    // Status label (disabled, informational)
    let status_item = MenuItem::with_id(
        "status",
        format!("ClipSync v{VERSION} — running"),
        false, // disabled — just a label
        None,
    );
    menu.append(&status_item)?;

    menu.append(&PredefinedMenuItem::separator())?;

    // Start pairing
    let pair_item = MenuItem::with_id(ids::PAIR, "Start Pairing…", true, None);
    menu.append(&pair_item)?;

    // Connected devices submenu (initially empty)
    let devices_submenu = Submenu::new("Connected Devices", true);
    let no_devices = MenuItem::with_id("no-devices", "(none)", false, None);
    devices_submenu.append(&no_devices)?;
    menu.append(&devices_submenu)?;

    menu.append(&PredefinedMenuItem::separator())?;

    // Quit
    let quit_item = MenuItem::with_id(ids::QUIT, "Quit ClipSync", true, None);
    menu.append(&quit_item)?;

    // Create a simple 16x16 RGBA icon (green square)
    let icon = create_default_icon();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(format!("ClipSync v{VERSION}"))
        .with_icon(icon)
        .build()?;

    // Spawn event handler
    let state_clone = state;
    std::thread::spawn(move || {
        let receiver = MenuEvent::receiver();
        loop {
            if let Ok(event) = receiver.recv() {
                match event.id().0.as_str() {
                    ids::PAIR => {
                        handle_pair_request(&state_clone);
                    }
                    ids::QUIT => {
                        tracing::info!("Quit requested from tray menu");
                        std::process::exit(0);
                    }
                    _ => {}
                }
            }
        }
    });

    tracing::info!("System tray initialized");
    Ok(tray)
}

/// Handle a pairing request from the tray menu.
fn handle_pair_request(state: &Arc<AppState>) {
    // Use a blocking runtime handle to interact with async pairing manager
    let rt = tokio::runtime::Handle::current();
    let state = state.clone();
    std::thread::spawn(move || {
        rt.block_on(async {
            let mut pm = state.pairing_manager.write().await;
            let code = pm.generate_code().to_string();
            tracing::info!("Pairing code generated: {code}");
            // In the future, display this in a dialog or notification
        });
    });
}

/// Create a simple 16x16 green square icon as a default tray icon.
fn create_default_icon() -> Icon {
    let size = 16u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for _ in 0..size * size {
        // Green color: #4CAF50
        rgba.push(0x4C); // R
        rgba.push(0xAF); // G
        rgba.push(0x50); // B
        rgba.push(0xFF); // A
    }
    Icon::from_rgba(rgba, size, size).expect("Failed to create tray icon")
}

/// Placeholder for when --no-tray is passed.
pub fn init_tray_disabled() {
    tracing::info!("System tray disabled (--no-tray)");
}
