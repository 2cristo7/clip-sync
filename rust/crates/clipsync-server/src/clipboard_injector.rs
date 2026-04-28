use clipsync_core::clipboard::{ClipboardProvider, SystemClipboard};
use clipsync_core::protocol::ClipPayload;

/// Inject a payload into the system clipboard.
pub fn inject_to_clipboard(payload: &ClipPayload) -> Result<(), String> {
    let clipboard = SystemClipboard::new();
    clipboard
        .write(payload)
        .map_err(|e| format!("Clipboard injection failed: {e}"))
}
