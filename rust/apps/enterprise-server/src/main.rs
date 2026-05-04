mod audit;
mod cli;
mod config;
mod policy_runtime;
mod registry;
mod routes;
mod ws_handler;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::Json;
use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

use clipsync_crypto::tls::{TlsIdentity, TlsPaths};
use clipsync_policy::Policy;
use clipsync_protocol::config::VERSION;

use crate::audit::AuditLog;
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

use crate::routes::broadcast::{DeliveryState, DeviceDeliveryStatus, PendingBroadcast};

#[derive(Default)]
struct WsHub {
    clients: RwLock<HashMap<String, WsClient>>,
    /// Pending broadcasts awaiting offline clients (within 1h window).
    pending_broadcasts: RwLock<Vec<PendingBroadcast>>,
}

impl WsHub {
    async fn register(&self, device: String, tx: mpsc::UnboundedSender<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.clients
            .write()
            .await
            .insert(id.clone(), WsClient { device, tx });
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
            if !policy_runtime
                .can_receive(&client.device, from_device_id)
                .await
            {
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

    /// Send a WS message to specific device IDs. Returns per-device delivery status.
    async fn send_to_devices(
        &self,
        device_ids: &[String],
        json: &str,
    ) -> Vec<DeviceDeliveryStatus> {
        let clients = self.clients.read().await;
        let mut results = Vec::with_capacity(device_ids.len());

        for target_id in device_ids {
            // Find a client whose device label matches
            let sent = clients
                .values()
                .filter(|c| &c.device == target_id)
                .any(|c| c.tx.send(json.to_string()).is_ok());

            results.push(DeviceDeliveryStatus {
                device_id: target_id.clone(),
                status: if sent {
                    DeliveryState::Delivered
                } else {
                    DeliveryState::Pending
                },
            });
        }

        results
    }

    /// Send a WS message to a single device (best-effort, no status return).
    async fn send_to_device(&self, device_id: &str, json: &str) {
        let clients = self.clients.read().await;
        for client in clients.values() {
            if client.device == device_id {
                let _ = client.tx.send(json.to_string());
            }
        }
    }

    /// Queue a pending broadcast for offline delivery.
    async fn queue_pending_broadcast(&self, pb: PendingBroadcast) {
        self.pending_broadcasts.write().await.push(pb);
    }

    /// Remove expired pending broadcasts (older than 1 hour).
    async fn expire_pending_broadcasts(&self) {
        let mut pending = self.pending_broadcasts.write().await;
        let before = pending.len();
        pending.retain(|pb| !pb.is_expired());
        let removed = before - pending.len();
        if removed > 0 {
            info!(removed = removed, "expired pending broadcasts cleaned up");
        }
    }

    /// Deliver any pending broadcasts to a device that just reconnected.
    async fn deliver_pending_to_device(&self, device_id: &str) {
        let mut pending = self.pending_broadcasts.write().await;
        let mut fully_delivered = Vec::new();

        for (idx, pb) in pending.iter_mut().enumerate() {
            if pb.is_expired() {
                continue;
            }
            if !pb.target_device_ids.contains(&device_id.to_string()) {
                continue;
            }
            if pb.delivered_to.contains(&device_id.to_string()) {
                continue;
            }

            // Try to deliver
            let clients = self.clients.read().await;
            let sent = clients
                .values()
                .filter(|c| c.device == device_id)
                .any(|c| c.tx.send(pb.to_ws_frame_json()).is_ok());
            drop(clients);

            if sent {
                pb.delivered_to.push(device_id.to_string());
                info!(
                    broadcast_id = %pb.id,
                    device_id = %device_id,
                    "delivered pending broadcast on reconnect"
                );

                // Emit delivery status update
                let status_event = serde_json::json!({
                    "type": "BroadcastStatus",
                    "broadcast_id": pb.id,
                    "delivery_status": [{
                        "device_id": device_id,
                        "status": "delivered",
                    }],
                });
                let clients = self.clients.read().await;
                for client in clients.values() {
                    if client.device == pb.sender_device_id {
                        let _ = client.tx.send(status_event.to_string());
                    }
                }

                // Check if all targets delivered
                if pb
                    .target_device_ids
                    .iter()
                    .all(|t| pb.delivered_to.contains(t))
                {
                    fully_delivered.push(idx);
                }
            }
        }

        // Remove fully-delivered broadcasts (in reverse to preserve indices)
        for idx in fully_delivered.into_iter().rev() {
            pending.remove(idx);
        }
    }
}

pub(crate) struct AppState {
    ws_hub: WsHub,
    tls_identity: TlsIdentity,
    registry: DeviceRegistry,
    policy_runtime: PolicyRuntime,
    data_dir: std::path::PathBuf,
    pub(crate) audit_log: AuditLog,
}

impl AppState {
    pub(crate) fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }
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

async fn ws_upgrade(State(state): State<Arc<AppState>>, ws: WebSocketUpgrade) -> impl IntoResponse {
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
    state
        .policy_runtime
        .set_policy(&device_id, body.policy.clone())
        .await;

    // Audit: policy_changed
    state
        .audit_log
        .log(audit::AuditEvent::policy_changed(&device_id, &policy_json))
        .await;

    info!(device_id = %device_id, policy = %body.policy, "device policy updated via API");

    Ok(Json(DeviceResponse {
        id: device.id,
        name: device.name,
        role: device.role,
        policy: body.policy,
    }))
}

// ---------------------------------------------------------------------------
// Audit query endpoint
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct AuditQueryParams {
    from: Option<String>,
    to: Option<String>,
    device_id: Option<String>,
    event_type: Option<String>,
    limit: Option<u32>,
}

async fn get_audit(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<AuditQueryParams>,
) -> Result<Json<Vec<clipsync_storage::models::AuditEntry>>, axum::http::StatusCode> {
    let limit = params.limit.unwrap_or(100).min(1000);

    let entries = state
        .audit_log
        .db()
        .query_audit(
            params.from.as_deref(),
            params.to.as_deref(),
            params.device_id.as_deref(),
            params.event_type.as_deref(),
            limit,
        )
        .await
        .map_err(|e| {
            warn!(error = %e, "audit query failed");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(entries))
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

    // Ensure data directory and broadcasts sub-directory exist
    if let Err(e) = std::fs::create_dir_all(&cfg.data_dir) {
        error!(error = %e, "failed to create data directory");
        std::process::exit(1);
    }
    if let Err(e) = std::fs::create_dir_all(cfg.data_dir.join("broadcasts")) {
        error!(error = %e, "failed to create broadcasts directory");
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

    // Audit log — shares the same SQLite database as the registry
    let audit_db =
        match clipsync_storage::db::Database::new(&cfg.data_dir.join("clipsync.db")).await {
            Ok(db) => db,
            Err(e) => {
                error!(error = %e, "failed to open audit database");
                std::process::exit(1);
            }
        };
    let audit_retention_days = cfg.audit_retention_days.unwrap_or(30);
    let audit_log = AuditLog::new(audit_db, audit_retention_days);
    audit::spawn_audit_purge_task(audit_log.clone());
    info!(
        retention_days = audit_retention_days,
        "audit log initialised"
    );

    let state = Arc::new(AppState {
        ws_hub: WsHub::default(),
        tls_identity,
        registry,
        policy_runtime,
        data_dir: cfg.data_dir.clone(),
        audit_log,
    });

    // Spawn background task to expire old broadcasts
    {
        let hub = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));
            loop {
                interval.tick().await;
                hub.ws_hub.expire_pending_broadcasts().await;
                // Clean up expired files on disk
                let dir = hub.data_dir().join("broadcasts");
                if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        if let Ok(meta) = entry.metadata().await {
                            if let Ok(modified) = meta.modified() {
                                if modified.elapsed().unwrap_or_default().as_secs() >= 3600 {
                                    let _ = tokio::fs::remove_file(entry.path()).await;
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    let app = axum::Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_upgrade))
        .route("/devices/{id}/policy", put(update_device_policy))
        .route("/broadcast", post(routes::broadcast::post_broadcast))
        .route("/audit", get(get_audit))
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
