//! Audit logging service for the enterprise server.
//!
//! Every state-changing event (clipboard push/deliver, device pair/revoke,
//! broadcast, policy change, connection open/close) is recorded in SQLite
//! via [`clipsync_storage`].
//!
//! **Privacy guarantee:** Raw clipboard content is NEVER stored.  Only a
//! SHA-256 hash + size + kind is persisted as `payload_summary`.

use std::time::Duration;

use clipsync_storage::db::Database;
use clipsync_storage::models::AuditEntry;
use sha2::{Digest, Sha256};
use tracing::{info, warn};
use uuid::Uuid;

/// Audit logger backed by the shared enterprise SQLite database.
#[derive(Clone)]
pub struct AuditLog {
    db: Database,
    /// Retention period in days (default 30).
    retention_days: u64,
}

impl AuditLog {
    /// Create a new audit logger wrapping the given database.
    pub fn new(db: Database, retention_days: u64) -> Self {
        Self { db, retention_days }
    }

    /// Record an audit event.
    pub async fn log(&self, event: AuditEvent) {
        let entry = AuditEntry {
            id: Uuid::new_v4().to_string(),
            ts: chrono::Utc::now().to_rfc3339(),
            event_type: event.event_type.to_string(),
            device_id: event.device_id,
            payload_summary: event.payload_summary,
            metadata_json: event
                .metadata
                .map(|m| serde_json::to_string(&m).unwrap_or_default()),
        };

        if let Err(e) = self.db.insert_audit_entry(&entry).await {
            warn!(error = %e, event_type = %entry.event_type, "failed to write audit entry");
        }
    }

    /// Purge entries older than the configured retention period.
    pub async fn purge_expired(&self) {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(self.retention_days as i64);
        let cutoff_str = cutoff.to_rfc3339();

        match self.db.purge_audit_before(&cutoff_str).await {
            Ok(n) if n > 0 => {
                info!(
                    purged = n,
                    retention_days = self.retention_days,
                    "audit entries purged"
                );
            }
            Ok(_) => {}
            Err(e) => {
                warn!(error = %e, "failed to purge audit entries");
            }
        }
    }

    /// Get the underlying database (for query API).
    pub fn db(&self) -> &Database {
        &self.db
    }

    /// Get retention days.
    #[allow(dead_code)]
    pub fn retention_days(&self) -> u64 {
        self.retention_days
    }
}

/// Spawn a background task that purges expired audit entries periodically.
pub fn spawn_audit_purge_task(audit_log: AuditLog) {
    tokio::spawn(async move {
        // Run purge every hour
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        loop {
            interval.tick().await;
            audit_log.purge_expired().await;
        }
    });
}

// ---------------------------------------------------------------------------
// Event builder
// ---------------------------------------------------------------------------

/// An audit event to be logged.
pub struct AuditEvent {
    pub event_type: EventType,
    pub device_id: String,
    pub payload_summary: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Supported audit event types.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum EventType {
    DevicePaired,
    DeviceRevoked,
    ClipboardPushed,
    ClipboardDelivered,
    BroadcastSent,
    BroadcastDelivered,
    PolicyChanged,
    ConnectionOpened,
    ConnectionClosed,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::DevicePaired => "device_paired",
            Self::DeviceRevoked => "device_revoked",
            Self::ClipboardPushed => "clipboard_pushed",
            Self::ClipboardDelivered => "clipboard_delivered",
            Self::BroadcastSent => "broadcast_sent",
            Self::BroadcastDelivered => "broadcast_delivered",
            Self::PolicyChanged => "policy_changed",
            Self::ConnectionOpened => "connection_opened",
            Self::ConnectionClosed => "connection_closed",
        };
        f.write_str(s)
    }
}

/// Build a privacy-safe payload summary: `sha256:<hash> size=<n> kind=<kind>`.
///
/// **NEVER** includes the raw clipboard text.
pub fn payload_summary(content: &[u8], kind: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let hash = hex::encode(hasher.finalize());
    format!("sha256:{hash} size={} kind={kind}", content.len())
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

#[allow(dead_code)]
impl AuditEvent {
    pub fn clipboard_pushed(device_id: &str, content: &[u8], kind: &str) -> Self {
        Self {
            event_type: EventType::ClipboardPushed,
            device_id: device_id.to_string(),
            payload_summary: Some(payload_summary(content, kind)),
            metadata: None,
        }
    }

    pub fn clipboard_delivered(device_id: &str, content: &[u8], kind: &str) -> Self {
        Self {
            event_type: EventType::ClipboardDelivered,
            device_id: device_id.to_string(),
            payload_summary: Some(payload_summary(content, kind)),
            metadata: None,
        }
    }

    pub fn connection_opened(device_id: &str) -> Self {
        Self {
            event_type: EventType::ConnectionOpened,
            device_id: device_id.to_string(),
            payload_summary: None,
            metadata: None,
        }
    }

    pub fn connection_closed(device_id: &str) -> Self {
        Self {
            event_type: EventType::ConnectionClosed,
            device_id: device_id.to_string(),
            payload_summary: None,
            metadata: None,
        }
    }

    pub fn broadcast_sent(
        device_id: &str,
        broadcast_id: &str,
        targets: &[String],
        file_size: usize,
    ) -> Self {
        Self {
            event_type: EventType::BroadcastSent,
            device_id: device_id.to_string(),
            payload_summary: Some(format!("broadcast={broadcast_id} size={file_size}")),
            metadata: Some(serde_json::json!({
                "broadcast_id": broadcast_id,
                "target_device_ids": targets,
            })),
        }
    }

    pub fn broadcast_delivered(device_id: &str, broadcast_id: &str) -> Self {
        Self {
            event_type: EventType::BroadcastDelivered,
            device_id: device_id.to_string(),
            payload_summary: Some(format!("broadcast={broadcast_id}")),
            metadata: Some(serde_json::json!({
                "broadcast_id": broadcast_id,
            })),
        }
    }

    pub fn policy_changed(device_id: &str, new_policy: &str) -> Self {
        Self {
            event_type: EventType::PolicyChanged,
            device_id: device_id.to_string(),
            payload_summary: None,
            metadata: Some(serde_json::json!({
                "new_policy": new_policy,
            })),
        }
    }

    pub fn device_paired(device_id: &str, name: &str) -> Self {
        Self {
            event_type: EventType::DevicePaired,
            device_id: device_id.to_string(),
            payload_summary: None,
            metadata: Some(serde_json::json!({
                "device_name": name,
            })),
        }
    }

    pub fn device_revoked(device_id: &str) -> Self {
        Self {
            event_type: EventType::DeviceRevoked,
            device_id: device_id.to_string(),
            payload_summary: None,
            metadata: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_summary_no_raw_content() {
        let raw_content = b"super secret clipboard text";
        let summary = payload_summary(raw_content, "text");

        // Must NOT contain the raw content
        assert!(!summary.contains("super secret"));
        assert!(!summary.contains("clipboard text"));

        // Must contain hash, size, kind
        assert!(summary.starts_with("sha256:"));
        assert!(summary.contains("size=27"));
        assert!(summary.contains("kind=text"));
    }

    #[test]
    fn test_payload_summary_deterministic() {
        let s1 = payload_summary(b"hello", "text");
        let s2 = payload_summary(b"hello", "text");
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_event_type_display() {
        assert_eq!(EventType::ClipboardPushed.to_string(), "clipboard_pushed");
        assert_eq!(EventType::PolicyChanged.to_string(), "policy_changed");
        assert_eq!(EventType::ConnectionOpened.to_string(), "connection_opened");
    }
}
