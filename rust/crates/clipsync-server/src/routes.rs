use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Json;
use serde::{Deserialize, Serialize};
use tower_http::limit::RequestBodyLimitLayer;

use clipsync_core::config::{MAX_PAYLOAD_BYTES, VERSION};
use clipsync_core::pairing::create_pair_response;
use clipsync_core::protocol::ClipPayload;

use crate::auth::auth_layer;
use crate::AppState;

/// Build the main Axum router with all endpoints.
pub fn build_router(state: Arc<AppState>) -> axum::Router {
    axum::Router::new()
        .route("/health", get(health))
        .route("/pair", get(pair))
        .route("/inject", post(inject))
        .route("/ws", get(ws_upgrade))
        .layer(RequestBodyLimitLayer::new(MAX_PAYLOAD_BYTES))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_layer,
        ))
        .with_state(state)
}

// --- /health ---

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
    version: &'static str,
    platform: String,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        version: VERSION,
        platform: std::env::consts::OS.to_string(),
    })
}

// --- /pair ---

#[derive(Deserialize)]
struct PairQuery {
    code: String,
}

async fn pair(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<PairQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let device = headers
        .get("X-ClipSync-Device")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    // Validate pairing code
    let mut pm = state.pairing_manager.write().await;
    if pm.validate_and_consume(&query.code).is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Generate pairing response (token + secret)
    let signing_secret = b"clipsync-pairing"; // TODO: use persistent secret
    let resp = create_pair_response(signing_secret);

    // Register the token
    let token_bytes = resp.token.as_bytes();
    let mut ts = state.token_store.write().await;
    if let Err(e) = ts.register(token_bytes, &device) {
        tracing::error!("Failed to register token: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    if let Err(e) = ts.save() {
        tracing::error!("Failed to save token store: {e}");
    }

    Ok(Json(resp))
}

// --- /inject ---

#[derive(Serialize)]
struct InjectResponse {
    ok: bool,
    nonce: String,
}

async fn inject(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ClipPayload>,
) -> Json<InjectResponse> {
    let nonce = uuid::Uuid::new_v4().to_string();

    // Broadcast to all connected WebSocket clients
    state.ws_hub.broadcast(&payload, None).await;

    // Write to local clipboard
    // (clipboard injection handled by clipboard_injector module)

    Json(InjectResponse { ok: true, nonce })
}

// --- /ws ---

async fn ws_upgrade(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let device = headers
        .get("X-ClipSync-Device")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    ws.on_upgrade(move |socket| handle_ws(socket, state, device))
}

async fn handle_ws(socket: WebSocket, state: Arc<AppState>, device: String) {
    use futures::{SinkExt, StreamExt};

    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    let client_id = state.ws_hub.register(device.clone(), tx).await;
    tracing::info!("WebSocket connected: {device} ({client_id})");

    // Forward hub messages to WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Receive messages from WebSocket and broadcast
    let hub = state.ws_hub.clone();
    let cid = client_id.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(payload) = serde_json::from_str::<ClipPayload>(&text) {
                        hub.broadcast(&payload, Some(&cid)).await;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    state.ws_hub.unregister(&client_id).await;
    tracing::info!("WebSocket disconnected: {device} ({client_id})");
}
