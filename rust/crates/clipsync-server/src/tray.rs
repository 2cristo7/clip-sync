// System tray module — placeholder for tray-icon + muda integration.
// Will be implemented with tray-icon and muda crates.
//
// Menu items:
// - Status label (e.g., "ClipSync v0.1.0 — running")
// - Start pairing
// - Connected devices (submenu)
// - Quit

/// Placeholder — system tray is not yet implemented.
pub fn init_tray() {
    tracing::info!("System tray: not yet implemented (use --no-tray to suppress)");
}
