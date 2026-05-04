mod cli;
mod config;
mod policy_runtime;
mod registry;
mod ws_handler;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::{get, put};
use axum::Json;
use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

use clipsync_crypto::tls::{TlsIdentity, TlsPaths};
use clipsync_policy::Policy;
use clipsync_protocol::config::VERSION;

use crate::cli::Cli;
use crate::config::AppConfig;
use crate::policy_runtime::PolicyRuntime;
use crate::registry::DeviceRegistry;

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct WsClient {
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

    /// Broadcast a message to all connected clients, respecting policies.
    /// `from_device_id` is the device label of the sender (used for
    /// FollowLeader checks).
    async fn broadcast_with_policy(
        &self,
        json: &str,
        exclude: Option<&str>,
        from_device_id: &str,
        policy_runtime: &PolicyRuntime,
    ) {
        let clients = self.clients.read().await;
        let mut stale = Vec::new();
        for (id, client) in clients.iter() {
            if exclude == Some(id.as_str()) {
                continue;
            }
            // Check if recipient's policy allows receiving from sender
            if !policy_runtime.can_receive(&client.device, from_device_id).await {
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

    /// Legacy broadcast without policy checks (for backward compat paths).
    #[allow(dead_code)]
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

pub(crate) struct AppState {
    ws_hub: WsHub,
    tls_identity: TlsIdentity,
    registry: DeviceRegistry,
    policy_runtime: PolicyRuntime,
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
    ws.on_upgrade(move |socket| ws_handler::handle_ws(socket, state))
}

// ---------------------------------------------------------------------------
// Policy endpoint
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct UpdatePolicyRequest {
    policy: Policy,
}

#[derive(Serialize)]
struct DeviceResponse {
    id: String,
    name: String,
    role: String,
    policy: Policy,
}

async fn update_device_policy(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
    Json(body): Json<UpdatePolicyRequest>,
) -> Result<Json<DeviceResponse>, axum::http::StatusCode> {
    let policy_json = body.policy.to_json_string();

    let device = state
        .registry
        .update_device_policy(&device_id, &policy_json)
        .await
        .map_err(|e| {
            warn!(error = %e, device_id = %device_id, "failed to update policy");
            axum::http::StatusCode::NOT_FOUND
        })?;

    // Update live runtime immediately
    state.policy_runtime.set_policy(&device_id, body.policy.clone()).await;

    info!(device_id = %device_id, policy = %body.policy, "device policy updated via API");

    Ok(Json(DeviceResponse {
        id: device.id,
        name: device.name,
        role: device.role,
        policy: body.policy,
    }))
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

    // Device registry (SQLite)
    let registry = match DeviceRegistry::init(&cfg.data_dir).await {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "failed to initialise device registry");
            std::process::exit(1);
        }
    };

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

    // Policy runtime — load live policies from DB
    let policy_runtime = PolicyRuntime::new();
    policy_runtime.load_from_registry(&registry).await;

    let state = Arc::new(AppState {
        ws_hub: WsHub::default(),
        tls_identity,
        registry,
        policy_runtime,
    });

    let app = axum::Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_upgrade))
        .route("/devices/{id}/policy", put(update_device_policy))
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
