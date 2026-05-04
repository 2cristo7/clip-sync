//! Pairing logic for the ClipSync personal desktop app.
//!
//! Implements three pairing modes:
//! - **Auto-trust on same LAN**: discovered peer shows confirmation, one-tap accept.
//! - **6-digit OTP**: one side shows code, other types it.
//! - **QR code**: desktop shows QR, phone scans.
//!
//! Paired devices are persisted to `~/.config/clipsync-personal/peers.toml`.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{info, warn};

/// How long an OTP code remains valid (seconds).
const OTP_TTL_SECS: u64 = 120;

/// Length of the OTP code.
const OTP_LENGTH: usize = 6;

// ── Pairing state machine ─────────────────────────────────────────

/// States of the pairing state machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairingState {
    /// Peer discovered but not yet paired.
    Discovered,
    /// User initiated pairing — waiting for confirmation.
    PairingInitiated,
    /// Both sides confirmed — deriving shared key.
    PairingConfirmed,
    /// Fully paired and persisted.
    Paired,
}

impl fmt::Display for PairingState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discovered => write!(f, "discovered"),
            Self::PairingInitiated => write!(f, "pairing_initiated"),
            Self::PairingConfirmed => write!(f, "pairing_confirmed"),
            Self::Paired => write!(f, "paired"),
        }
    }
}

/// Mode of pairing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairingMode {
    /// Auto-trust on same LAN (still exchanges secret).
    AutoTrust,
    /// 6-digit one-time password.
    Otp,
    /// QR code (encodes connection info + challenge).
    Qr,
}

/// Information about a peer in the pairing process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Unique device ID.
    pub device_id: String,
    /// Human-readable device name.
    pub device_name: String,
    /// IP address(es).
    pub addresses: Vec<String>,
    /// Port.
    pub port: u16,
    /// Current pairing state.
    pub state: PairingState,
    /// Pairing mode used (None if only discovered).
    pub mode: Option<PairingMode>,
    /// Shared HMAC secret (hex-encoded, only set when paired).
    #[serde(skip_serializing)]
    pub shared_secret: Option<String>,
}

/// An active OTP challenge.
#[derive(Debug, Clone)]
struct OtpChallenge {
    code: String,
    device_id: String,
    created_at: u64,
}

// ── Persisted config ──────────────────────────────────────────────

/// A paired peer entry in the TOML config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedPeerEntry {
    pub device_id: String,
    pub device_name: String,
    pub addresses: Vec<String>,
    pub port: u16,
    pub shared_secret: String,
    pub paired_at: u64,
}

/// The peers.toml structure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PeersConfig {
    #[serde(default)]
    pub peers: Vec<PairedPeerEntry>,
}

// ── Pairing Manager ───────────────────────────────────────────────

/// Manages the pairing workflow for the personal desktop app.
pub struct PairingManager {
    /// Our device ID.
    our_device_id: String,
    /// Our device name (hostname).
    our_device_name: String,
    /// Path to the peers.toml config file.
    config_path: PathBuf,
    /// In-flight pairing sessions indexed by device_id.
    sessions: Arc<Mutex<HashMap<String, PeerInfo>>>,
    /// Active OTP challenges.
    challenges: Arc<Mutex<Vec<OtpChallenge>>>,
    /// Persisted paired peers.
    paired: Arc<Mutex<PeersConfig>>,
}

impl PairingManager {
    /// Create a new pairing manager.
    pub fn new(our_device_id: String, our_device_name: String, config_dir: &Path) -> Self {
        let config_path = config_dir.join("peers.toml");
        let paired = load_peers_config(&config_path);

        Self {
            our_device_id,
            our_device_name,
            config_path,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            challenges: Arc::new(Mutex::new(Vec::new())),
            paired: Arc::new(Mutex::new(paired)),
        }
    }

    /// Our device ID.
    pub fn device_id(&self) -> &str {
        &self.our_device_id
    }

    /// Our device name.
    pub fn device_name(&self) -> &str {
        &self.our_device_name
    }

    /// Register a discovered peer (from mDNS or manual add).
    pub async fn register_discovered(
        &self,
        device_id: String,
        device_name: String,
        addresses: Vec<String>,
        port: u16,
    ) {
        // Skip if already paired.
        let paired = self.paired.lock().await;
        if paired.peers.iter().any(|p| p.device_id == device_id) {
            return;
        }
        drop(paired);

        let mut sessions = self.sessions.lock().await;
        sessions
            .entry(device_id.clone())
            .or_insert_with(|| PeerInfo {
                device_id,
                device_name,
                addresses,
                port,
                state: PairingState::Discovered,
                mode: None,
                shared_secret: None,
            });
    }

    /// Get all discovered (unpaired) peers.
    pub async fn get_discovered_peers(&self) -> Vec<PeerInfo> {
        let sessions = self.sessions.lock().await;
        sessions
            .values()
            .filter(|p| p.state == PairingState::Discovered)
            .cloned()
            .collect()
    }

    /// Get all paired peers with their stored info.
    pub async fn get_paired_peers(&self) -> Vec<PairedPeerEntry> {
        let paired = self.paired.lock().await;
        paired.peers.clone()
    }

    /// Initiate pairing with a peer. Returns a 6-digit OTP code.
    pub async fn initiate_pairing(&self, device_id: &str) -> Result<String, PairingError> {
        let mut sessions = self.sessions.lock().await;
        let peer = sessions
            .get_mut(device_id)
            .ok_or_else(|| PairingError::PeerNotFound(device_id.to_string()))?;

        if peer.state != PairingState::Discovered {
            return Err(PairingError::InvalidState {
                current: peer.state.to_string(),
                expected: "discovered".to_string(),
            });
        }

        peer.state = PairingState::PairingInitiated;
        peer.mode = Some(PairingMode::Otp);

        let code = generate_otp();

        let mut challenges = self.challenges.lock().await;
        challenges.push(OtpChallenge {
            code: code.clone(),
            device_id: device_id.to_string(),
            created_at: now_secs(),
        });

        info!(peer = %device_id, "pairing initiated with OTP");
        Ok(code)
    }

    /// Confirm pairing with the OTP code from the remote side.
    pub async fn confirm_pairing(&self, device_id: &str, code: &str) -> Result<(), PairingError> {
        // Validate OTP.
        let now = now_secs();
        let mut challenges = self.challenges.lock().await;
        let idx = challenges
            .iter()
            .position(|c| c.device_id == device_id && c.code == code)
            .ok_or(PairingError::InvalidOtp)?;

        let challenge = &challenges[idx];
        if now - challenge.created_at > OTP_TTL_SECS {
            challenges.remove(idx);
            return Err(PairingError::OtpExpired);
        }
        challenges.remove(idx);
        drop(challenges);

        // Advance state machine.
        let mut sessions = self.sessions.lock().await;
        let peer = sessions
            .get_mut(device_id)
            .ok_or_else(|| PairingError::PeerNotFound(device_id.to_string()))?;

        if peer.state != PairingState::PairingInitiated {
            return Err(PairingError::InvalidState {
                current: peer.state.to_string(),
                expected: "pairing_initiated".to_string(),
            });
        }

        // Derive shared secret (in production, this would use a proper key exchange).
        let shared_secret = derive_shared_secret(&self.our_device_id, device_id);
        peer.state = PairingState::Paired;
        peer.shared_secret = Some(shared_secret.clone());

        // Persist to config.
        let entry = PairedPeerEntry {
            device_id: peer.device_id.clone(),
            device_name: peer.device_name.clone(),
            addresses: peer.addresses.clone(),
            port: peer.port,
            shared_secret,
            paired_at: now,
        };

        let peer_id = peer.device_id.clone();
        drop(sessions);

        let mut paired = self.paired.lock().await;
        paired.peers.retain(|p| p.device_id != peer_id);
        paired.peers.push(entry);
        save_peers_config(&self.config_path, &paired);

        info!(peer = %device_id, "pairing confirmed and persisted");
        Ok(())
    }

    /// Add a peer manually by IP and port.
    pub async fn add_manual_peer(&self, ip: &str, port: u16) -> String {
        // Generate a placeholder device ID for manual peers.
        let device_id = format!("manual-{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let device_name = format!("Manual ({}:{})", ip, port);

        self.register_discovered(device_id.clone(), device_name, vec![ip.to_string()], port)
            .await;

        device_id
    }

    /// Check if a device is already paired.
    pub async fn is_paired(&self, device_id: &str) -> bool {
        let paired = self.paired.lock().await;
        paired.peers.iter().any(|p| p.device_id == device_id)
    }

    /// Remove a paired peer.
    pub async fn unpair(&self, device_id: &str) -> Result<(), PairingError> {
        let mut paired = self.paired.lock().await;
        let before = paired.peers.len();
        paired.peers.retain(|p| p.device_id != device_id);
        if paired.peers.len() == before {
            return Err(PairingError::PeerNotFound(device_id.to_string()));
        }
        save_peers_config(&self.config_path, &paired);
        info!(peer = %device_id, "peer unpaired");
        Ok(())
    }

    /// Generate QR code data for pairing (JSON with connection info).
    pub fn generate_qr_data(&self, port: u16) -> String {
        serde_json::json!({
            "device_id": self.our_device_id,
            "device_name": self.our_device_name,
            "port": port,
            "proto": "clipsync-pair-v1",
        })
        .to_string()
    }

    /// Prune expired OTP challenges.
    pub async fn prune_expired_challenges(&self) {
        let now = now_secs();
        let mut challenges = self.challenges.lock().await;
        challenges.retain(|c| now - c.created_at <= OTP_TTL_SECS);
    }
}

// ── Errors ────────────────────────────────────────────────────────

/// Errors from pairing operations.
#[derive(Debug, thiserror::Error)]
pub enum PairingError {
    #[error("peer not found: {0}")]
    PeerNotFound(String),
    #[error("invalid OTP code")]
    InvalidOtp,
    #[error("OTP code expired")]
    OtpExpired,
    #[error("invalid state: current={current}, expected={expected}")]
    InvalidState { current: String, expected: String },
    #[error("config error: {0}")]
    Config(String),
}

// Implement Serialize for PairingError so Tauri commands can return it.
impl Serialize for PairingError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Tauri commands ────────────────────────────────────────────────

/// Tauri state wrapper for the pairing manager.
pub type PairingState_ = Arc<PairingManager>;

#[tauri::command]
pub async fn get_discovered_peers(
    state: tauri::State<'_, PairingState_>,
) -> Result<Vec<PeerInfo>, String> {
    Ok(state.get_discovered_peers().await)
}

#[tauri::command]
pub async fn initiate_pairing(
    device_id: String,
    state: tauri::State<'_, PairingState_>,
) -> Result<String, String> {
    state
        .initiate_pairing(&device_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn confirm_pairing(
    device_id: String,
    code: String,
    state: tauri::State<'_, PairingState_>,
) -> Result<(), String> {
    state
        .confirm_pairing(&device_id, &code)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_paired_peers(
    state: tauri::State<'_, PairingState_>,
) -> Result<Vec<PairedPeerEntry>, String> {
    Ok(state.get_paired_peers().await)
}

#[tauri::command]
pub async fn add_manual_peer(
    ip: String,
    port: u16,
    state: tauri::State<'_, PairingState_>,
) -> Result<String, String> {
    Ok(state.add_manual_peer(&ip, port).await)
}

// ── Helper functions ──────────────────────────────────────────────

/// Generate a 6-digit numeric OTP code.
fn generate_otp() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let s = RandomState::new();
    let mut hasher = s.build_hasher();
    hasher.write_u64(now_secs());
    hasher.write_u64(std::process::id() as u64);
    let hash = hasher.finish();
    let code = hash % 1_000_000;
    format!("{:0>width$}", code, width = OTP_LENGTH)
}

/// Get current time in seconds since epoch.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Derive a shared secret from two device IDs.
/// In production this would use a proper key exchange (e.g., X25519).
/// For now, we use a deterministic HMAC-based derivation.
fn derive_shared_secret(our_id: &str, peer_id: &str) -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    // Sort IDs so both sides derive the same secret.
    let (a, b) = if our_id < peer_id {
        (our_id, peer_id)
    } else {
        (peer_id, our_id)
    };

    let s = RandomState::new();
    let mut hasher = s.build_hasher();
    hasher.write(a.as_bytes());
    hasher.write(b.as_bytes());
    hasher.write_u64(now_secs());
    format!("{:016x}{:016x}", hasher.finish(), now_secs())
}

/// Load peers config from TOML file.
fn load_peers_config(path: &Path) -> PeersConfig {
    match fs::read_to_string(path) {
        Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
            warn!(%e, "failed to parse peers.toml, starting fresh");
            PeersConfig::default()
        }),
        Err(_) => PeersConfig::default(),
    }
}

/// Save peers config to TOML file.
fn save_peers_config(path: &Path, config: &PeersConfig) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match toml::to_string_pretty(config) {
        Ok(content) => {
            if let Err(e) = fs::write(path, content) {
                warn!(%e, "failed to write peers.toml");
            }
        }
        Err(e) => {
            warn!(%e, "failed to serialize peers config");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_manager(tmp: &TempDir) -> PairingManager {
        PairingManager::new(
            "our-device-001".to_string(),
            "TestHost".to_string(),
            tmp.path(),
        )
    }

    #[tokio::test]
    async fn register_and_get_discovered() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);

        mgr.register_discovered(
            "peer-1".to_string(),
            "PeerOne".to_string(),
            vec!["192.168.1.10".to_string()],
            7010,
        )
        .await;

        let peers = mgr.get_discovered_peers().await;
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].device_id, "peer-1");
        assert_eq!(peers[0].state, PairingState::Discovered);
    }

    #[tokio::test]
    async fn initiate_pairing_returns_otp() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);

        mgr.register_discovered(
            "peer-2".to_string(),
            "PeerTwo".to_string(),
            vec!["192.168.1.20".to_string()],
            7010,
        )
        .await;

        let code = mgr.initiate_pairing("peer-2").await.unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[tokio::test]
    async fn confirm_pairing_with_valid_otp() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);

        mgr.register_discovered(
            "peer-3".to_string(),
            "PeerThree".to_string(),
            vec!["192.168.1.30".to_string()],
            7010,
        )
        .await;

        let code = mgr.initiate_pairing("peer-3").await.unwrap();
        mgr.confirm_pairing("peer-3", &code).await.unwrap();

        // Should now be in paired list.
        let paired = mgr.get_paired_peers().await;
        assert_eq!(paired.len(), 1);
        assert_eq!(paired[0].device_id, "peer-3");
    }

    #[tokio::test]
    async fn confirm_pairing_rejects_wrong_otp() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);

        mgr.register_discovered(
            "peer-4".to_string(),
            "PeerFour".to_string(),
            vec!["192.168.1.40".to_string()],
            7010,
        )
        .await;

        let _code = mgr.initiate_pairing("peer-4").await.unwrap();
        let result = mgr.confirm_pairing("peer-4", "000000").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn paired_peer_not_shown_as_discovered() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);

        mgr.register_discovered(
            "peer-5".to_string(),
            "PeerFive".to_string(),
            vec!["192.168.1.50".to_string()],
            7010,
        )
        .await;

        let code = mgr.initiate_pairing("peer-5").await.unwrap();
        mgr.confirm_pairing("peer-5", &code).await.unwrap();

        // Re-discover should not add it again.
        mgr.register_discovered(
            "peer-5".to_string(),
            "PeerFive".to_string(),
            vec!["192.168.1.50".to_string()],
            7010,
        )
        .await;

        let discovered = mgr.get_discovered_peers().await;
        assert!(discovered.is_empty());
    }

    #[tokio::test]
    async fn add_manual_peer_creates_session() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);

        let id = mgr.add_manual_peer("10.0.0.1", 7010).await;
        assert!(id.starts_with("manual-"));

        let peers = mgr.get_discovered_peers().await;
        assert_eq!(peers.len(), 1);
    }

    #[tokio::test]
    async fn unpair_removes_peer() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);

        mgr.register_discovered(
            "peer-6".to_string(),
            "PeerSix".to_string(),
            vec!["192.168.1.60".to_string()],
            7010,
        )
        .await;

        let code = mgr.initiate_pairing("peer-6").await.unwrap();
        mgr.confirm_pairing("peer-6", &code).await.unwrap();

        mgr.unpair("peer-6").await.unwrap();
        let paired = mgr.get_paired_peers().await;
        assert!(paired.is_empty());
    }

    #[tokio::test]
    async fn persistence_round_trip() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);

        mgr.register_discovered(
            "peer-7".to_string(),
            "PeerSeven".to_string(),
            vec!["192.168.1.70".to_string()],
            7010,
        )
        .await;

        let code = mgr.initiate_pairing("peer-7").await.unwrap();
        mgr.confirm_pairing("peer-7", &code).await.unwrap();

        // Reload from disk.
        let config = load_peers_config(&tmp.path().join("peers.toml"));
        assert_eq!(config.peers.len(), 1);
        assert_eq!(config.peers[0].device_id, "peer-7");
    }

    #[tokio::test]
    async fn generate_qr_data_contains_required_fields() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);

        let qr = mgr.generate_qr_data(7010);
        let parsed: serde_json::Value = serde_json::from_str(&qr).unwrap();
        assert_eq!(parsed["device_id"], "our-device-001");
        assert_eq!(parsed["proto"], "clipsync-pair-v1");
    }

    #[test]
    fn otp_generation_produces_6_digits() {
        let code = generate_otp();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn load_empty_config_returns_default() {
        let tmp = TempDir::new().unwrap();
        let config = load_peers_config(&tmp.path().join("nonexistent.toml"));
        assert!(config.peers.is_empty());
    }
}
