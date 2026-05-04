//! Live policy runtime — in-memory cache of device policies.
//!
//! The [`PolicyRuntime`] is an `Arc`-wrapped concurrent map that the
//! WS hub consults on every frame.  Policy changes via the REST API
//! update the map immediately so live connections see the effect
//! within the next frame (< 1 s latency).

use std::collections::HashMap;
use std::sync::Arc;

use clipsync_policy::Policy;
use tokio::sync::RwLock;
use tracing::info;

use crate::registry::DeviceRegistry;

// ---------------------------------------------------------------------------
// PolicyRuntime
// ---------------------------------------------------------------------------

/// Shared, concurrent policy cache keyed by device ID.
#[derive(Clone)]
pub struct PolicyRuntime {
    inner: Arc<RwLock<HashMap<String, Policy>>>,
}

impl PolicyRuntime {
    /// Create an empty runtime (for tests or before DB is ready).
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load all device policies from the database into memory.
    pub async fn load_from_registry(&self, registry: &DeviceRegistry) {
        match registry.list_devices().await {
            Ok(devices) => {
                let mut map = self.inner.write().await;
                for device in devices {
                    let policy = Policy::from_json_str(&device.policy);
                    map.insert(device.id.clone(), policy);
                }
                info!(count = map.len(), "policy runtime loaded from DB");
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to load policies from DB");
            }
        }
    }

    /// Set the policy for a device (called after DB update).
    pub async fn set_policy(&self, device_id: &str, policy: Policy) {
        info!(device_id = %device_id, policy = %policy, "policy updated in runtime");
        self.inner
            .write()
            .await
            .insert(device_id.to_string(), policy);
    }

    /// Get the policy for a device, defaulting to ReadWrite if unknown.
    pub async fn get_policy(&self, device_id: &str) -> Policy {
        self.inner
            .read()
            .await
            .get(device_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Check whether `device_id` is allowed to push clipboard content.
    pub async fn can_push(&self, device_id: &str) -> bool {
        self.get_policy(device_id).await.can_push()
    }

    /// Check whether `device_id` is allowed to receive content from
    /// `from_device_id`.
    pub async fn can_receive(&self, device_id: &str, from_device_id: &str) -> bool {
        self.get_policy(device_id).await.can_receive(from_device_id)
    }
}

impl Default for PolicyRuntime {
    fn default() -> Self {
        Self::new()
    }
}
