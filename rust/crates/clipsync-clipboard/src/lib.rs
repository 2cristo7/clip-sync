//! Cross-platform clipboard provider for ClipSync, wrapping `arboard`
//! with the per-OS quirks (TIFF→PNG conversion on macOS, file save
//! handling, native notifications) the Rust apps need. Polling cadence
//! lives in `config::CLIPBOARD_POLL_MS`.

pub mod clipboard;
pub mod config;

// Convenience re-exports so `clipsync_clipboard::ClipboardProvider`
// works at the crate root.
pub use clipboard::*;
