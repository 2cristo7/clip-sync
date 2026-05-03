use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use reqwest::Client;
use rustls::pki_types::ServerName;
use tracing::info;

use clipsync_core::config::PORT;
use clipsync_core::fingerprint::spki_sha256;
use clipsync_core::mdns::{discover, DiscoveredServer};
use clipsync_core::pairing::PairResponse;

use crate::credentials::ClientCredentials;

#[derive(Debug, thiserror::Error)]
pub enum PairingError {
    #[error("no servers found via mDNS")]
    NoServersFound,
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("invalid server response: {0}")]
    InvalidResponse(String),
    #[error("TLS error: {0}")]
    Tls(String),
    #[error("user cancelled")]
    Cancelled,
    #[error("invalid server address: {0}")]
    InvalidAddress(String),
}

/// Run the full pairing flow.
///
/// - If `server_addr` is Some, use manual mode (direct connection).
/// - Otherwise, use mDNS to discover servers.
/// - `code` is the 6-digit pairing code (prompted if None).
/// - `device_label` is sent as X-ClipSync-Device header.
pub async fn pair(
    server_addr: Option<&str>,
    code: Option<&str>,
    device_label: &str,
) -> Result<ClientCredentials, PairingError> {
    let (host, port, server_name, fingerprint_hint) = if let Some(addr) = server_addr {
        let (h, p) = parse_server_addr(addr)?;
        (h, p, None, None)
    } else {
        let server = discover_server().await?;
        let addr = server
            .addresses
            .first()
            .map(|a| a.to_string())
            .unwrap_or_else(|| server.host.trim_end_matches('.').to_string());
        (
            addr,
            server.port,
            Some(server.name.clone()),
            Some(server.fingerprint.clone()),
        )
    };

    info!("connecting to {}:{}", host, port);

    // Build a TLS client that accepts any cert (TOFU: we'll pin after seeing it)
    let client = build_tofu_client()?;

    let code = match code {
        Some(c) => c.to_string(),
        None => prompt_code()?,
    };

    // Perform the pairing request
    let url = format!("https://{}:{}/pair?code={}", host, port, code);
    info!("pairing: GET {}", url);

    let resp = client
        .get(&url)
        .header("X-ClipSync-Device", device_label)
        .send()
        .await
        .map_err(|e| PairingError::Http(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(PairingError::Http(format!(
            "server returned {}: {}",
            status, body
        )));
    }

    let pair_resp: PairResponse = resp
        .json()
        .await
        .map_err(|e| PairingError::InvalidResponse(e.to_string()))?;

    // Verify we got valid base64 tokens
    BASE64
        .decode(&pair_resp.token)
        .map_err(|e| PairingError::InvalidResponse(format!("invalid token: {}", e)))?;
    BASE64
        .decode(&pair_resp.secret)
        .map_err(|e| PairingError::InvalidResponse(format!("invalid secret: {}", e)))?;

    // Get the server's TLS certificate fingerprint by connecting again
    let fingerprint = match fingerprint_hint {
        Some(fp) if !fp.is_empty() => fp,
        _ => fetch_server_fingerprint(&host, port).await?,
    };

    info!("pairing successful, fingerprint: {}", fingerprint);

    Ok(ClientCredentials {
        token: pair_resp.token,
        secret: pair_resp.secret,
        host: host.to_string(),
        port,
        fingerprint,
        server_name,
    })
}

/// Discover a ClipSync server via mDNS.
async fn discover_server() -> Result<DiscoveredServer, PairingError> {
    info!("searching for ClipSync servers via mDNS...");

    // Run mDNS discovery in a blocking thread (it uses synchronous recv)
    let servers = tokio::task::spawn_blocking(|| discover(Duration::from_secs(5)))
        .await
        .map_err(|e| PairingError::Http(format!("discovery task failed: {}", e)))?
        .map_err(|e| PairingError::Http(e.to_string()))?;

    if servers.is_empty() {
        return Err(PairingError::NoServersFound);
    }

    if servers.len() == 1 {
        let s = &servers[0];
        info!("found server: {} ({}:{})", s.name, s.host, s.port);
        return Ok(servers.into_iter().next().unwrap());
    }

    // Multiple servers: let user choose
    println!("Found {} ClipSync servers:", servers.len());
    for (i, s) in servers.iter().enumerate() {
        let addr = s
            .addresses
            .first()
            .map(|a| a.to_string())
            .unwrap_or_else(|| s.host.clone());
        println!("  [{}] {} ({}:{})", i + 1, s.name, addr, s.port);
    }

    print!("Select server [1]: ");
    io::stdout()
        .flush()
        .map_err(|e| PairingError::Http(e.to_string()))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| PairingError::Http(e.to_string()))?;

    let choice: usize = input.trim().parse().unwrap_or(1);
    if choice < 1 || choice > servers.len() {
        return Err(PairingError::Cancelled);
    }

    Ok(servers.into_iter().nth(choice - 1).unwrap())
}

/// Parse "host:port" or just "host" (defaults to PORT).
fn parse_server_addr(addr: &str) -> Result<(String, u16), PairingError> {
    if let Some((host, port_str)) = addr.rsplit_once(':') {
        let port: u16 = port_str
            .parse()
            .map_err(|_| PairingError::InvalidAddress(format!("invalid port: {}", port_str)))?;
        Ok((host.to_string(), port))
    } else {
        Ok((addr.to_string(), PORT))
    }
}

/// Prompt the user for a 6-digit pairing code.
fn prompt_code() -> Result<String, PairingError> {
    print!("Enter 6-digit pairing code: ");
    io::stdout().flush().map_err(|_| PairingError::Cancelled)?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|_| PairingError::Cancelled)?;

    let code = input.trim().to_string();
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return Err(PairingError::InvalidAddress(
            "code must be exactly 6 digits".to_string(),
        ));
    }

    Ok(code)
}

/// Build a reqwest client that accepts any TLS certificate (for TOFU pairing).
fn build_tofu_client() -> Result<Client, PairingError> {
    Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| PairingError::Tls(e.to_string()))
}

/// Connect to the server and extract its TLS certificate fingerprint.
async fn fetch_server_fingerprint(host: &str, port: u16) -> Result<String, PairingError> {
    use tokio::net::TcpStream;
    use tokio_rustls::TlsConnector;

    // Build a rustls config that accepts any cert (we just want to see it)
    let mut config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCertVerifier))
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];

    let connector = TlsConnector::from(Arc::new(config));
    let server_name = ServerName::try_from(host.to_string())
        .unwrap_or_else(|_| ServerName::try_from("localhost".to_string()).unwrap());

    let stream = TcpStream::connect(format!("{}:{}", host, port))
        .await
        .map_err(|e| PairingError::Tls(format!("connect failed: {}", e)))?;

    let tls_stream = connector
        .connect(server_name, stream)
        .await
        .map_err(|e| PairingError::Tls(format!("TLS handshake failed: {}", e)))?;

    let (_, conn) = tls_stream.get_ref();
    let certs = conn
        .peer_certificates()
        .ok_or_else(|| PairingError::Tls("no peer certificate".to_string()))?;

    let cert_der = certs
        .first()
        .ok_or_else(|| PairingError::Tls("empty certificate chain".to_string()))?;

    spki_sha256(cert_der.as_ref())
        .map_err(|e| PairingError::Tls(format!("fingerprint failed: {}", e)))
}

/// A TLS certificate verifier that accepts any certificate (for TOFU).
#[derive(Debug)]
struct AcceptAnyCertVerifier;

impl rustls::client::danger::ServerCertVerifier for AcceptAnyCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
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
    fn parse_host_and_port() {
        let (host, port) = parse_server_addr("192.168.1.100:7010").unwrap();
        assert_eq!(host, "192.168.1.100");
        assert_eq!(port, 7010);
    }

    #[test]
    fn parse_host_only() {
        let (host, port) = parse_server_addr("192.168.1.100").unwrap();
        assert_eq!(host, "192.168.1.100");
        assert_eq!(port, PORT);
    }

    #[test]
    fn parse_invalid_port() {
        let result = parse_server_addr("192.168.1.100:abc");
        assert!(result.is_err());
    }
}
