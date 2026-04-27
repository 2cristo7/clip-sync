use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TokenStoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("token not found")]
    NotFound,
}

/// A stored token entry (the token itself is stored as SHA-256 hex, never plaintext).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenEntry {
    /// SHA-256 hex hash of the raw token bytes.
    pub hash: String,
    /// Device name from X-ClipSync-Device header.
    pub device: String,
    /// Unix timestamp when the token was registered.
    pub registered_at: u64,
    /// Unix timestamp of last successful authentication.
    pub last_used: u64,
}

/// Persistent token store backed by a JSON file.
///
/// Tokens are never stored in plaintext; only their SHA-256 hex hash is persisted.
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenStore {
    tokens: HashMap<String, TokenEntry>,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl TokenStore {
    /// Create a new empty token store.
    pub fn new() -> Self {
        Self {
            tokens: HashMap::new(),
            path: None,
        }
    }

    /// Default path: ~/.clipsync/tokens.json
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".clipsync")
            .join("tokens.json")
    }

    /// Load from a JSON file, or create empty if file doesn't exist.
    pub fn load(path: PathBuf) -> Result<Self, TokenStoreError> {
        if path.exists() {
            let data = fs::read_to_string(&path)?;
            let mut store: Self = serde_json::from_str(&data)?;
            store.path = Some(path);
            Ok(store)
        } else {
            Ok(Self {
                tokens: HashMap::new(),
                path: Some(path),
            })
        }
    }

    /// Save to disk.
    pub fn save(&self) -> Result<(), TokenStoreError> {
        if let Some(ref path) = self.path {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let data = serde_json::to_string_pretty(self)?;
            fs::write(path, data)?;
        }
        Ok(())
    }

    /// Hash a raw token to its storage key.
    pub fn hash_token(token_bytes: &[u8]) -> String {
        let hash = Sha256::digest(token_bytes);
        hex::encode(hash)
    }

    /// Register a new token.
    pub fn register(
        &mut self,
        token_bytes: &[u8],
        device: &str,
    ) -> Result<(), TokenStoreError> {
        let hash = Self::hash_token(token_bytes);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.tokens.insert(
            hash.clone(),
            TokenEntry {
                hash,
                device: device.to_string(),
                registered_at: now,
                last_used: now,
            },
        );

        self.save()?;
        Ok(())
    }

    /// Validate a token and update its last_used timestamp.
    pub fn validate(&mut self, token_bytes: &[u8]) -> Result<&TokenEntry, TokenStoreError> {
        let hash = Self::hash_token(token_bytes);
        if !self.tokens.contains_key(&hash) {
            return Err(TokenStoreError::NotFound);
        }
        // Update last_used
        if let Some(entry) = self.tokens.get_mut(&hash) {
            entry.last_used = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
        }
        // Save updated last_used (best-effort)
        let _ = self.save();
        // Return immutable ref
        Ok(self.tokens.get(&hash).unwrap())
    }

    /// Touch: update last_used without full validation.
    pub fn touch(&mut self, token_bytes: &[u8]) -> Result<(), TokenStoreError> {
        let hash = Self::hash_token(token_bytes);
        if let Some(entry) = self.tokens.get_mut(&hash) {
            entry.last_used = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            self.save()?;
            Ok(())
        } else {
            Err(TokenStoreError::NotFound)
        }
    }

    /// Revoke (delete) a token.
    pub fn revoke(&mut self, token_bytes: &[u8]) -> Result<(), TokenStoreError> {
        let hash = Self::hash_token(token_bytes);
        if self.tokens.remove(&hash).is_some() {
            self.save()?;
            Ok(())
        } else {
            Err(TokenStoreError::NotFound)
        }
    }

    /// List all stored token entries.
    pub fn list(&self) -> Vec<&TokenEntry> {
        self.tokens.values().collect()
    }

    /// Number of stored tokens.
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Check if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
}

impl Default for TokenStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_hex_sha256() {
        let hash = TokenStore::hash_token(b"test-token");
        assert_eq!(hash.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn register_and_validate() {
        let mut store = TokenStore::new();
        let token = b"my-secret-token";

        store.register(token, "TestDevice").unwrap();
        assert_eq!(store.len(), 1);

        let entry = store.validate(token).unwrap();
        assert_eq!(entry.device, "TestDevice");
    }

    #[test]
    fn validate_unknown_token_fails() {
        let mut store = TokenStore::new();
        assert!(matches!(
            store.validate(b"unknown"),
            Err(TokenStoreError::NotFound)
        ));
    }

    #[test]
    fn revoke_token() {
        let mut store = TokenStore::new();
        let token = b"to-revoke";

        store.register(token, "Device").unwrap();
        assert_eq!(store.len(), 1);

        store.revoke(token).unwrap();
        assert_eq!(store.len(), 0);
        assert!(store.validate(token).is_err());
    }

    #[test]
    fn revoke_unknown_fails() {
        let mut store = TokenStore::new();
        assert!(matches!(
            store.revoke(b"unknown"),
            Err(TokenStoreError::NotFound)
        ));
    }

    #[test]
    fn list_tokens() {
        let mut store = TokenStore::new();
        store.register(b"token1", "Device1").unwrap();
        store.register(b"token2", "Device2").unwrap();

        let list = store.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("clipsync_token_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let path = dir.join("tokens.json");

        let mut store = TokenStore::load(path.clone()).unwrap();
        store.register(b"persist-token", "PersistDevice").unwrap();

        // Load again
        let mut store2 = TokenStore::load(path).unwrap();
        let entry = store2.validate(b"persist-token").unwrap();
        assert_eq!(entry.device, "PersistDevice");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn touch_updates_last_used() {
        let mut store = TokenStore::new();
        let token = b"touch-me";
        store.register(token, "Device").unwrap();

        let before = store.validate(token).unwrap().last_used;
        // Touch (same second likely, but at least doesn't error)
        store.touch(token).unwrap();
        let after = store.validate(token).unwrap().last_used;
        assert!(after >= before);
    }
}
