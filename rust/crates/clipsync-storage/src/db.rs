use std::path::Path;

use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use tracing::info;

use crate::models::{Device, Token};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("device not found: {0}")]
    DeviceNotFound(String),

    #[error("token not found: {0}")]
    TokenNotFound(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

// ---------------------------------------------------------------------------
// Database wrapper
// ---------------------------------------------------------------------------

/// Async SQLite database for the enterprise device registry.
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Open (or create) a SQLite database at the given path.
    pub async fn new(path: &Path) -> Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;

        Ok(Self { pool })
    }

    /// Run embedded migrations from the `migrations/` directory.
    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        info!("database migrations applied");
        Ok(())
    }

    // -- Device operations --------------------------------------------------

    /// Register a new device. Returns the device ID.
    pub async fn register_device(
        &self,
        id: &str,
        name: &str,
        fingerprint: &str,
        role: &str,
        token_hash: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO devices (id, name, fingerprint, role, paired_at, last_seen, token_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(name)
        .bind(fingerprint)
        .bind(role)
        .bind(&now)
        .bind(&now)
        .bind(token_hash)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List all registered devices.
    pub async fn list_devices(&self) -> Result<Vec<Device>> {
        let devices = sqlx::query_as::<_, Device>("SELECT * FROM devices ORDER BY paired_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(devices)
    }

    /// Get a single device by ID.
    pub async fn get_device(&self, id: &str) -> Result<Device> {
        sqlx::query_as::<_, Device>("SELECT * FROM devices WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StorageError::DeviceNotFound(id.to_string()))
    }

    /// Update the `last_seen` timestamp for a device.
    pub async fn update_last_seen(&self, id: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query("UPDATE devices SET last_seen = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(StorageError::DeviceNotFound(id.to_string()));
        }
        Ok(())
    }

    /// Look up a device by its token hash.
    pub async fn get_device_by_token_hash(&self, token_hash: &str) -> Result<Device> {
        sqlx::query_as::<_, Device>("SELECT * FROM devices WHERE token_hash = ?")
            .bind(token_hash)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StorageError::DeviceNotFound(format!("token_hash={token_hash}")))
    }

    // -- Token operations ---------------------------------------------------

    /// Store a new token record (token_id is the hash of the raw token).
    pub async fn store_token(&self, token_id: &str, device_id: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO tokens (token_id, device_id, created_at) VALUES (?, ?, ?)",
        )
        .bind(token_id)
        .bind(device_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Revoke a token by setting `revoked_at`.
    pub async fn revoke_token(&self, token_id: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let result =
            sqlx::query("UPDATE tokens SET revoked_at = ? WHERE token_id = ? AND revoked_at IS NULL")
                .bind(&now)
                .bind(token_id)
                .execute(&self.pool)
                .await?;
        if result.rows_affected() == 0 {
            return Err(StorageError::TokenNotFound(token_id.to_string()));
        }
        Ok(())
    }

    /// Verify that a token hash exists and has not been revoked.
    pub async fn verify_token_hash(&self, token_hash: &str) -> Result<bool> {
        let row = sqlx::query_as::<_, Token>(
            "SELECT * FROM tokens WHERE token_id = ? AND revoked_at IS NULL",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    /// Get the underlying pool (for testing or advanced use).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

/// Hash a raw token string to its SHA-256 hex digest.
pub fn hash_token(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup_db() -> (Database, TempDir) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();
        db.run_migrations().await.unwrap();
        (db, dir)
    }

    #[tokio::test]
    async fn test_register_and_get_device() {
        let (db, _dir) = setup_db().await;
        let token_hash = hash_token("test-token-123");

        db.register_device("dev-1", "MacBook", "fp-abc", "admin", &token_hash)
            .await
            .unwrap();

        let device = db.get_device("dev-1").await.unwrap();
        assert_eq!(device.name, "MacBook");
        assert_eq!(device.fingerprint, "fp-abc");
        assert_eq!(device.role, "admin");
        assert_eq!(device.token_hash, token_hash);
    }

    #[tokio::test]
    async fn test_list_devices() {
        let (db, _dir) = setup_db().await;

        db.register_device("d1", "Mac", "fp1", "admin", &hash_token("t1"))
            .await
            .unwrap();
        db.register_device("d2", "Android", "fp2", "client", &hash_token("t2"))
            .await
            .unwrap();

        let devices = db.list_devices().await.unwrap();
        assert_eq!(devices.len(), 2);
    }

    #[tokio::test]
    async fn test_update_last_seen() {
        let (db, _dir) = setup_db().await;
        db.register_device("d1", "Mac", "fp1", "admin", &hash_token("t1"))
            .await
            .unwrap();

        let before = db.get_device("d1").await.unwrap().last_seen;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        db.update_last_seen("d1").await.unwrap();
        let after = db.get_device("d1").await.unwrap().last_seen;

        assert_ne!(before, after);
    }

    #[tokio::test]
    async fn test_device_not_found() {
        let (db, _dir) = setup_db().await;
        let err = db.get_device("nonexistent").await.unwrap_err();
        assert!(matches!(err, StorageError::DeviceNotFound(_)));
    }

    #[tokio::test]
    async fn test_store_and_verify_token() {
        let (db, _dir) = setup_db().await;
        let raw = "my-raw-token";
        let th = hash_token(raw);

        db.register_device("d1", "Mac", "fp1", "admin", &th)
            .await
            .unwrap();
        db.store_token(&th, "d1").await.unwrap();

        assert!(db.verify_token_hash(&th).await.unwrap());
        assert!(!db.verify_token_hash("bogus").await.unwrap());
    }

    #[tokio::test]
    async fn test_revoke_token() {
        let (db, _dir) = setup_db().await;
        let th = hash_token("tok");

        db.register_device("d1", "Mac", "fp1", "admin", &th)
            .await
            .unwrap();
        db.store_token(&th, "d1").await.unwrap();

        assert!(db.verify_token_hash(&th).await.unwrap());
        db.revoke_token(&th).await.unwrap();
        assert!(!db.verify_token_hash(&th).await.unwrap());
    }

    #[tokio::test]
    async fn test_get_device_by_token_hash() {
        let (db, _dir) = setup_db().await;
        let th = hash_token("tok");

        db.register_device("d1", "Mac", "fp1", "admin", &th)
            .await
            .unwrap();

        let device = db.get_device_by_token_hash(&th).await.unwrap();
        assert_eq!(device.id, "d1");
    }

    #[tokio::test]
    async fn test_hash_token_deterministic() {
        let h1 = hash_token("same-input");
        let h2 = hash_token("same-input");
        assert_eq!(h1, h2);
        assert_ne!(h1, hash_token("different-input"));
    }
}
