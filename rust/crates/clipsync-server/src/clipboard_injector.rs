use clipsync_core::clipboard::{ClipboardProvider, SystemClipboard, notify_received};
use clipsync_core::protocol::{ClipPayload, ClipType};

/// Inject a payload into the system clipboard and show notification for non-text.
pub fn inject_to_clipboard(payload: &ClipPayload) -> Result<(), String> {
    let clipboard = SystemClipboard::new();
    clipboard
        .write(payload)
        .map_err(|e| format!("Clipboard injection failed: {e}"))?;

    // Show desktop notification for image and file payloads
    if payload.clip_type != ClipType::Text {
        if let Err(e) = notify_received(payload) {
            tracing::warn!("notification failed: {}", e);
        }
    }

    Ok(())
}
