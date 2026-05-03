use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use clipsync_core::config::{PORT, VERSION};
use clipsync_core::pairing::PairingManager;
use clipsync_core::tls::{TlsIdentity, TlsPaths};
use clipsync_core::token_store::TokenStore;
use tokio::sync::RwLock;
use tracing::{error, info};

use clipsync_server::routes;
use clipsync_server::ws_hub::WsHub;
use clipsync_server::AppState;

/// ClipSync server — real-time clipboard synchronization over LAN/Tailscale
#[derive(Parser, Debug)]
#[command(name = "clipsync-server", version = VERSION)]
pub struct Cli {
    /// TCP port to listen on
    #[arg(short, long, default_value_t = PORT)]
    pub port: u16,

    /// Data directory for tokens, TLS certs, and config
    #[arg(long, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,

    /// Disable the system tray icon
    #[arg(long)]
    pub no_tray: bool,
}

fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".clipsync")
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let data_dir = cli.data_dir.unwrap_or_else(default_data_dir);

    info!("ClipSync server v{VERSION} starting");
    info!("Data directory: {}", data_dir.display());

    // Ensure data directory exists
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        error!("Failed to create data directory: {e}");
        std::process::exit(1);
    }

    // Load or generate TLS identity
    let tls_paths = TlsPaths {
        cert_der: data_dir.join("cert.der"),
        key_pem: data_dir.join("key.pem"),
    };
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "localhost".to_string());

    let hostnames = vec![
        "localhost".to_string(),
        hostname.clone(),
        format!("{hostname}.local"),
    ];

    // Gather local IPs for TLS SANs
    let ips: Vec<std::net::IpAddr> = local_ip_address::list_afinet_netifas()
        .unwrap_or_default()
        .into_iter()
        .map(|(_, ip)| ip)
        .chain(std::iter::once(std::net::IpAddr::from([127, 0, 0, 1])))
        .collect();

    let tls_identity = match TlsIdentity::load_or_generate(&tls_paths, &hostnames, &ips) {
        Ok(id) => id,
        Err(e) => {
            error!("TLS setup failed: {e}");
            std::process::exit(1);
        }
    };

    // Load token store
    let token_path = data_dir.join("tokens.json");
    let token_store = match TokenStore::load(token_path) {
        Ok(ts) => ts,
        Err(e) => {
            error!("Failed to load token store: {e}");
            std::process::exit(1);
        }
    };

    let state = Arc::new(AppState {
        token_store: RwLock::new(token_store),
        pairing_manager: RwLock::new(PairingManager::new()),
        ws_hub: WsHub::new(),
        tls_identity,
        data_dir,
    });

    // Build the Axum router
    let app = routes::build_router(state.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], cli.port));
    info!("Listening on https://{addr}");

    // Build TLS acceptor
    let tls_config = state
        .tls_identity
        .server_config()
        .expect("Failed to build TLS server config");
    let tls_acceptor = tokio_rustls::TlsAcceptor::from(tls_config);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind to {addr}: {e}");
            std::process::exit(1);
        }
    };

    info!("Server ready — accepting connections");

    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                error!("Accept error: {e}");
                continue;
            }
        };

        let acceptor = tls_acceptor.clone();
        let app = app.clone();

        tokio::spawn(async move {
            let tls_stream = match acceptor.accept(stream).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::debug!("TLS handshake failed from {peer_addr}: {e}");
                    return;
                }
            };

            let io = hyper_util::rt::TokioIo::new(tls_stream);

            // Inject the peer's `SocketAddr` as `ConnectInfo` so the
            // rate-limit middleware can throttle per-IP.
            // (Hummingbird gives this for free; with hyper-util we wrap
            // the service to add it manually.)
            use tower::ServiceExt;
            let svc = app.map_request(move |mut req: axum::http::Request<_>| {
                req.extensions_mut()
                    .insert(axum::extract::ConnectInfo(peer_addr));
                req
            });
            let service = hyper_util::service::TowerToHyperService::new(svc);

            if let Err(e) =
                hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new())
                    .serve_connection(io, service)
                    .await
            {
                tracing::debug!("Connection error from {peer_addr}: {e}");
            }
        });
    }
}
