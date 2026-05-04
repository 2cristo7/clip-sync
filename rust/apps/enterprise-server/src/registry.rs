use std::path::Path;

use clipsync_storage::db::{self, Database, StorageError};
use clipsync_storage::models::Device;
use tracing::info;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Device registry — thin wrapper over clipsync_storage::Database
// ---------------------------------------------------------------------------

/// Enterprise device registry backed by SQLite.
#[derive(Clone)]
pub struct DeviceRegistry {
    #[allow(dead_code)]
    db: Database,
}

#[allow(dead_code)]
impl DeviceRegistry {
    /// Initialise the registry, running migrations on the given database path.
    pub async fn init(data_dir: &Path) -> Result<Self, StorageError> {
        let db_path = data_dir.join("clipsync.db");
        let db = Database::new(&db_path).await?;
        db.run_migrations().await?;
        info!(path = %db_path.display(), "device registry initialised");
        Ok(Self { db })
    }

    /// Pair a new device: generates an ID + raw token, stores hashed token.
    /// Returns `(device_id, raw_token)`.
    pub async fn pair_device(
        &self,
        name: &str,
        fingerprint: &str,
        role: &str,
    ) -> Result<(String, String), StorageError> {
        let device_id = Uuid::new_v4().to_string();
        let raw_token = Uuid::new_v4().to_string();
        let token_hash = db::hash_token(&raw_token);

        self.db
            .register_device(&device_id, name, fingerprint, role, &token_hash)
            .await?;

        self.db.store_token(&token_hash, &device_id).await?;

        info!(device_id = %device_id, name = %name, "device paired");
        Ok((device_id, raw_token))
    }

    /// List all registered devices.
    pub async fn list_devices(&self) -> Result<Vec<Device>, StorageError> {
        self.db.list_devices().await
    }

    /// Get a device by ID.
    pub async fn get_device(&self, id: &str) -> Result<Device, StorageError> {
        self.db.get_device(id).await
    }

    /// Touch a device's `last_seen` timestamp.
    pub async fn touch_device(&self, id: &str) -> Result<(), StorageError> {
        self.db.update_last_seen(id).await
    }

    /// Revoke a specific token (by its hash).
    pub async fn revoke_token(&self, raw_token: &str) -> Result<(), StorageError> {
        let token_hash = db::hash_token(raw_token);
        self.db.revoke_token(&token_hash).await
    }

    /// Verify a raw token: hash it, check it exists and is not revoked.
    pub async fn verify_token(&self, raw_token: &str) -> Result<bool, StorageError> {
        let token_hash = db::hash_token(raw_token);
        self.db.verify_token_hash(&token_hash).await
    }

    /// Look up the device associated with a raw token.
    pub async fn device_for_token(
        &self,
        raw_token: &str,
    ) -> Result<Device, StorageError> {
        let token_hash = db::hash_token(raw_token);
        self.db.get_device_by_token_hash(&token_hash).await
    }
}
