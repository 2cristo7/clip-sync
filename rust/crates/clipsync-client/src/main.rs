use std::path::PathBuf;

use clap::Parser;
use tracing::info;

use clipsync_client::credentials::ClientCredentials;

/// ClipSync client — connects to a ClipSync server for clipboard synchronization.
#[derive(Parser, Debug)]
#[command(name = "clipsync-client", version = clipsync_core::config::VERSION)]
pub struct Cli {
    /// Server address (ip:port). If omitted, uses mDNS discovery.
    #[arg(long)]
    server: Option<String>,

    /// Disable the system tray icon.
    #[arg(long, default_value_t = false)]
    no_tray: bool,

    /// Data directory for credentials and config.
    /// Defaults to ~/.clipsync/
    #[arg(long)]
    data_dir: Option<PathBuf>,

    /// Force re-pairing even if credentials exist.
    #[arg(long, default_value_t = false)]
    repair: bool,

    /// Pairing code (6 digits). If omitted, will prompt interactively.
    #[arg(long)]
    code: Option<String>,

    /// Device label sent during pairing.
    #[arg(long, default_value = "rust-client")]
    device_label: String,
}

impl Cli {
    /// Resolve the data directory, creating it if necessary.
    pub fn data_dir(&self) -> PathBuf {
        self.data_dir.clone().unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".clipsync")
        })
    }

    /// Path to the credentials file.
    pub fn creds_path(&self) -> PathBuf {
        self.data_dir().join("client_creds.json")
    }
}

fn main() {
    // Install the default rustls crypto provider (ring).
    let _ = rustls::crypto::ring::default_provider().install_default();

    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!(
        "ClipSync client v{} starting",
        clipsync_core::config::VERSION
    );
    info!("data dir: {}", cli.data_dir().display());

    // Create data directory
    let data_dir = cli.data_dir();
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        tracing::error!("failed to create data directory: {}", e);
        std::process::exit(1);
    }

    let creds_path = cli.creds_path();
    let has_creds = ClientCredentials::exists(&creds_path);

    if has_creds && !cli.repair {
        match ClientCredentials::load(&creds_path) {
            Ok(creds) => {
                info!(
                    "loaded credentials for server {}:{}",
                    creds.host, creds.port
                );
                info!("would start connector + clipboard watcher (not yet implemented)");
            }
            Err(e) => {
                tracing::error!("failed to load credentials: {}", e);
                tracing::error!("run with --repair to re-pair");
                std::process::exit(1);
            }
        }
    } else {
        info!("no credentials found, pairing required");
        if let Some(ref server) = cli.server {
            info!("manual mode: server={}", server);
        } else {
            info!("auto mode: will use mDNS discovery");
        }
        info!("pairing flow not yet implemented");
    }
}
