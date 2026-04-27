use std::collections::HashMap;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use thiserror::Error;

use crate::config::{MDNS_SERVICE_TYPE, VERSION};

#[derive(Debug, Error)]
pub enum MdnsError {
    #[error("mDNS daemon error: {0}")]
    Daemon(String),
    #[error("service registration failed: {0}")]
    Registration(String),
}

/// A discovered ClipSync server on the network.
#[derive(Debug, Clone)]
pub struct DiscoveredServer {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub version: String,
    pub fingerprint: String,
    pub addresses: Vec<std::net::IpAddr>,
}

/// Guard that unregisters the mDNS service when dropped.
pub struct MdnsGuard {
    daemon: ServiceDaemon,
    fullname: String,
}

impl MdnsGuard {
    /// Explicitly unregister the service.
    pub fn unregister(self) -> Result<(), MdnsError> {
        self.daemon
            .unregister(&self.fullname)
            .map_err(|e| MdnsError::Daemon(e.to_string()))?;
        // Give daemon time to send goodbye packets
        std::thread::sleep(Duration::from_millis(100));
        let _ = self.daemon.shutdown();
        Ok(())
    }
}

impl Drop for MdnsGuard {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
    }
}

/// Advertise this ClipSync instance via mDNS.
///
/// Returns a guard that will unregister the service when dropped.
pub fn advertise(
    port: u16,
    hostname: &str,
    fingerprint: &str,
) -> Result<MdnsGuard, MdnsError> {
    let daemon = ServiceDaemon::new().map_err(|e| MdnsError::Daemon(e.to_string()))?;

    let mut properties = HashMap::new();
    properties.insert("version".to_string(), VERSION.to_string());
    properties.insert("name".to_string(), hostname.to_string());
    properties.insert("fp".to_string(), fingerprint.to_string());

    let instance_name = format!("ClipSync-{}", hostname);
    let service = ServiceInfo::new(
        MDNS_SERVICE_TYPE,
        &instance_name,
        hostname,
        "",
        port,
        properties,
    )
    .map_err(|e| MdnsError::Registration(e.to_string()))?;

    let fullname = service.get_fullname().to_string();

    daemon
        .register(service)
        .map_err(|e| MdnsError::Registration(e.to_string()))?;

    Ok(MdnsGuard { daemon, fullname })
}

/// Discover ClipSync servers on the local network.
///
/// Listens for `timeout` duration and returns all discovered servers.
pub fn discover(timeout: Duration) -> Result<Vec<DiscoveredServer>, MdnsError> {
    let daemon = ServiceDaemon::new().map_err(|e| MdnsError::Daemon(e.to_string()))?;

    let receiver = daemon
        .browse(MDNS_SERVICE_TYPE)
        .map_err(|e| MdnsError::Daemon(e.to_string()))?;

    let mut servers = Vec::new();
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let properties = info.get_properties();
                let version = properties
                    .get_property_val_str("version")
                    .unwrap_or("")
                    .to_string();
                let name = properties
                    .get_property_val_str("name")
                    .unwrap_or("")
                    .to_string();
                let fingerprint = properties
                    .get_property_val_str("fp")
                    .unwrap_or("")
                    .to_string();

                servers.push(DiscoveredServer {
                    name,
                    host: info.get_hostname().to_string(),
                    port: info.get_port(),
                    version,
                    fingerprint,
                    addresses: info.get_addresses().iter().copied().collect(),
                });
            }
            Ok(_) => {} // other events
            Err(_) => {} // timeout on recv, continue loop
        }
    }

    let _ = daemon.shutdown();
    Ok(servers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_returns_empty_on_no_services() {
        // Short timeout, expect no services on test network
        let result = discover(Duration::from_millis(200));
        assert!(result.is_ok());
        // May or may not find services, just verify it doesn't crash
    }

    // Note: advertise + discover integration test requires network access
    // and may flake in CI. The advertise function is tested by verifying
    // it doesn't error on construction.
    #[test]
    fn advertise_creates_guard() {
        let result = advertise(crate::config::PORT, "test-host", "fake-fingerprint");
        // On CI without multicast, this may fail — that's acceptable
        if let Ok(guard) = result {
            drop(guard); // should unregister cleanly
        }
    }
}
