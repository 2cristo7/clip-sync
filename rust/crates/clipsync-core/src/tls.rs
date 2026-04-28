use std::fs;
use std::io::BufReader;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, SanType};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use thiserror::Error;

use crate::config::TLS_CERT_VALIDITY_DAYS;

#[derive(Debug, Error)]
pub enum TlsError {
    #[error("certificate generation failed: {0}")]
    CertGeneration(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TLS configuration error: {0}")]
    Config(String),
    #[error("certificate expired or invalid")]
    CertExpired,
    #[error("PEM parsing error: {0}")]
    PemParse(String),
}

/// Paths for persisted TLS identity.
pub struct TlsPaths {
    pub cert_der: PathBuf,
    pub key_pem: PathBuf,
}

impl TlsPaths {
    /// Default paths under ~/.clipsync/
    pub fn default_paths() -> Self {
        let base = dirs_base();
        Self {
            cert_der: base.join("cert.der"),
            key_pem: base.join("key.pem"),
        }
    }
}

fn dirs_base() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".clipsync")
}

/// A generated or loaded TLS identity (cert + key).
pub struct TlsIdentity {
    pub cert_der: Vec<u8>,
    pub key_pkcs8_der: Vec<u8>,
}

impl TlsIdentity {
    /// Generate a new self-signed EC P-256 certificate.
    ///
    /// SANs: localhost, hostname, *.local, provided IPs, 127.0.0.1
    pub fn generate(hostnames: &[String], ips: &[IpAddr]) -> Result<Self, TlsError> {
        let key_pair = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
            .map_err(|e| TlsError::CertGeneration(e.to_string()))?;

        let mut params = CertificateParams::default();
        params.is_ca = IsCa::NoCa;

        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "ClipSync");
        params.distinguished_name = dn;

        // Validity
        let now = time::OffsetDateTime::now_utc();
        params.not_before = now;
        params.not_after = now + time::Duration::days(TLS_CERT_VALIDITY_DAYS as i64);

        // Subject Alternative Names
        let mut sans = vec![
            SanType::DnsName("localhost".try_into().map_err(|e: rcgen::Error| TlsError::CertGeneration(e.to_string()))?),
            SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        ];

        for name in hostnames {
            if let Ok(san) = name.clone().try_into() {
                sans.push(SanType::DnsName(san));
            }
        }

        // Add *.local
        if let Ok(san) = "*.local".to_string().try_into() {
            sans.push(SanType::DnsName(san));
        }

        for ip in ips {
            sans.push(SanType::IpAddress(*ip));
        }

        params.subject_alt_names = sans;

        let cert = params
            .self_signed(&key_pair)
            .map_err(|e| TlsError::CertGeneration(e.to_string()))?;

        Ok(Self {
            cert_der: cert.der().to_vec(),
            key_pkcs8_der: key_pair.serialize_der(),
        })
    }

    /// Persist certificate and key to disk.
    pub fn save(&self, paths: &TlsPaths) -> Result<(), TlsError> {
        if let Some(parent) = paths.cert_der.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&paths.cert_der, &self.cert_der)?;

        // Write key as PEM
        let pem = pem::encode(&pem::Pem::new("PRIVATE KEY", self.key_pkcs8_der.clone()));
        fs::write(&paths.key_pem, pem.as_bytes())?;
        Ok(())
    }

    /// Load an existing identity from disk. Returns None if files don't exist.
    pub fn load(paths: &TlsPaths) -> Result<Option<Self>, TlsError> {
        if !paths.cert_der.exists() || !paths.key_pem.exists() {
            return Ok(None);
        }

        let cert_der = fs::read(&paths.cert_der)?;

        // Check expiry
        if is_cert_expired(&cert_der) {
            return Err(TlsError::CertExpired);
        }

        let key_pem_bytes = fs::read(&paths.key_pem)?;
        let mut reader = BufReader::new(key_pem_bytes.as_slice());
        let key_der = rustls_pemfile::private_key(&mut reader)
            .map_err(|e| TlsError::PemParse(e.to_string()))?
            .ok_or_else(|| TlsError::PemParse("no private key found in PEM".into()))?;

        let key_bytes = match key_der {
            PrivateKeyDer::Pkcs8(ref der) => der.secret_pkcs8_der().to_vec(),
            PrivateKeyDer::Pkcs1(ref der) => der.secret_pkcs1_der().to_vec(),
            PrivateKeyDer::Sec1(ref der) => der.secret_sec1_der().to_vec(),
            _ => return Err(TlsError::PemParse("unsupported key format".into())),
        };

        Ok(Some(Self {
            cert_der,
            key_pkcs8_der: key_bytes,
        }))
    }

    /// Load from disk if valid, otherwise generate and save.
    pub fn load_or_generate(
        paths: &TlsPaths,
        hostnames: &[String],
        ips: &[IpAddr],
    ) -> Result<Self, TlsError> {
        match Self::load(paths) {
            Ok(Some(identity)) => Ok(identity),
            Ok(None) | Err(TlsError::CertExpired) => {
                let identity = Self::generate(hostnames, ips)?;
                identity.save(paths)?;
                Ok(identity)
            }
            Err(e) => Err(e),
        }
    }

    /// Build a rustls ServerConfig from this identity.
    pub fn server_config(&self) -> Result<Arc<rustls::ServerConfig>, TlsError> {
        let cert = CertificateDer::from(self.cert_der.clone());
        let key = PrivatePkcs8KeyDer::from(self.key_pkcs8_der.clone());

        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert], PrivateKeyDer::Pkcs8(key))
            .map_err(|e| TlsError::Config(e.to_string()))?;

        Ok(Arc::new(config))
    }

    /// Build a rustls ClientConfig that trusts this specific certificate (TOFU).
    pub fn client_config(&self) -> Result<Arc<rustls::ClientConfig>, TlsError> {
        let cert = CertificateDer::from(self.cert_der.clone());

        let mut root_store = rustls::RootCertStore::empty();
        root_store
            .add(cert)
            .map_err(|e| TlsError::Config(e.to_string()))?;

        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        Ok(Arc::new(config))
    }
}

/// Check if a DER-encoded certificate is expired.
fn is_cert_expired(cert_der: &[u8]) -> bool {
    use x509_parser::prelude::*;
    match X509Certificate::from_der(cert_der) {
        Ok((_, cert)) => {
            let now = x509_parser::time::ASN1Time::now();
            cert.validity().not_after < now
        }
        Err(_) => true, // treat parse failure as expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn generate_self_signed_cert() {
        let identity = TlsIdentity::generate(
            &["myhost.local".to_string()],
            &[IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100))],
        )
        .unwrap();

        assert!(!identity.cert_der.is_empty());
        assert!(!identity.key_pkcs8_der.is_empty());
    }

    #[test]
    fn cert_is_not_expired() {
        let identity = TlsIdentity::generate(&[], &[]).unwrap();
        assert!(!is_cert_expired(&identity.cert_der));
    }

    #[test]
    fn server_config_builds() {
        let identity = TlsIdentity::generate(&[], &[]).unwrap();
        let config = identity.server_config();
        assert!(config.is_ok());
    }

    #[test]
    fn client_config_builds() {
        let identity = TlsIdentity::generate(&[], &[]).unwrap();
        let config = identity.client_config();
        assert!(config.is_ok());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("clipsync_tls_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let paths = TlsPaths {
            cert_der: dir.join("cert.der"),
            key_pem: dir.join("key.pem"),
        };

        let original = TlsIdentity::generate(&[], &[]).unwrap();
        original.save(&paths).unwrap();

        let loaded = TlsIdentity::load(&paths).unwrap().unwrap();
        assert_eq!(original.cert_der, loaded.cert_der);

        let _ = fs::remove_dir_all(&dir);
    }
}
