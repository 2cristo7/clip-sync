// platform integrations: tray, notif, autostart — populated in Plan 2/3
//
// Stubbed during the Phase 1.9 workspace split so the new crate
// compiles cleanly. Real per-OS adapters land alongside the desktop
// app shells in later plans.

/// Marker placeholder so the crate exports something. Will be removed
/// once real platform adapters land.
pub const PLACEHOLDER: &str = "clipsync-platform: stub crate (Phase 1.9)";
