/// A registered device in the enterprise registry.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub fingerprint: String,
    pub role: String,
    pub paired_at: String,
    pub last_seen: String,
    pub token_hash: String,
    pub policy: String,
}

/// An authentication token associated with a device.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Token {
    pub token_id: String,
    pub device_id: String,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

/// An audit log entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct AuditEntry {
    pub id: String,
    pub ts: String,
    pub event_type: String,
    pub device_id: String,
    pub payload_summary: Option<String>,
    pub metadata_json: Option<String>,
}
