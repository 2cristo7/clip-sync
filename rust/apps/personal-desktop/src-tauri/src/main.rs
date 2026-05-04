//! ClipSync Personal Desktop — Tauri binary entry point wiring mesh discovery.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;

use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use clipsync_personal_tauri_lib::discovery::MeshDiscovery;
use clipsync_personal_tauri_lib::{config_dir, load_or_create_device_id, local_hostname};
use clipsync_protocol::config::PORT;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config_path = config_dir();
    let device_id = load_or_create_device_id(&config_path);
    let hostname = format!("{}.", local_hostname());

    info!(device_id = %device_id, "ClipSync personal desktop starting");

    let discovery = MeshDiscovery::new(device_id, hostname, PORT);

    // Start advertising our presence on the LAN.
    let _guard = match discovery.advertise() {
        Ok(guard) => {
            info!("mDNS advertisement active");
            guard
        }
        Err(e) => {
            error!(%e, "failed to start mDNS advertisement");
            return;
        }
    };

    // Perform an initial peer scan.
    match discovery.browse(Duration::from_secs(3)) {
        Ok(peers) => {
            info!(count = peers.len(), "initial peer scan complete");
            for peer in &peers {
                info!(
                    id = %peer.device_id,
                    host = %peer.hostname,
                    port = peer.port,
                    "found peer"
                );
            }
        }
        Err(e) => {
            error!(%e, "peer scan failed");
        }
    }

    // Hand off to Tauri event loop.
    clipsync_personal_tauri_lib::run();
}
