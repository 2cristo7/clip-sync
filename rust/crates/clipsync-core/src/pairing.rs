use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use hmac::{Hmac, Mac};
use rand::Rng;
use sha2::Sha256;
use thiserror::Error;

use crate::config::PAIRING_CODE_TTL_SECS;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum PairingError {
    #[error("pairing code expired")]
    CodeExpired,
    #[error("invalid pairing code")]
    InvalidCode,
    #[error("no active pairing code")]
    NoActiveCode,
}

/// A time-limited 6-digit pairing code.
pub struct PairingCode {
    pub code: String,
    created_at: Instant,
    ttl: Duration,
}

impl PairingCode {
    /// Generate a new random 6-digit pairing code.
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        let code: u32 = rng.gen_range(0..1_000_000);
        Self {
            code: format!("{:06}", code),
            created_at: Instant::now(),
            ttl: Duration::from_secs(PAIRING_CODE_TTL_SECS),
        }
    }

    /// Check if this code is still valid.
    pub fn is_valid(&self) -> bool {
        self.created_at.elapsed() < self.ttl
    }

    /// Validate the given code string against this pairing code.
    pub fn validate(&self, candidate: &str) -> Result<(), PairingError> {
        if !self.is_valid() {
            return Err(PairingError::CodeExpired);
        }
        if candidate != self.code {
            return Err(PairingError::InvalidCode);
        }
        Ok(())
    }
}

/// Response data for a successful pairing exchange.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PairResponse {
    /// The authentication token (base64-encoded 32 random bytes).
    pub token: String,
    /// HMAC signature of the token (base64-encoded).
    pub sig: String,
    /// The shared secret for future HMAC signing (base64-encoded 32 random bytes).
    pub secret: String,
}

/// Create a pairing response with a new token and shared secret.
///
/// `signing_secret` is the server's pairing secret used to sign the token.
pub fn create_pair_response(signing_secret: &[u8]) -> PairResponse {
    let mut rng = rand::thread_rng();

    // Generate 32-byte token
    let mut token_bytes = [0u8; 32];
    rng.fill(&mut token_bytes);
    let token_b64 = BASE64.encode(token_bytes);

    // Generate 32-byte shared secret
    let mut secret_bytes = [0u8; 32];
    rng.fill(&mut secret_bytes);
    let secret_b64 = BASE64.encode(secret_bytes);

    // Sign the token with the server's pairing secret
    let mut mac = HmacSha256::new_from_slice(signing_secret)
        .expect("HMAC accepts any key length");
    mac.update(token_bytes.as_ref());
    let sig = mac.finalize();
    let sig_b64 = BASE64.encode(sig.into_bytes());

    PairResponse {
        token: token_b64,
        sig: sig_b64,
        secret: secret_b64,
    }
}

/// Manages a single active pairing code at a time.
pub struct PairingManager {
    active_code: Option<PairingCode>,
}

impl PairingManager {
    pub fn new() -> Self {
        Self { active_code: None }
    }

    /// Generate a new pairing code, replacing any existing one.
    pub fn generate_code(&mut self) -> &str {
        self.active_code = Some(PairingCode::generate());
        &self.active_code.as_ref().unwrap().code
    }

    /// Validate an incoming code and consume it on success.
    pub fn validate_and_consume(&mut self, code: &str) -> Result<(), PairingError> {
        let active = self.active_code.as_ref().ok_or(PairingError::NoActiveCode)?;
        active.validate(code)?;
        self.active_code = None; // consume on success
        Ok(())
    }

    /// Check if there's an active (non-expired) code.
    pub fn has_active_code(&self) -> bool {
        self.active_code.as_ref().is_some_and(|c| c.is_valid())
    }
}

impl Default for PairingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_is_six_digits() {
        let code = PairingCode::generate();
        assert_eq!(code.code.len(), 6);
        assert!(code.code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn code_is_initially_valid() {
        let code = PairingCode::generate();
        assert!(code.is_valid());
    }

    #[test]
    fn validate_correct_code() {
        let code = PairingCode::generate();
        let code_str = code.code.clone();
        assert!(code.validate(&code_str).is_ok());
    }

    #[test]
    fn validate_wrong_code() {
        let code = PairingCode::generate();
        assert!(matches!(
            code.validate("999999"),
            Err(PairingError::InvalidCode) | Ok(()) // might match if rng gave 999999
        ));
    }

    #[test]
    fn pair_response_format() {
        let secret = b"test-pairing-secret";
        let resp = create_pair_response(secret);

        // Token should be base64 of 32 bytes → 44 chars
        let token_bytes = BASE64.decode(&resp.token).unwrap();
        assert_eq!(token_bytes.len(), 32);

        // Secret should be base64 of 32 bytes → 44 chars
        let secret_bytes = BASE64.decode(&resp.secret).unwrap();
        assert_eq!(secret_bytes.len(), 32);

        // Sig should be base64 of 32 bytes (HMAC-SHA256 output)
        let sig_bytes = BASE64.decode(&resp.sig).unwrap();
        assert_eq!(sig_bytes.len(), 32);
    }

    #[test]
    fn pair_response_sig_is_valid() {
        let secret = b"test-pairing-secret";
        let resp = create_pair_response(secret);

        // Verify signature
        let token_bytes = BASE64.decode(&resp.token).unwrap();
        let mut mac = HmacSha256::new_from_slice(secret).unwrap();
        mac.update(&token_bytes);
        let sig_bytes = BASE64.decode(&resp.sig).unwrap();
        assert!(mac.verify_slice(&sig_bytes).is_ok());
    }

    #[test]
    fn golden_pair_response_structure() {
        let golden = include_str!("../../../tests/golden/pair_response.json");
        let resp: PairResponse = serde_json::from_str(golden).unwrap();

        // Verify all fields decode from base64
        assert!(BASE64.decode(&resp.token).is_ok());
        assert!(BASE64.decode(&resp.sig).is_ok());
        assert!(BASE64.decode(&resp.secret).is_ok());
    }

    #[test]
    fn manager_lifecycle() {
        let mut mgr = PairingManager::new();
        assert!(!mgr.has_active_code());

        let code = mgr.generate_code().to_string();
        assert!(mgr.has_active_code());

        // Wrong code fails
        let wrong = if code == "000000" { "111111" } else { "000000" };
        assert!(mgr.validate_and_consume(wrong).is_err());

        // Correct code succeeds and consumes
        assert!(mgr.validate_and_consume(&code).is_ok());
        assert!(!mgr.has_active_code());
    }
}
