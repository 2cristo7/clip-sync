//! POST /broadcast — multipart file broadcast to targeted devices.
//!
//! Accepts a multipart form with:
//! - `file`: the file to broadcast (max 50 MB)
//! - `target_device_ids[]`: one or more target device IDs
//!
//! The server stores the file temporarily in `<data-dir>/broadcasts/<id>` for
//! retry (expires after 1 hour), then pushes a `BroadcastFile` WS frame to
//! each target. Offline clients receive the broadcast on reconnect within the
//! 1-hour window.

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::Serialize;
use tracing::info;
use uuid::Uuid;

use crate::AppState;

/// Hard cap: 50 MB.
const MAX_BROADCAST_SIZE: usize = 50 * 1024 * 1024;

/// Broadcast expiry: 1 hour.
const BROADCAST_EXPIRY_SECS: u64 = 3600;

/// Response from POST /broadcast.
#[derive(Serialize)]
pub struct BroadcastResponse {
    pub id: String,
    pub file_name: String,
    pub size: usize,
    pub target_device_ids: Vec<String>,
    pub delivery_status: Vec<DeviceDeliveryStatus>,
}

/// Per-device delivery status.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceDeliveryStatus {
    pub device_id: String,
    pub status: DeliveryState,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryState {
    Pending,
    Delivered,
    #[allow(dead_code)]
    Failed,
}

/// WS frame sent to clients for file broadcasts.
#[derive(Clone, Serialize)]
pub struct BroadcastFileFrame {
    #[serde(rename = "type")]
    pub frame_type: String,
    pub id: String,
    pub name: String,
    pub mime: String,
    pub bytes_b64: String,
    pub sender_device_id: String,
    pub target_device_ids: Vec<String>,
}

/// Stored broadcast metadata for offline delivery / retry.
#[derive(Clone)]
pub struct PendingBroadcast {
    pub id: String,
    pub file_name: String,
    pub mime: String,
    pub bytes_b64: String,
    pub sender_device_id: String,
    pub target_device_ids: Vec<String>,
    pub created_at: Instant,
    pub delivered_to: Vec<String>,
}

impl PendingBroadcast {
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed().as_secs() >= BROADCAST_EXPIRY_SECS
    }

    /// Build the WS frame JSON for this broadcast.
    pub fn to_ws_frame_json(&self) -> String {
        let frame = BroadcastFileFrame {
            frame_type: "BroadcastFile".to_string(),
            id: self.id.clone(),
            name: self.file_name.clone(),
            mime: self.mime.clone(),
            bytes_b64: self.bytes_b64.clone(),
            sender_device_id: self.sender_device_id.clone(),
            target_device_ids: self.target_device_ids.clone(),
        };
        serde_json::to_string(&frame).expect("BroadcastFileFrame is always serializable")
    }
}

/// Handle POST /broadcast multipart upload.
pub async fn post_broadcast(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_name = String::from("untitled");
    let mut mime_type = String::from("application/octet-stream");
    let mut target_device_ids: Vec<String> = Vec::new();
    let mut sender_device_id = String::from("api");

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                if let Some(fname) = field.file_name() {
                    file_name = fname.to_string();
                }
                if let Some(ct) = field.content_type() {
                    mime_type = ct.to_string();
                }
                let data = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("failed to read file field: {e}"),
                    )
                })?;
                if data.len() > MAX_BROADCAST_SIZE {
                    return Err((
                        StatusCode::PAYLOAD_TOO_LARGE,
                        format!(
                            "file size {} bytes exceeds maximum allowed size of {} bytes (50 MB)",
                            data.len(),
                            MAX_BROADCAST_SIZE,
                        ),
                    ));
                }
                file_bytes = Some(data.to_vec());
            }
            "target_device_ids[]" => {
                let val = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("failed to read target_device_ids[] field: {e}"),
                    )
                })?;
                if !val.is_empty() {
                    target_device_ids.push(val);
                }
            }
            "sender_device_id" => {
                let val = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("failed to read sender_device_id field: {e}"),
                    )
                })?;
                if !val.is_empty() {
                    sender_device_id = val;
                }
            }
            _ => {
                // Skip unknown fields
            }
        }
    }

    let raw_bytes = file_bytes.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "missing required 'file' field in multipart form".to_string(),
        )
    })?;

    if target_device_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "at least one target_device_ids[] is required".to_string(),
        ));
    }

    let broadcast_id = Uuid::new_v4().to_string();
    let bytes_b64 = BASE64.encode(&raw_bytes);
    let file_size = raw_bytes.len();

    // Store file on disk for retry/offline delivery
    let broadcasts_dir = state.data_dir().join("broadcasts");
    tokio::fs::create_dir_all(&broadcasts_dir)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to create broadcasts directory: {e}"),
            )
        })?;
    tokio::fs::write(broadcasts_dir.join(&broadcast_id), &raw_bytes)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to store broadcast file: {e}"),
            )
        })?;

    let pending = PendingBroadcast {
        id: broadcast_id.clone(),
        file_name: file_name.clone(),
        mime: mime_type,
        bytes_b64,
        sender_device_id: sender_device_id.clone(),
        target_device_ids: target_device_ids.clone(),
        created_at: Instant::now(),
        delivered_to: Vec::new(),
    };

    let frame_json = pending.to_ws_frame_json();

    // Deliver to online targets, track who received it
    let delivery_status = state
        .ws_hub
        .send_to_devices(&target_device_ids, &frame_json)
        .await;

    // Update delivered_to based on who got it
    let mut delivered_devices: Vec<String> = Vec::new();
    for ds in &delivery_status {
        if ds.status == DeliveryState::Delivered {
            delivered_devices.push(ds.device_id.clone());
        }
    }

    // Store pending broadcast for offline retry (only if some targets missed)
    let has_pending = delivery_status
        .iter()
        .any(|ds| ds.status == DeliveryState::Pending);
    if has_pending {
        let mut pb = pending;
        pb.delivered_to = delivered_devices;
        state.ws_hub.queue_pending_broadcast(pb).await;
    }

    // Audit: broadcast_sent
    state
        .audit_log
        .log(crate::audit::AuditEvent::broadcast_sent(
            &sender_device_id,
            &broadcast_id,
            &target_device_ids,
            file_size,
        ))
        .await;

    // Audit: broadcast_delivered for each device that received it immediately
    for ds in &delivery_status {
        if ds.status == DeliveryState::Delivered {
            state
                .audit_log
                .log(crate::audit::AuditEvent::broadcast_delivered(
                    &ds.device_id,
                    &broadcast_id,
                ))
                .await;
        }
    }

    // Emit delivery status event over WS to sender
    let status_event = serde_json::json!({
        "type": "BroadcastStatus",
        "broadcast_id": broadcast_id,
        "delivery_status": delivery_status,
    });
    state
        .ws_hub
        .send_to_device(&sender_device_id, &status_event.to_string())
        .await;

    info!(
        broadcast_id = %broadcast_id,
        file_name = %file_name,
        size = file_size,
        targets = ?target_device_ids,
        "broadcast created"
    );

    Ok(Json(BroadcastResponse {
        id: broadcast_id,
        file_name,
        size: file_size,
        target_device_ids,
        delivery_status,
    }))
}
