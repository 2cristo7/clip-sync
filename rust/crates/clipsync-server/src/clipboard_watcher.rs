use clipsync_core::clipboard::{ClipboardWatcher, SystemClipboard};
use clipsync_core::config::CLIPBOARD_POLL_MS;
use clipsync_core::protocol::ClipPayload;

use crate::ws_hub::WsHub;

/// Start a clipboard watcher that polls and broadcasts changes to the WebSocket hub.
pub async fn run_clipboard_watcher(hub: WsHub) {
    let clipboard = match SystemClipboard::new() {
        cb => cb,
    };
    let watcher = ClipboardWatcher::new(clipboard, CLIPBOARD_POLL_MS);
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ClipPayload>(32);

    // Spawn the polling loop
    tokio::spawn(async move {
        if let Err(e) = watcher.watch(tx).await {
            tracing::error!("Clipboard watcher error: {e}");
        }
    });

    // Forward clipboard changes to WebSocket hub
    while let Some(payload) = rx.recv().await {
        tracing::info!(
            "Clipboard changed: {:?} ({} bytes)",
            payload.clip_type,
            payload.data.len()
        );
        hub.broadcast(&payload, None).await;
    }
}
