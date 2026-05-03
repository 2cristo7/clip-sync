use data_encoding::BASE64URL_NOPAD;
use sha2::{Digest, Sha256};
use x509_parser::prelude::*;

/// Compute the SHA-256 fingerprint of the Subject Public Key Info (SPKI)
/// from a DER-encoded X.509 certificate.
///
/// Returns base64url encoding WITHOUT padding, as per ClipSync protocol.
pub fn spki_sha256(cert_der: &[u8]) -> Result<String, String> {
    let (_, cert) = X509Certificate::from_der(cert_der)
        .map_err(|e| format!("failed to parse certificate: {}", e))?;

    let spki_der = cert.tbs_certificate.subject_pki.raw;

    let hash = Sha256::digest(spki_der);
    Ok(BASE64URL_NOPAD.encode(&hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tls::TlsIdentity;

    #[test]
    fn fingerprint_format() {
        let identity = TlsIdentity::generate(&[], &[]).unwrap();
        let fp = spki_sha256(&identity.cert_der).unwrap();

        // base64url no padding: [A-Za-z0-9_-]+, no = at end
        assert!(!fp.is_empty());
        assert!(!fp.contains('='));
        assert!(!fp.contains('+'));
        assert!(!fp.contains('/'));
        assert!(fp
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'));

        // SHA-256 = 32 bytes → 43 base64url chars (no padding)
        assert_eq!(fp.len(), 43);
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let identity = TlsIdentity::generate(&[], &[]).unwrap();
        let fp1 = spki_sha256(&identity.cert_der).unwrap();
        let fp2 = spki_sha256(&identity.cert_der).unwrap();
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn different_certs_different_fingerprints() {
        let id1 = TlsIdentity::generate(&[], &[]).unwrap();
        let id2 = TlsIdentity::generate(&[], &[]).unwrap();
        let fp1 = spki_sha256(&id1.cert_der).unwrap();
        let fp2 = spki_sha256(&id2.cert_der).unwrap();
        assert_ne!(fp1, fp2);
    }
}
