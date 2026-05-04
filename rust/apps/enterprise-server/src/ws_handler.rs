//! WebSocket handler with Hello/Welcome enterprise handshake.
//!
//! Implements the Phase 2.3 role-negotiation protocol:
//!
//! 1. On WS connect, wait up to 5 s for the first text frame.
//! 2. If the frame deserializes as [`Hello`] → enterprise path: validate
//!    `protocol_version`, record the device role in the hub, reply with
//!    [`Welcome`] containing the device's policy.
//! 3. If the frame does NOT parse as `Hello` → legacy path: treat the
//!    client as a personal peer with default policy `"read_write"` and
//!    process the frame as a normal [`ClipPayload`].
//! 4. If `protocol_version > CURRENT_PROTOCOL_VERSION` → send
//!    [`HandshakeError`] and close.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{info, warn};

use clipsync_protocol::handshake::{HandshakeError, Hello, Welcome, CURRENT_PROTOCOL_VERSION};
use clipsync_protocol::protocol::ClipPayload;
use clipsync_transport::config::WS_PING_INTERVAL;

use crate::audit::AuditEvent;
use crate::AppState;

/// Drive a single WebSocket connection through the handshake and then
/// the clipboard message loop.
pub async fn handle_ws(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // -----------------------------------------------------------------------
    // Phase 1 — Handshake (with 5 s timeout)
    // -----------------------------------------------------------------------
    let device_label: String;
    let mut first_payload: Option<ClipPayload> = None;

    let first_frame =
        tokio::time::timeout(Duration::from_secs(5), recv_text_frame(&mut ws_rx)).await;

    match first_frame {
        // Received a text frame in time
        Ok(Some(text)) => {
            if let Ok(hello) = serde_json::from_str::<Hello>(&text) {
                // Enterprise client — check protocol version
                if hello.protocol_version > CURRENT_PROTOCOL_VERSION {
                    let err = HandshakeError {
                        code: "unsupported_version".to_string(),
                        message: format!(
                            "server supports protocol version {} but client sent {}",
                            CURRENT_PROTOCOL_VERSION, hello.protocol_version,
                        ),
                    };
                    let _ = ws_tx
                        .send(Message::Text(serde_json::to_string(&err).unwrap().into()))
                        .await;
                    let _ = ws_tx.close().await;
                    return;
                }

                device_label = hello.device_id.clone();
                info!(
                    device_id = %hello.device_id,
                    role = ?hello.role,
                    protocol_version = hello.protocol_version,
                    "enterprise Hello received"
                );

                // Look up device policy from runtime (default read_write)
                let device_policy = state.policy_runtime.get_policy(&hello.device_id).await;
                let policy = device_policy.to_string();

                let server_id = hostname::get()
                    .map(|h| h.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "enterprise-server".to_string());

                let welcome = Welcome {
                    server_id,
                    server_capabilities: vec![
                        "broadcast".to_string(),
                        "policy".to_string(),
                        "audit".to_string(),
                    ],
                    your_policy: policy,
                    protocol_version: CURRENT_PROTOCOL_VERSION,
                };

                if ws_tx
                    .send(Message::Text(
                        serde_json::to_string(&welcome).unwrap().into(),
                    ))
                    .await
                    .is_err()
                {
                    return;
                }

                info!(device_id = %hello.device_id, "Welcome sent");
            } else {
                // Not a Hello frame — legacy client.  Try to parse as
                // ClipPayload so we don't drop the first message.
                device_label = "legacy-client".to_string();
                info!("legacy client connected (first frame is not Hello)");

                if let Ok(payload) = serde_json::from_str::<ClipPayload>(&text) {
                    first_payload = Some(payload);
                }
            }
        }
        // Timeout or WS closed before any text
        Ok(None) | Err(_) => {
            device_label = "legacy-client".to_string();
            info!("legacy client connected (no Hello within 5 s)");
        }
    }

    // -----------------------------------------------------------------------
    // Phase 2 — Register in hub + message loop
    // -----------------------------------------------------------------------
    let client_id = state.ws_hub.register(device_label.clone(), tx).await;
    info!(client_id = %client_id, device = %device_label, "ws client registered");

    // Audit: connection_opened
    state
        .audit_log
        .log(AuditEvent::connection_opened(&device_label))
        .await;

    // Deliver any pending broadcasts queued while this device was offline
    state.ws_hub.deliver_pending_to_device(&device_label).await;

    // If we captured a first payload from a legacy client, broadcast it now
    if let Some(payload) = first_payload {
        if let Ok(json) = serde_json::to_string(&payload) {
            state
                .ws_hub
                .broadcast_with_policy(
                    &json,
                    Some(&client_id),
                    &device_label,
                    &state.policy_runtime,
                )
                .await;
        }
    }

    // Outbound task: hub → client
    let send_task = tokio::spawn(async move {
        let mut ping_ticker = tokio::time::interval(WS_PING_INTERVAL);
        ping_ticker.tick().await; // skip immediate first tick
        loop {
            tokio::select! {
                maybe_msg = rx.recv() => {
                    match maybe_msg {
                        Some(msg) => {
                            if ws_tx.send(Message::Text(msg.into())).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                _ = ping_ticker.tick() => {
                    if ws_tx.send(Message::Ping(Bytes::new())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Inbound task: client → hub (broadcast with policy checks)
    let cid = client_id.clone();
    let device_label_clone = device_label.clone();
    let state_clone = state.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(text) => {
                    // Check if this is a BroadcastFile frame forwarded via WS
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                        if value.get("type").and_then(|v| v.as_str()) == Some("BroadcastFile") {
                            // BroadcastFile frames from clients are forwarded
                            // to their target_device_ids (server-side relay)
                            if let Some(targets) =
                                value.get("target_device_ids").and_then(|v| v.as_array())
                            {
                                let target_ids: Vec<String> = targets
                                    .iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect();
                                state_clone.ws_hub.send_to_devices(&target_ids, &text).await;
                            }
                            continue;
                        }
                    }

                    if let Ok(payload) = serde_json::from_str::<ClipPayload>(&text) {
                        // Policy: check if this device can push
                        if !state_clone
                            .policy_runtime
                            .can_push(&device_label_clone)
                            .await
                        {
                            warn!(
                                device = %device_label_clone,
                                "push rejected by policy"
                            );
                            continue;
                        }
                        let json = match serde_json::to_string(&payload) {
                            Ok(j) => j,
                            Err(e) => {
                                warn!(error = %e, "failed to re-serialize payload");
                                continue;
                            }
                        };

                        // Audit: clipboard_pushed (hash the serialized JSON, never raw content)
                        let content_bytes = json.as_bytes();
                        let kind = format!("{:?}", payload.clip_type).to_lowercase();
                        state_clone
                            .audit_log
                            .log(AuditEvent::clipboard_pushed(
                                &device_label_clone,
                                content_bytes,
                                &kind,
                            ))
                            .await;

                        state_clone
                            .ws_hub
                            .broadcast_with_policy(
                                &json,
                                Some(&cid),
                                &device_label_clone,
                                &state_clone.policy_runtime,
                            )
                            .await;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    state.ws_hub.unregister(&client_id).await;

    // Audit: connection_closed
    state
        .audit_log
        .log(AuditEvent::connection_closed(&device_label))
        .await;

    info!(client_id = %client_id, device = %device_label, "ws client disconnected");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Receive the next text frame from the WS stream, skipping pings/pongs.
/// Returns `None` if the stream closes before a text frame arrives.
async fn recv_text_frame(
    rx: &mut (impl StreamExt<Item = Result<Message, axum::Error>> + Unpin),
) -> Option<String> {
    while let Some(Ok(msg)) = rx.next().await {
        match msg {
            Message::Text(t) => return Some(t.to_string()),
            Message::Close(_) => return None,
            _ => continue, // skip ping, pong, binary
        }
    }
    None
}
