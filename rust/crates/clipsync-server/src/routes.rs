use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Json;
use serde::{Deserialize, Serialize};
use tower_http::limit::RequestBodyLimitLayer;

use clipsync_core::config::{MAX_PAYLOAD_BYTES, VERSION, WS_PING_INTERVAL};
use clipsync_core::pairing::{create_pair_response, PairingError};
use clipsync_core::protocol::{unix_millis, ClipPayload};

use crate::auth::auth_layer;
use crate::errors::{classify_decode_failure, InjectError};
use crate::rate_limit::{rate_limit_layer, RateLimiter};
use crate::AppState;

/// Build the main Axum router with all endpoints.
///
/// Uses a fresh [`RateLimiter`] — call [`build_router_with_limiter`] if
/// you need to share the limiter state across multiple router instances
/// (e.g. for a "server restart" path that keeps in-flight throttling).
pub fn build_router(state: Arc<AppState>) -> axum::Router {
    build_router_with_limiter(state, RateLimiter::new())
}

/// Build the router with an externally-supplied rate limiter.
///
/// Tower layer order is **bottom-up**: the layer added last runs first.
/// We register them in this order so the request flow is:
///
/// ```text
/// rate_limit_layer  (outermost — runs first; throttles bad-token storms
///                    BEFORE auth would 401 them)
///   └─ auth_layer            (Bearer + HMAC for /inject)
///      └─ RequestBodyLimitLayer
///         └─ route handler
/// ```
///
/// This is the fix for the original Mac bug ported here from
/// `e2cb5451`: previously the limiter ran inside the `/inject` handler,
/// so attackers spamming bad tokens hit the 401 path forever and the
/// limiter never engaged.
pub fn build_router_with_limiter(state: Arc<AppState>, limiter: RateLimiter) -> axum::Router {
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
        .layer(axum::middleware::from_fn_with_state(
            limiter,
            rate_limit_layer,
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

/// Body shape for `/pair` 401 responses.
///
/// Note: this is intentionally `{"error": "<code>"}` — there is no
/// `message` field. `/inject` uses a different shape
/// (`{error, message}`, see [`crate::errors::InjectError`]). The plan
/// (Phase 1.5) keeps these separate on purpose: `/pair` codes mirror the
/// Mac Swift vocabulary verbatim (`invalid | expired | consumed |
/// notStarted`) so Android/Tauri clients can decode them as a closed enum
/// without needing to inspect a free-form message.
#[derive(Serialize)]
struct PairErrorBody {
    error: &'static str,
}

/// Convert a [`PairingError`] into the HTTP response the wire protocol
/// expects: `401 Unauthorized` with `{"error": "<code>"}`.
fn pair_error_response(err: PairingError) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(PairErrorBody { error: err.code() }),
    )
        .into_response()
}

async fn pair(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<PairQuery>,
) -> Response {
    let device = headers
        .get("X-ClipSync-Device")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    // Validate pairing code — surface the precise PairingError variant so
    // the 401 body carries the matching wire code.
    {
        let mut pm = state.pairing_manager.write().await;
        if let Err(e) = pm.validate_and_consume(&query.code) {
            return pair_error_response(e);
        }
    }

    // Generate pairing response (token + secret)
    let signing_secret = b"clipsync-pairing"; // TODO: use persistent secret
    let resp = create_pair_response(signing_secret);

    // Register the token
    let token_bytes = resp.token.as_bytes();
    let mut ts = state.token_store.write().await;
    if let Err(e) = ts.register(token_bytes, &device) {
        tracing::error!("Failed to register token: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if let Err(e) = ts.save() {
        tracing::error!("Failed to save token store: {e}");
    }

    Json(resp).into_response()
}

// --- /inject ---

#[derive(Serialize)]
struct InjectResponse {
    ok: bool,
    nonce: String,
}

/// `POST /inject`.
///
/// All 4xx responses use the standardized body shape from
/// [`crate::errors::InjectError`]:
///
/// ```json
/// { "error": "<machine-code>", "message": "<human-readable>" }
/// ```
///
/// Codes returned: `decode_error`, `timestamp_out_of_range`,
/// `payload_too_large`, `unsupported_kind`. See `errors.rs` for details.
async fn inject(
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> Result<Json<InjectResponse>, InjectError> {
    // 1. Body size — explicit check so the response is our standardized shape
    //    rather than the layer's default 413 plain-text body.
    if body.len() > MAX_PAYLOAD_BYTES {
        return Err(InjectError::PayloadTooLarge(format!(
            "body is {} bytes, max {MAX_PAYLOAD_BYTES} bytes",
            body.len()
        )));
    }

    // 2. Decode + classify (decode_error vs unsupported_kind).
    let payload: ClipPayload =
        serde_json::from_slice(&body).map_err(|_| classify_decode_failure(&body))?;

    // 3. Validate (timestamp window, etc.).
    //    ClipPayload.ts is in MILLISECONDS. See CLAUDE.md §"Wire Protocol Invariants".
    let now_ms = unix_millis() as i64;
    payload.validate(now_ms).map_err(InjectError::from)?;

    let nonce = uuid::Uuid::new_v4().to_string();

    // Broadcast to all connected WebSocket clients
    state.ws_hub.broadcast(&payload, None).await;

    // Write to local clipboard
    // (clipboard injection handled by clipboard_injector module)

    Ok(Json(InjectResponse { ok: true, nonce }))
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

    // Forward hub messages to WebSocket and emit periodic Ping frames so
    // the client can detect a half-open link via its read timeout. The
    // ping interval is the single-source-of-truth constant from
    // `clipsync_core::config` (mirrors Mac fc9b1d38).
    let send_task = tokio::spawn(async move {
        let mut ping_ticker = tokio::time::interval(WS_PING_INTERVAL);
        // First tick fires immediately; skip it so we don't ping before
        // the upgrade handshake-side state is fully settled on the wire.
        ping_ticker.tick().await;
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
