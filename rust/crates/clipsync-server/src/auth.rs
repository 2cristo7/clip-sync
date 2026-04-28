use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use clipsync_core::config::HMAC_MAX_SKEW_SECS;
use clipsync_core::hmac;

use crate::AppState;

/// Auth middleware layer.
///
/// - `/health` and `/pair`: no auth required (pass through).
/// - `/ws`: Bearer token required (no HMAC).
/// - `/inject`: Bearer token + HMAC signature required.
/// - All other routes: Bearer token required.
pub async fn auth_layer(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();

    // Public routes — no auth needed
    if path == "/health" || path == "/pair" {
        return next.run(req).await;
    }

    // Extract Bearer token
    let token = match extract_bearer(req.headers()) {
        Some(t) => t,
        None => return unauthorized(),
    };

    // Validate token exists in store
    {
        let mut store = state.token_store.write().await;
        if store.validate(token.as_bytes()).is_err() {
            return unauthorized();
        }
    }

    // For /inject, also validate HMAC
    if path == "/inject" {
        let hmac_header = match req.headers().get("X-ClipSync-Signature") {
            Some(h) => match h.to_str() {
                Ok(s) => s.to_string(),
                Err(_) => return unauthorized(),
            },
            None => return unauthorized(),
        };

        // We need to read the body to verify HMAC, then reconstruct the request.
        // For now, HMAC verification is deferred to the handler or done via a body extractor.
        // This is a known limitation — full HMAC verification requires body buffering.
        let (parts, body) = req.into_parts();
        let body_bytes = match axum::body::to_bytes(body, clipsync_core::config::MAX_PAYLOAD_BYTES)
            .await
        {
            Ok(b) => b,
            Err(_) => return unauthorized(),
        };

        // Look up the secret for this token
        // For now use a default pairing secret — in production this would be
        // looked up per-token from the pairing exchange.
        let secret = b"clipsync-pairing".to_vec();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if hmac::verify(&secret, &hmac_header, &body_bytes, now, HMAC_MAX_SKEW_SECS).is_err() {
            return unauthorized();
        }

        // Reconstruct the request with the consumed body
        let req = Request::from_parts(parts, Body::from(body_bytes));
        return next.run(req).await;
    }

    next.run(req).await
}

/// Extract a Bearer token from the Authorization header.
fn extract_bearer(headers: &axum::http::HeaderMap) -> Option<String> {
    let auth = headers.get("Authorization")?.to_str().ok()?;
    auth.strip_prefix("Bearer ").map(|t| t.to_string())
}

/// Return a 401 response with body "Invalid".
fn unauthorized() -> Response {
    (StatusCode::UNAUTHORIZED, "Invalid").into_response()
}
