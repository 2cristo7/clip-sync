use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tracing::error;

use clipsync_core::clipboard::ClipboardProvider;
use clipsync_core::config::CLIPBOARD_POLL_MS;

use crate::connector::IncomingPayloadRx;
use crate::credentials::ClientCredentials;
use crate::sender;

/// Maximum number of recent nonces to track for echo suppression.
const ECHO_BUFFER_SIZE: usize = 32;

/// Run the clipboard watcher loop.
///
/// Polls the clipboard every CLIPBOARD_POLL_MS, detects changes, and sends
/// them to the server via POST /inject. Suppresses echoes from payloads
/// we recently received via WebSocket.
pub async fn run_watcher<C: ClipboardProvider>(
    creds: ClientCredentials,
    clipboard: Arc<C>,
    mut incoming_rx: IncomingPayloadRx,
    paused: Arc<AtomicBool>,
) {
    let http_client = match sender::build_send_client(&creds) {
        Ok(c) => c,
        Err(e) => {
            error!("failed to build HTTP client: {}", e);
            return;
        }
    };

    let recent_nonces: Arc<Mutex<VecDeque<String>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(ECHO_BUFFER_SIZE)));
    let recent_digests: Arc<Mutex<VecDeque<String>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(ECHO_BUFFER_SIZE)));

    // Spawn a task to collect incoming payload nonces/digests for echo suppression
    let nonces_clone = recent_nonces.clone();
    let digests_clone = recent_digests.clone();
    tokio::spawn(async move {
        while let Some(payload) = incoming_rx.recv().await {
            let mut nonces = nonces_clone.lock().unwrap();
            if nonces.len() >= ECHO_BUFFER_SIZE {
                nonces.pop_front();
            }
            nonces.push_back(payload.nonce.clone());

            let mut digests = digests_clone.lock().unwrap();
            if digests.len() >= ECHO_BUFFER_SIZE {
                digests.pop_front();
            }
            digests.push_back(payload.digest());
        }
    });

    let poll_interval = Duration::from_millis(CLIPBOARD_POLL_MS);
    let mut last_digest: Option<String> = None;

    loop {
        tokio::time::sleep(poll_interval).await;

        if paused.load(Ordering::Relaxed) {
            continue;
        }

        // Read clipboard
        let payload = match clipboard.read() {
            Ok(Some(p)) => p,
            Ok(None) => continue,
            Err(e) => {
                error!("clipboard read error: {}", e);
                continue;
            }
        };

        let digest = payload.digest();

        // Skip if same as last sent
        if last_digest.as_deref() == Some(&digest) {
            continue;
        }

        // Echo suppression: skip if this digest was recently received from server
        {
            let digests = recent_digests.lock().unwrap();
            if digests.iter().any(|d| d == &digest) {
                continue;
            }
        }

        // Send to server
        match sender::send_payload(&http_client, &creds, &payload).await {
            Ok(()) => {
                last_digest = Some(digest);
            }
            Err(e) => {
                error!("failed to send clipboard: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_buffer_size_is_reasonable() {
        assert!(ECHO_BUFFER_SIZE >= 8);
        assert!(ECHO_BUFFER_SIZE <= 128);
    }

    #[test]
    fn echo_suppression_logic() {
        let digests: VecDeque<String> = VecDeque::from(vec![
            "aaa".to_string(),
            "bbb".to_string(),
            "ccc".to_string(),
        ]);
        // Should find "bbb" in recent
        assert!(digests.iter().any(|d| d == "bbb"));
        // Should not find "ddd"
        assert!(!digests.iter().any(|d| d == "ddd"));
    }
}
