pub mod auth;
pub mod clipboard_injector;
pub mod clipboard_watcher;
pub mod errors;
pub mod rate_limit;
pub mod routes;
pub mod tray;
pub mod ws_hub;

use std::path::PathBuf;

use clipsync_core::pairing::PairingManager;
use clipsync_core::tls::TlsIdentity;
use clipsync_core::token_store::TokenStore;
use tokio::sync::RwLock;

use crate::ws_hub::WsHub;

/// Shared application state accessible from all routes and background tasks.
pub struct AppState {
    pub token_store: RwLock<TokenStore>,
    pub pairing_manager: RwLock<PairingManager>,
    pub ws_hub: WsHub,
    pub tls_identity: TlsIdentity,
    pub data_dir: PathBuf,
}
