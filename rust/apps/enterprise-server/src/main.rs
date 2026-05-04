mod cli;
mod config;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Json;
use clap::Parser;
use futures::{SinkExt, StreamExt};
use serde::Serialize;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

use clipsync_crypto::tls::{TlsIdentity, TlsPaths};
use clipsync_protocol::config::VERSION;
use clipsync_protocol::protocol::ClipPayload;
use clipsync_transport::config::WS_PING_INTERVAL;

use crate::cli::Cli;
use crate::config::AppConfig;

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct WsClient {
    #[allow(dead_code)]
    device: String,
    tx: mpsc::UnboundedSender<String>,
}

#[derive(Default)]
struct WsHub {
    clients: RwLock<HashMap<String, WsClient>>,
}

impl WsHub {
    async fn register(&self, device: String, tx: mpsc::UnboundedSender<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.clients.write().await.insert(
            id.clone(),
            WsClient {
                device,
                tx,
            },
        );
        id
    }

    async fn unregister(&self, id: &str) {
        self.clients.write().await.remove(id);
    }

    async fn broadcast(&self, json: &str, exclude: Option<&str>) {
        let clients = self.clients.read().await;
        let mut stale = Vec::new();
        for (id, client) in clients.iter() {
            if exclude == Some(id.as_str()) {
                continue;
            }
            if client.tx.send(json.to_string()).is_err() {
                stale.push(id.clone());
            }
        }
        drop(clients);

        if !stale.is_empty() {
            let mut clients = self.clients.write().await;
            for id in &stale {
                clients.remove(id);
                info!(client_id = %id, "removed stale ws client");
            }
        }
    }

    async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }
}

struct AppState {
    ws_hub: WsHub,
    tls_identity: TlsIdentity,
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
    version: &'static str,
    platform: String,
    mode: &'static str,
    connected_clients: usize,
}

async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        version: VERSION,
        platform: std::env::consts::OS.to_string(),
        mode: "enterprise",
        connected_clients: state.ws_hub.client_count().await,
    })
}

async fn ws_upgrade(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    let client_id = state.ws_hub.register("enterprise-client".to_string(), tx).await;
    info!(client_id = %client_id, "ws client connected");

    let send_task = tokio::spawn(async move {
        let mut ping_ticker = tokio::time::interval(WS_PING_INTERVAL);
        ping_ticker.tick().await; // skip immediate first tick
        loop {
            tokio::select! {
                maybe_msg = rx.recv() => {
                    match maybe_msg {
                        Some(msg) => {
                            if ws_tx.send(Message::Text(msg.into())).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                _ = ping_ticker.tick() => {
                    if ws_tx.send(Message::Ping(Bytes::new())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    let cid = client_id.clone();
    let state_clone = state.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(payload) = serde_json::from_str::<ClipPayload>(&text) {
                        let json = match serde_json::to_string(&payload) {
                            Ok(j) => j,
                            Err(e) => {
                                warn!(error = %e, "failed to re-serialize payload");
                                continue;
                            }
                        };
                        state_clone.ws_hub.broadcast(&json, Some(&cid)).await;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    state.ws_hub.unregister(&client_id).await;
    info!(client_id = %client_id, "ws client disconnected");
}

// ---------------------------------------------------------------------------
// Entrypoint
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let cfg = match AppConfig::load(&cli) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("configuration error: {e}");
            std::process::exit(1);
        }
    };

    // Structured JSON logging to stdout
    let env_filter = tracing_subscriber::EnvFilter::try_new(&cfg.log_level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(env_filter)
        .with_target(true)
        .init();

    info!(
        version = VERSION,
        port = cfg.port,
        bind = %cfg.bind,
        data_dir = %cfg.data_dir.display(),
        "ClipSync Enterprise Server starting"
    );

    // Ensure data directory exists
    if let Err(e) = std::fs::create_dir_all(&cfg.data_dir) {
        error!(error = %e, "failed to create data directory");
        std::process::exit(1);
    }

    // TLS identity
    let tls_paths = TlsPaths {
        cert_der: cfg.data_dir.join("cert.der"),
        key_pem: cfg.data_dir.join("key.pem"),
    };

    let host = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "localhost".to_string());

    let hostnames = vec![
        "localhost".to_string(),
        host.clone(),
        format!("{host}.local"),
    ];

    let ips: Vec<std::net::IpAddr> = local_ip_address::list_afinet_netifas()
        .unwrap_or_default()
        .into_iter()
        .map(|(_, ip)| ip)
        .chain(std::iter::once(std::net::IpAddr::from([127, 0, 0, 1])))
        .collect();

    let tls_identity = match TlsIdentity::load_or_generate(&tls_paths, &hostnames, &ips) {
        Ok(id) => id,
        Err(e) => {
            error!(error = %e, "TLS setup failed");
            std::process::exit(1);
        }
    };

    let state = Arc::new(AppState {
        ws_hub: WsHub::default(),
        tls_identity,
    });

    let app = axum::Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_upgrade))
        .with_state(state.clone());

    let addr = SocketAddr::from((cfg.bind, cfg.port));

    // TLS acceptor
    let tls_config = state
        .tls_identity
        .server_config()
        .expect("failed to build TLS server config");
    let tls_acceptor = tokio_rustls::TlsAcceptor::from(tls_config);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            error!(error = %e, addr = %addr, "failed to bind");
            std::process::exit(2);
        }
    };

    info!(addr = %addr, "server ready, accepting TLS connections");

    // Graceful shutdown on SIGTERM / Ctrl-C
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, peer_addr) = match result {
                    Ok(s) => s,
                    Err(e) => {
                        warn!(error = %e, "accept error");
                        continue;
                    }
                };

                let acceptor = tls_acceptor.clone();
                let app = app.clone();

                tokio::spawn(async move {
                    let tls_stream = match acceptor.accept(stream).await {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::debug!(peer = %peer_addr, error = %e, "TLS handshake failed");
                            return;
                        }
                    };

                    let io = hyper_util::rt::TokioIo::new(tls_stream);

                    use tower::ServiceExt;
                    let svc = app.map_request(move |mut req: axum::http::Request<_>| {
                        req.extensions_mut()
                            .insert(axum::extract::ConnectInfo(peer_addr));
                        req
                    });
                    let service = hyper_util::service::TowerToHyperService::new(svc);

                    if let Err(e) =
                        hyper_util::server::conn::auto::Builder::new(
                            hyper_util::rt::TokioExecutor::new(),
                        )
                        .serve_connection(io, service)
                        .await
                    {
                        tracing::debug!(peer = %peer_addr, error = %e, "connection error");
                    }
                });
            }
            _ = &mut shutdown => {
                info!("shutdown signal received, draining connections");
                // Allow 10 seconds for in-flight WS connections to close
                tokio::time::sleep(Duration::from_secs(10)).await;
                info!("shutdown complete");
                break;
            }
        }
    }
}
