use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use tracing::info;

use clipsync_core::config::VERSION;

/// Menu item IDs for event handling.
mod ids {
    pub const STATUS: &str = "status";
    pub const PAIR: &str = "pair";
    pub const PAUSE: &str = "pause";
    pub const QUIT: &str = "quit";
}

/// Shared state for the client tray.
pub struct ClientTrayState {
    /// Whether sync is currently paused.
    pub paused: Arc<AtomicBool>,
    /// Description of connection status.
    pub status_text: Arc<std::sync::Mutex<String>>,
}

impl ClientTrayState {
    pub fn new() -> Self {
        Self {
            paused: Arc::new(AtomicBool::new(false)),
            status_text: Arc::new(std::sync::Mutex::new("Disconnected".to_string())),
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn set_status(&self, text: &str) {
        *self.status_text.lock().unwrap() = text.to_string();
    }
}

impl Default for ClientTrayState {
    fn default() -> Self {
        Self::new()
    }
}

/// Callback type for tray events.
pub enum TrayEvent {
    /// User requested to start pairing.
    Pair,
    /// User toggled pause/resume.
    TogglePause,
    /// User requested quit.
    Quit,
}

/// Build the client system tray icon and menu.
///
/// Must be called from the main thread on macOS.
/// Returns the TrayIcon guard and spawns an event handler thread.
pub fn build_tray(
    state: Arc<ClientTrayState>,
    event_tx: tokio::sync::mpsc::UnboundedSender<TrayEvent>,
) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let menu = Menu::new();

    // Status label (disabled, informational)
    let status_item = MenuItem::with_id(
        ids::STATUS,
        format!("ClipSync Client v{VERSION} — Disconnected"),
        false, // disabled — just a label
        None,
    );
    menu.append(&status_item)?;
    menu.append(&PredefinedMenuItem::separator())?;

    // Pair
    let pair_item = MenuItem::with_id(ids::PAIR, "Pair with Server...", true, None);
    menu.append(&pair_item)?;

    // Pause / Resume
    let pause_item = MenuItem::with_id(ids::PAUSE, "Pause Sync", true, None);
    menu.append(&pause_item)?;

    menu.append(&PredefinedMenuItem::separator())?;

    // Quit
    let quit_item = MenuItem::with_id(ids::QUIT, "Quit ClipSync Client", true, None);
    menu.append(&quit_item)?;

    // Create 16x16 icon (blue square for client, green is server)
    let icon = create_client_icon();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(format!("ClipSync Client v{VERSION}"))
        .with_icon(icon)
        .build()?;

    // Spawn event handler thread
    let state_clone = state;
    std::thread::spawn(move || {
        let receiver = MenuEvent::receiver();
        loop {
            if let Ok(event) = receiver.recv() {
                match event.id().0.as_str() {
                    ids::PAIR => {
                        let _ = event_tx.send(TrayEvent::Pair);
                    }
                    ids::PAUSE => {
                        let was_paused = state_clone.paused.load(Ordering::Relaxed);
                        state_clone.paused.store(!was_paused, Ordering::Relaxed);
                        if was_paused {
                            info!("sync resumed from tray");
                        } else {
                            info!("sync paused from tray");
                        }
                        let _ = event_tx.send(TrayEvent::TogglePause);
                    }
                    ids::QUIT => {
                        info!("quit requested from tray menu");
                        let _ = event_tx.send(TrayEvent::Quit);
                    }
                    _ => {}
                }
            }
        }
    });

    info!("client system tray initialized");
    Ok(tray)
}

/// Create a simple 16x16 blue square icon for the client tray.
fn create_client_icon() -> Icon {
    let size = 16u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for _ in 0..size * size {
        // Blue color: #2196F3
        rgba.push(0x21); // R
        rgba.push(0x96); // G
        rgba.push(0xF3); // B
        rgba.push(0xFF); // A
    }
    Icon::from_rgba(rgba, size, size).expect("failed to create tray icon")
}

/// Placeholder for when --no-tray is passed.
pub fn init_tray_disabled() {
    info!("client system tray disabled (--no-tray)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_state_defaults() {
        let state = ClientTrayState::new();
        assert!(!state.is_paused());
        assert_eq!(*state.status_text.lock().unwrap(), "Disconnected");
    }

    #[test]
    fn tray_state_pause_toggle() {
        let state = ClientTrayState::new();
        assert!(!state.is_paused());
        state.paused.store(true, Ordering::Relaxed);
        assert!(state.is_paused());
    }

    #[test]
    fn tray_state_set_status() {
        let state = ClientTrayState::new();
        state.set_status("Connected to MyMac");
        assert_eq!(*state.status_text.lock().unwrap(), "Connected to MyMac");
    }
}
