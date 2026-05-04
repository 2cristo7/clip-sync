//! Mesh peer discovery for the ClipSync personal desktop app.
//!
//! Each peer advertises itself via mDNS with TXT records indicating its role,
//! protocol version, and unique device ID. Peers continuously browse for other
//! peers on the local network and maintain a discovered-peers list.
//!
//! mDNS does not work over Tailscale (no multicast), so a manual "Add by IP"
//! fallback is also provided.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use thiserror::Error;
use tracing::{debug, info};

use clipsync_protocol::config::MDNS_SERVICE_TYPE;

/// Protocol version advertised in TXT records.
const PROTO_VERSION: &str = "2";

/// TXT record key for peer role.
const TXT_ROLE: &str = "role";

/// TXT record key for protocol version.
const TXT_PROTO: &str = "proto";

/// TXT record key for device ID.
const TXT_ID: &str = "id";

/// Errors that can occur during mesh discovery operations.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("mDNS daemon error: {0}")]
    Daemon(String),

    #[error("service registration failed: {0}")]
    Registration(String),

    #[error("browse operation failed: {0}")]
    Browse(String),
}

/// A discovered mesh peer on the local network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPeer {
    /// Unique device identifier (UUID).
    pub device_id: String,

    /// Hostname of the peer.
    pub hostname: String,

    /// IP addresses where the peer can be reached.
    pub addresses: Vec<IpAddr>,

    /// Port the peer is listening on.
    pub port: u16,

    /// Protocol version advertised by the peer.
    pub proto_version: String,

    /// Whether this peer was added manually via IP (not discovered via mDNS).
    pub manual: bool,
}

/// A manually-added peer (IP fallback for Tailscale or remote networks).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualPeer {
    /// IP address provided by the user.
    pub address: IpAddr,

    /// Port (defaults to protocol default port).
    pub port: u16,
}

/// Guard that stops mDNS advertisement when dropped.
pub struct AdvertiseGuard {
    daemon: ServiceDaemon,
    fullname: String,
}

impl AdvertiseGuard {
    /// Explicitly stop advertising. Called automatically on drop.
    pub fn stop(self) -> Result<(), DiscoveryError> {
        self.daemon
            .unregister(&self.fullname)
            .map_err(|e| DiscoveryError::Registration(e.to_string()))?;
        self.daemon
            .shutdown()
            .map_err(|e| DiscoveryError::Daemon(e.to_string()))?;
        Ok(())
    }
}

impl Drop for AdvertiseGuard {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
    }
}

/// The mesh discovery service: advertises self and browses for peers.
pub struct MeshDiscovery {
    /// Our device ID.
    device_id: String,

    /// Port we are listening on.
    port: u16,

    /// Hostname to advertise.
    hostname: String,

    /// Currently known peers discovered via mDNS.
    peers: Arc<Mutex<HashMap<String, DiscoveredPeer>>>,

    /// Manually added peers (IP fallback).
    manual_peers: Arc<Mutex<Vec<ManualPeer>>>,
}

impl MeshDiscovery {
    /// Create a new mesh discovery instance.
    pub fn new(device_id: String, hostname: String, port: u16) -> Self {
        Self {
            device_id,
            port,
            hostname,
            peers: Arc::new(Mutex::new(HashMap::new())),
            manual_peers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Advertise this peer on the local network via mDNS.
    ///
    /// Returns an `AdvertiseGuard` that unregisters the service when dropped.
    pub fn advertise(&self) -> Result<AdvertiseGuard, DiscoveryError> {
        let daemon = ServiceDaemon::new().map_err(|e| DiscoveryError::Daemon(e.to_string()))?;

        let mut properties = HashMap::new();
        properties.insert(TXT_ROLE.to_string(), PEER_ROLE.to_string());
        properties.insert(TXT_PROTO.to_string(), PROTO_VERSION.to_string());
        properties.insert(TXT_ID.to_string(), self.device_id.clone());

        let service_type = MDNS_SERVICE_TYPE.trim_end_matches('.');
        let instance_name = format!("ClipSync-{}", &self.device_id[..8]);

        let service_info = ServiceInfo::new(
            service_type,
            &instance_name,
            &self.hostname,
            "",
            self.port,
            properties,
        )
        .map_err(|e| DiscoveryError::Registration(e.to_string()))?;

        let fullname = service_info.get_fullname().to_string();

        daemon
            .register(service_info)
            .map_err(|e| DiscoveryError::Registration(e.to_string()))?;

        info!(
            device_id = %self.device_id,
            port = self.port,
            "mDNS mesh peer advertisement started"
        );

        Ok(AdvertiseGuard { daemon, fullname })
    }

    /// Browse the local network for other ClipSync mesh peers.
    ///
    /// This performs a one-shot scan for `timeout` duration and updates the
    /// internal peers list. Returns the current list of discovered peers.
    pub fn browse(&self, timeout: Duration) -> Result<Vec<DiscoveredPeer>, DiscoveryError> {
        let daemon = ServiceDaemon::new().map_err(|e| DiscoveryError::Daemon(e.to_string()))?;

        let service_type = MDNS_SERVICE_TYPE.trim_end_matches('.');
        let receiver = daemon
            .browse(service_type)
            .map_err(|e| DiscoveryError::Browse(e.to_string()))?;

        let deadline = std::time::Instant::now() + timeout;

        while std::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            match receiver.recv_timeout(remaining) {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    self.handle_resolved_service(&info);
                }
                Ok(ServiceEvent::ServiceRemoved(_, fullname)) => {
                    self.handle_removed_service(&fullname);
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }

        let _ = daemon.shutdown();

        let peers = self.peers.lock().unwrap();
        Ok(peers.values().cloned().collect())
    }

    /// Add a peer manually by IP address (fallback for Tailscale/remote).
    pub fn add_manual_peer(&self, address: IpAddr, port: u16) {
        let manual = ManualPeer { address, port };
        let mut manual_peers = self.manual_peers.lock().unwrap();
        if !manual_peers.contains(&manual) {
            info!(%address, port, "manual peer added");
            manual_peers.push(manual);
        }
    }

    /// Remove a manually-added peer.
    pub fn remove_manual_peer(&self, address: IpAddr, port: u16) {
        let mut manual_peers = self.manual_peers.lock().unwrap();
        manual_peers.retain(|p| !(p.address == address && p.port == port));
    }

    /// Get all currently discovered peers (both mDNS and manual).
    pub fn discovered_peers(&self) -> Vec<DiscoveredPeer> {
        let mdns_peers: Vec<DiscoveredPeer> = {
            let peers = self.peers.lock().unwrap();
            peers.values().cloned().collect()
        };

        let manual_peers: Vec<DiscoveredPeer> = {
            let manual = self.manual_peers.lock().unwrap();
            manual
                .iter()
                .map(|m| DiscoveredPeer {
                    device_id: String::new(),
                    hostname: m.address.to_string(),
                    addresses: vec![m.address],
                    port: m.port,
                    proto_version: PROTO_VERSION.to_string(),
                    manual: true,
                })
                .collect()
        };

        let mut all = mdns_peers;
        all.extend(manual_peers);
        all
    }

    /// Get our device ID.
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Handle a resolved mDNS service event.
    fn handle_resolved_service(&self, info: &ServiceInfo) {
        let properties = info.get_properties();

        // Only accept peers with role=peer
        let role = properties
            .get_property_val_str(TXT_ROLE)
            .unwrap_or_default();
        if role != PEER_ROLE {
            debug!(fullname = %info.get_fullname(), "ignoring non-peer service");
            return;
        }

        let device_id = properties
            .get_property_val_str(TXT_ID)
            .unwrap_or_default()
            .to_string();

        // Skip ourselves
        if device_id == self.device_id {
            return;
        }

        let proto = properties
            .get_property_val_str(TXT_PROTO)
            .unwrap_or_default()
            .to_string();

        let addresses: Vec<IpAddr> = info.get_addresses().iter().copied().collect();

        let peer = DiscoveredPeer {
            device_id: device_id.clone(),
            hostname: info.get_hostname().to_string(),
            addresses,
            port: info.get_port(),
            proto_version: proto,
            manual: false,
        };

        info!(
            peer_id = %peer.device_id,
            hostname = %peer.hostname,
            port = peer.port,
            "mesh peer discovered"
        );

        let mut peers = self.peers.lock().unwrap();
        peers.insert(device_id, peer);
    }

    /// Handle a removed mDNS service event.
    fn handle_removed_service(&self, fullname: &str) {
        let mut peers = self.peers.lock().unwrap();
        // Find and remove by matching fullname prefix to device_id
        peers.retain(|id, _| {
            let instance_prefix = format!("ClipSync-{}", &id[..8.min(id.len())]);
            if fullname.starts_with(&instance_prefix) {
                info!(peer_id = %id, "mesh peer removed");
                false
            } else {
                true
            }
        });
    }
}

/// Role value for mesh peers.
const PEER_ROLE: &str = "peer";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_discovery_has_empty_peers() {
        let disc = MeshDiscovery::new(
            "test-id-1234".to_string(),
            "test-host.local.".to_string(),
            7010,
        );
        assert!(disc.discovered_peers().is_empty());
    }

    #[test]
    fn manual_peer_add_remove() {
        let disc = MeshDiscovery::new(
            "test-id-5678".to_string(),
            "test-host.local.".to_string(),
            7010,
        );

        let addr: IpAddr = "192.168.1.100".parse().unwrap();
        disc.add_manual_peer(addr, 7010);
        assert_eq!(disc.discovered_peers().len(), 1);
        assert!(disc.discovered_peers()[0].manual);

        disc.remove_manual_peer(addr, 7010);
        assert!(disc.discovered_peers().is_empty());
    }

    #[test]
    fn duplicate_manual_peer_not_added() {
        let disc = MeshDiscovery::new(
            "test-id-9999".to_string(),
            "test-host.local.".to_string(),
            7010,
        );

        let addr: IpAddr = "10.0.0.5".parse().unwrap();
        disc.add_manual_peer(addr, 7010);
        disc.add_manual_peer(addr, 7010);
        assert_eq!(disc.discovered_peers().len(), 1);
    }

    #[test]
    fn device_id_accessor() {
        let disc = MeshDiscovery::new("my-unique-id".to_string(), "host.local.".to_string(), 7010);
        assert_eq!(disc.device_id(), "my-unique-id");
    }
}
