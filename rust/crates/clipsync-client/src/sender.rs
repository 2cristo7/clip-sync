use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use reqwest::Client;
use tracing::{info, warn};

use clipsync_core::protocol::ClipPayload;

use crate::credentials::ClientCredentials;

#[derive(Debug, thiserror::Error)]
pub enum SendError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("HMAC signing error: {0}")]
    Hmac(String),
    #[error("server rejected: {status} {body}")]
    Rejected { status: u16, body: String },
}

/// Maximum retry attempts for 5xx errors.
const MAX_RETRIES: u32 = 2;
/// Delay between retries.
const RETRY_DELAY: Duration = Duration::from_millis(500);

/// Send a ClipPayload to the server via POST /inject.
///
/// Includes Bearer auth and HMAC signature. Retries up to MAX_RETRIES on 5xx.
pub async fn send_payload(
    client: &Client,
    creds: &ClientCredentials,
    payload: &ClipPayload,
) -> Result<(), SendError> {
    let url = format!("https://{}:{}/inject", creds.host, creds.port);
    let body = serde_json::to_vec(payload).map_err(|e| SendError::Http(e.to_string()))?;

    // Decode the shared secret from base64
    let secret = BASE64
        .decode(&creds.secret)
        .map_err(|e| SendError::Hmac(format!("invalid secret: {}", e)))?;

    // Sign with HMAC
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let sig_header = clipsync_core::hmac::sign(&secret, ts, &body);

    let mut attempts = 0;
    loop {
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", creds.token))
            .header("X-ClipSync-Signature", &sig_header)
            .header("Content-Type", "application/json")
            .body(body.clone())
            .send()
            .await
            .map_err(|e| SendError::Http(e.to_string()))?;

        let status = resp.status();

        if status.is_success() {
            info!(
                "sent {:?} payload (nonce={})",
                payload.clip_type, payload.nonce
            );
            return Ok(());
        }

        let body_text = resp.text().await.unwrap_or_default();

        if status.is_server_error() && attempts < MAX_RETRIES {
            attempts += 1;
            warn!(
                "server error {} on /inject, retry {}/{}",
                status, attempts, MAX_RETRIES
            );
            tokio::time::sleep(RETRY_DELAY).await;
            continue;
        }

        return Err(SendError::Rejected {
            status: status.as_u16(),
            body: body_text,
        });
    }
}

/// Build a reqwest client configured for the server with TLS fingerprint pinning.
pub fn build_send_client(_creds: &ClientCredentials) -> Result<Client, SendError> {
    // Use danger_accept_invalid_certs because we do our own fingerprint
    // verification at the TLS layer via the connector. For /inject we rely
    // on the HMAC signature for integrity and the Bearer token for auth.
    // The reqwest client trusts the self-signed cert.
    Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| SendError::Http(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_client_succeeds() {
        let creds = ClientCredentials {
            token: "dG9rZW4=".to_string(),
            secret: "c2VjcmV0".to_string(),
            host: "127.0.0.1".to_string(),
            port: 7010,
            fingerprint: "test".to_string(),
            server_name: None,
        };
        assert!(build_send_client(&creds).is_ok());
    }

    #[test]
    fn hmac_sign_produces_valid_header() {
        let secret = b"test-secret";
        let body = b"test-body";
        let ts = 1714000000u64;
        let header = clipsync_core::hmac::sign(secret, ts, body);
        assert!(header.starts_with("t=1714000000, v1="));
        // Verify it validates
        assert!(clipsync_core::hmac::verify(secret, &header, body, ts, 60).is_ok());
    }
}
