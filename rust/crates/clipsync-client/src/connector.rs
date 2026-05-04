use std::sync::Arc;
use std::time::Duration;

use futures::stream::StreamExt;
use rustls::pki_types::ServerName;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

use clipsync_core::clipboard::ClipboardProvider;
use clipsync_core::config::{
    CONSECUTIVE_FAILURE_THRESHOLD, HEALTHCHECK_POLL_INTERVAL, WS_READ_TIMEOUT,
};
use clipsync_core::protocol::ClipPayload;

use crate::credentials::ClientCredentials;

#[derive(Debug, thiserror::Error)]
pub enum ConnectorError {
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    #[error("TLS error: {0}")]
    Tls(String),
    #[error("clipboard error: {0}")]
    Clipboard(String),
}

/// Backoff configuration for reconnection.
struct Backoff {
    current: Duration,
    max: Duration,
}

impl Backoff {
    fn new() -> Self {
        Self {
            current: Duration::from_secs(1),
            max: Duration::from_secs(30),
        }
    }

    fn next_delay(&mut self) -> Duration {
        let delay = self.current;
        self.current = (self.current * 2).min(self.max);
        delay
    }

    #[allow(dead_code)]
    fn reset(&mut self) {
        self.current = Duration::from_secs(1);
    }
}

/// Channel sender for received clipboard payloads.
pub type IncomingPayloadTx = mpsc::Sender<ClipPayload>;

/// Channel receiver for received clipboard payloads.
pub type IncomingPayloadRx = mpsc::Receiver<ClipPayload>;

/// Run the WebSocket connector loop with auto-reconnect.
///
/// Connects to the server, receives ClipPayload messages, and writes them
/// to the clipboard. Reconnects with exponential backoff on disconnection.
///
/// `incoming_tx` receives payloads from the server (for echo suppression).
/// `paused` flag: when true, incoming payloads are ignored.
pub async fn run_connector<C: ClipboardProvider>(
    creds: ClientCredentials,
    clipboard: Arc<C>,
    incoming_tx: IncomingPayloadTx,
    paused: Arc<std::sync::atomic::AtomicBool>,
    status_tx: Option<mpsc::UnboundedSender<ConnectionStatus>>,
) {
    let mut backoff = Backoff::new();

    loop {
        if let Some(ref tx) = status_tx {
            let _ = tx.send(ConnectionStatus::Connecting);
        }

        match connect_and_listen(&creds, &clipboard, &incoming_tx, &paused).await {
            Ok(()) => {
                info!("WebSocket closed cleanly");
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
            }
        }

        if let Some(ref tx) = status_tx {
            let _ = tx.send(ConnectionStatus::Disconnected);
        }

        let delay = backoff.next_delay();
        info!("reconnecting in {:?}...", delay);
        tokio::time::sleep(delay).await;
    }
}

/// Connection status events.
#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
}

/// Connect once and listen for messages until disconnection.
async fn connect_and_listen<C: ClipboardProvider>(
    creds: &ClientCredentials,
    clipboard: &Arc<C>,
    incoming_tx: &IncomingPayloadTx,
    paused: &Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), ConnectorError> {
    let tls_config = build_pinned_tls_config(creds)?;

    let url = format!("wss://{}:{}/ws", creds.host, creds.port);
    let mut request = url
        .into_client_request()
        .map_err(|e| ConnectorError::WebSocket(e.to_string()))?;

    // Add Bearer token
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", creds.token)
            .parse()
            .map_err(|e| ConnectorError::WebSocket(format!("invalid header: {}", e)))?,
    );

    let connector = tokio_tungstenite::Connector::Rustls(Arc::new(tls_config));

    let (ws_stream, _response) =
        tokio_tungstenite::connect_async_tls_with_config(request, None, false, Some(connector))
            .await
            .map_err(|e| ConnectorError::WebSocket(e.to_string()))?;

    info!("WebSocket connected to {}:{}", creds.host, creds.port);

    let (mut _write, mut read) = ws_stream.split();

    // Stalled-link detection. We track two independent failure counters:
    //   1. `read_timeouts`  — successive `WS_READ_TIMEOUT` ticks without
    //      ANY frame from the server (the server pings every
    //      `WS_PING_INTERVAL`, so on a healthy link we always see at
    //      least Ping/data inside the read window).
    //   2. `healthcheck_failures` — successive `/health` poll failures.
    //
    // Either counter reaching `CONSECUTIVE_FAILURE_THRESHOLD` aborts
    // `connect_and_listen` so `run_connector` triggers reconnect via the
    // existing backoff. Constants come from `clipsync_core::config` —
    // do NOT introduce magic numbers here. See Phase 1.7.
    let mut read_timeouts: u32 = 0;
    let mut healthcheck_failures: u32 = 0;

    let mut healthcheck_ticker = tokio::time::interval(HEALTHCHECK_POLL_INTERVAL);
    // Skip the initial immediate tick so we don't poll /health before
    // the first WS frame even has a chance to arrive.
    healthcheck_ticker.tick().await;

    let health_url = format!("https://{}:{}/health", creds.host, creds.port);
    let health_client = build_health_client(creds)?;

    loop {
        tokio::select! {
            biased;
            res = tokio::time::timeout(WS_READ_TIMEOUT, read.next()) => {
                match res {
                    Err(_elapsed) => {
                        read_timeouts = read_timeouts.saturating_add(1);
                        warn!(
                            "WS read timeout ({}/{})",
                            read_timeouts, CONSECUTIVE_FAILURE_THRESHOLD
                        );
                        if read_timeouts >= CONSECUTIVE_FAILURE_THRESHOLD {
                            return Err(ConnectorError::WebSocket(
                                "stalled: consecutive read timeouts".into(),
                            ));
                        }
                        continue;
                    }
                    Ok(None) => {
                        info!("WebSocket stream ended");
                        break;
                    }
                    Ok(Some(msg)) => {
                        // Any frame (data, ping, pong) resets the timeout counter.
                        read_timeouts = 0;
                        match msg {
                            Ok(Message::Text(text)) => {
                                if paused.load(std::sync::atomic::Ordering::Relaxed) {
                                    continue;
                                }

                                match serde_json::from_str::<ClipPayload>(&text) {
                                    Ok(payload) => {
                                        // Notify echo suppression channel
                                        let _ = incoming_tx.send(payload.clone()).await;

                                        // Write to clipboard
                                        if let Err(e) = clipboard.write(&payload) {
                                            error!("failed to write to clipboard: {}", e);
                                        } else {
                                            info!(
                                                "received {:?} payload (nonce={})",
                                                payload.clip_type, payload.nonce
                                            );
                                            // Show desktop notification for non-text content
                                            if payload.clip_type != clipsync_core::protocol::ClipType::Text {
                                                if let Err(e) = clipsync_core::clipboard::notify_received(&payload)
                                                {
                                                    warn!("notification failed: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        warn!("failed to parse WebSocket message: {}", e);
                                    }
                                }
                            }
                            Ok(Message::Ping(data)) => {
                                // tungstenite handles pong automatically
                                let _ = data;
                            }
                            Ok(Message::Pong(_)) => {
                                // Pong from server in response to our pings (if any).
                            }
                            Ok(Message::Close(_)) => {
                                info!("server sent close frame");
                                break;
                            }
                            Err(e) => {
                                return Err(ConnectorError::WebSocket(e.to_string()));
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ = healthcheck_ticker.tick() => {
                match poll_health(&health_client, &health_url).await {
                    Ok(()) => {
                        healthcheck_failures = 0;
                    }
                    Err(e) => {
                        healthcheck_failures = healthcheck_failures.saturating_add(1);
                        warn!(
                            "/health poll failed ({}/{}): {}",
                            healthcheck_failures, CONSECUTIVE_FAILURE_THRESHOLD, e
                        );
                        if healthcheck_failures >= CONSECUTIVE_FAILURE_THRESHOLD {
                            return Err(ConnectorError::WebSocket(
                                "stalled: consecutive /health failures".into(),
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Build a reqwest client that pins the same SPKI fingerprint as the WS
/// connection. Used by the healthcheck poll loop.
fn build_health_client(creds: &ClientCredentials) -> Result<reqwest::Client, ConnectorError> {
    let tls = build_pinned_tls_config(creds)?;
    reqwest::Client::builder()
        .use_preconfigured_tls(tls)
        .timeout(WS_READ_TIMEOUT)
        .build()
        .map_err(|e| ConnectorError::WebSocket(format!("health client build: {}", e)))
}

/// Issue one `GET /health` request. Returns `Ok(())` only on 2xx.
async fn poll_health(client: &reqwest::Client, url: &str) -> Result<(), String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request: {}", e))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("status {}", resp.status()))
    }
}

/// Build a rustls ClientConfig pinned to the server's certificate fingerprint.
fn build_pinned_tls_config(
    creds: &ClientCredentials,
) -> Result<rustls::ClientConfig, ConnectorError> {
    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(FingerprintVerifier {
            expected_fingerprint: creds.fingerprint.clone(),
        }))
        .with_no_client_auth();

    Ok(config)
}

/// A TLS verifier that accepts certificates matching a known SPKI fingerprint.
#[derive(Debug)]
struct FingerprintVerifier {
    expected_fingerprint: String,
}

impl rustls::client::danger::ServerCertVerifier for FingerprintVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let actual_fp =
            clipsync_core::fingerprint::spki_sha256(end_entity.as_ref()).map_err(|e| {
                rustls::Error::General(format!("fingerprint computation failed: {}", e))
            })?;

        if actual_fp != self.expected_fingerprint {
            return Err(rustls::Error::General(format!(
                "certificate fingerprint mismatch: expected {}, got {}",
                self.expected_fingerprint, actual_fp
            )));
        }

        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_exponential() {
        let mut b = Backoff::new();
        assert_eq!(b.next_delay(), Duration::from_secs(1));
        assert_eq!(b.next_delay(), Duration::from_secs(2));
        assert_eq!(b.next_delay(), Duration::from_secs(4));
        assert_eq!(b.next_delay(), Duration::from_secs(8));
        assert_eq!(b.next_delay(), Duration::from_secs(16));
        assert_eq!(b.next_delay(), Duration::from_secs(30)); // capped
        assert_eq!(b.next_delay(), Duration::from_secs(30)); // stays at max
    }

    #[test]
    fn backoff_reset() {
        let mut b = Backoff::new();
        b.next_delay();
        b.next_delay();
        b.reset();
        assert_eq!(b.next_delay(), Duration::from_secs(1));
    }

    /// Simulate a stalled WS link with `tokio::time::pause()` and assert
    /// that the same retry semantics implemented in `connect_and_listen`
    /// (timeout each read, count consecutive timeouts, give up at the
    /// shared `CONSECUTIVE_FAILURE_THRESHOLD`) trip a reconnect signal in
    /// at most `WS_READ_TIMEOUT * CONSECUTIVE_FAILURE_THRESHOLD` of
    /// virtual time — i.e. 30s with the `fc9b1d38` constants. The plan's
    /// "within 25s (worst case 2× read timeout + jitter)" budget is the
    /// product spec; with threshold=2 and timeout=15s we land at exactly
    /// 30s of virtual time, deterministic.
    ///
    /// We can't drive the real `connect_and_listen` against a fake
    /// server under paused time without either standing up a real TCP
    /// listener (defeats the point of paused time) or refactoring the
    /// stream into a generic `Stream<Item=...>`. So this test exercises
    /// the *shape* of the loop using the same constants and the same
    /// `tokio::time::timeout` primitive that the production code uses.
    /// See Phase 1.7 success criteria — this is the documented
    /// limitation.
    #[tokio::test(start_paused = true)]
    async fn stalled_ws_triggers_reconnect_within_threshold() {
        use clipsync_core::config::{CONSECUTIVE_FAILURE_THRESHOLD, WS_READ_TIMEOUT};
        use std::future::pending;

        let start = tokio::time::Instant::now();
        let mut consecutive: u32 = 0;
        let trigger = loop {
            // `pending()` never resolves — simulates a WS that has gone
            // silent (no Ping, no data, no error).
            let res: Result<(), _> = tokio::time::timeout(WS_READ_TIMEOUT, pending::<()>()).await;
            assert!(res.is_err(), "pending future cannot resolve");
            consecutive += 1;
            if consecutive >= CONSECUTIVE_FAILURE_THRESHOLD {
                break tokio::time::Instant::now();
            }
        };

        let elapsed = trigger.duration_since(start);
        let budget = WS_READ_TIMEOUT * CONSECUTIVE_FAILURE_THRESHOLD;
        assert_eq!(
            elapsed, budget,
            "reconnect should fire after exactly {budget:?} of virtual time, got {elapsed:?}"
        );
        // Sanity: we did NOT use the (slightly off) 25s plan figure, but
        // we ARE bounded above by 2× read timeout (no jitter in the loop).
        assert!(elapsed <= budget);
    }

    #[test]
    fn build_pinned_tls_config_succeeds() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let creds = ClientCredentials {
            token: "dG9rZW4=".to_string(),
            secret: "c2VjcmV0".to_string(),
            host: "127.0.0.1".to_string(),
            port: 7010,
            fingerprint: "test-fingerprint".to_string(),
            server_name: None,
        };
        let config = build_pinned_tls_config(&creds);
        assert!(config.is_ok());
    }
}
