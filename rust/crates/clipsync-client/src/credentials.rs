use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Persisted credentials from a successful pairing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCredentials {
    /// Bearer token (base64-encoded).
    pub token: String,
    /// Shared HMAC secret (base64-encoded).
    pub secret: String,
    /// Server host (IP or hostname).
    pub host: String,
    /// Server port.
    pub port: u16,
    /// TLS certificate fingerprint (base64url, no padding).
    pub fingerprint: String,
    /// Human-readable server name from mDNS.
    #[serde(default)]
    pub server_name: Option<String>,
}

impl ClientCredentials {
    /// Default path: ~/.clipsync/client_creds.json
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".clipsync")
            .join("client_creds.json")
    }

    /// Load credentials from a JSON file.
    pub fn load(path: &Path) -> Result<Self, CredentialError> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| CredentialError::Io(e.to_string()))?;
        serde_json::from_str(&data)
            .map_err(|e| CredentialError::Parse(e.to_string()))
    }

    /// Save credentials to a JSON file.
    pub fn save(&self, path: &Path) -> Result<(), CredentialError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CredentialError::Io(e.to_string()))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| CredentialError::Parse(e.to_string()))?;
        std::fs::write(path, json)
            .map_err(|e| CredentialError::Io(e.to_string()))?;
        Ok(())
    }

    /// Check if a credentials file exists at the given path.
    pub fn exists(path: &Path) -> bool {
        path.exists()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("parse error: {0}")]
    Parse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("clipsync_creds_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("client_creds.json");
        let creds = ClientCredentials {
            token: "dG9rZW4=".to_string(),
            secret: "c2VjcmV0".to_string(),
            host: "192.168.1.100".to_string(),
            port: 7010,
            fingerprint: "abc123".to_string(),
            server_name: Some("MyMac".to_string()),
        };

        creds.save(&path).unwrap();
        let loaded = ClientCredentials::load(&path).unwrap();
        assert_eq!(loaded.token, creds.token);
        assert_eq!(loaded.secret, creds.secret);
        assert_eq!(loaded.host, creds.host);
        assert_eq!(loaded.port, creds.port);
        assert_eq!(loaded.fingerprint, creds.fingerprint);
        assert_eq!(loaded.server_name, creds.server_name);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn default_path_is_under_home() {
        let path = ClientCredentials::default_path();
        assert!(path.to_string_lossy().contains(".clipsync"));
        assert!(path.to_string_lossy().ends_with("client_creds.json"));
    }
}
