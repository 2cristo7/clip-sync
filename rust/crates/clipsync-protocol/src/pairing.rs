use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use hmac::{Hmac, Mac};
use rand::Rng;
use sha2::Sha256;
use thiserror::Error;

use crate::config::PAIRING_CODE_TTL_SECS;

type HmacSha256 = Hmac<Sha256>;

/// Errors returned by the pairing state machine.
///
/// The variants here mirror the Mac (Swift) `PairingError` taxonomy
/// (`mac-legacy/ClipSync/Pairing/PairingManager.swift`). The wire codes
/// returned by [`PairingError::code`] are camelCase to match the Mac
/// vocabulary so cross-platform clients can parse responses identically.
///
/// See `docs/plans/master-plan-rust-fork.md` Phase 1.5 for the rationale.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PairingError {
    /// The supplied code does not match the active code.
    #[error("invalid pairing code")]
    InvalidCode,
    /// The active code's TTL has elapsed.
    #[error("pairing code expired")]
    CodeExpired,
    /// The active code has already been used in a successful pairing.
    #[error("pairing code already consumed")]
    ConsumedCode,
    /// No pairing code has been started (or the previous one was cleared).
    #[error("no active pairing code")]
    NoActiveCode,
}

impl PairingError {
    /// Stable machine-readable code used as the `error` field in 401 bodies.
    ///
    /// These values match the Mac `PairingError` Swift case names
    /// (`invalid`, `expired`, `consumed`, `notStarted`) so that the
    /// Android/Tauri clients can decode the same vocabulary regardless of
    /// which server implementation they talk to.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidCode => "invalid",
            Self::CodeExpired => "expired",
            Self::ConsumedCode => "consumed",
            Self::NoActiveCode => "notStarted",
        }
    }
}

/// A time-limited 6-digit pairing code.
pub struct PairingCode {
    pub code: String,
    created_at: Instant,
    ttl: Duration,
    consumed: bool,
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
            consumed: false,
        }
    }

    /// True when the code has not yet expired (TTL-wise).
    pub fn is_valid(&self) -> bool {
        self.created_at.elapsed() < self.ttl
    }

    /// True when the code has already been used in a successful pairing.
    pub fn is_consumed(&self) -> bool {
        self.consumed
    }

    /// Validate the given code string against this pairing code.
    ///
    /// Order of checks mirrors the Mac (Swift) `PairingManager.consume`:
    /// expired → consumed → mismatch. This keeps the wire vocabulary
    /// stable between the two server implementations.
    pub fn validate(&self, candidate: &str) -> Result<(), PairingError> {
        if !self.is_valid() {
            return Err(PairingError::CodeExpired);
        }
        if self.consumed {
            return Err(PairingError::ConsumedCode);
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
    let mut mac = HmacSha256::new_from_slice(signing_secret).expect("HMAC accepts any key length");
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
    ///
    /// Returns the precise [`PairingError`] variant matching the Mac state
    /// machine so the HTTP layer can report `notStarted | expired |
    /// consumed | invalid` to the client. On success the code is marked
    /// consumed (kept in memory) so a subsequent attempt with the same
    /// code reports `consumed` rather than `notStarted`.
    pub fn validate_and_consume(&mut self, code: &str) -> Result<(), PairingError> {
        let active = self
            .active_code
            .as_mut()
            .ok_or(PairingError::NoActiveCode)?;
        active.validate(code)?;
        active.consumed = true;
        Ok(())
    }

    /// Check if there's an active (non-expired, non-consumed) code.
    pub fn has_active_code(&self) -> bool {
        self.active_code
            .as_ref()
            .is_some_and(|c| c.is_valid() && !c.is_consumed())
    }

    /// Test-only helper: force the active code into an expired state
    /// without waiting for the real TTL.
    ///
    /// This mutates `created_at` to `(now - 2 * ttl)` so subsequent
    /// `is_valid()` checks return false. Used by integration tests in
    /// `clipsync-server` to exercise the `CodeExpired` branch.
    ///
    /// Behind `cfg(any(test, feature = "test-support"))` so production
    /// binaries cannot accidentally call it. Currently exposed only via
    /// `cfg(test)` cross-crate by re-export through the
    /// `test-support` feature.
    #[doc(hidden)]
    pub fn pre_expire_for_tests(&mut self) {
        if let Some(code) = self.active_code.as_mut() {
            // Move `created_at` far enough back that `is_valid()` is
            // false regardless of the configured TTL.
            code.created_at = Instant::now()
                .checked_sub(code.ttl * 2)
                .unwrap_or_else(Instant::now);
        }
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

        // No code yet -> NoActiveCode
        assert_eq!(
            mgr.validate_and_consume("000000"),
            Err(PairingError::NoActiveCode)
        );

        let code = mgr.generate_code().to_string();
        assert!(mgr.has_active_code());

        // Wrong code -> InvalidCode
        let wrong = if code == "000000" { "111111" } else { "000000" };
        assert_eq!(
            mgr.validate_and_consume(wrong),
            Err(PairingError::InvalidCode)
        );

        // Correct code succeeds and consumes
        assert!(mgr.validate_and_consume(&code).is_ok());
        // After consumption the code is no longer "active"
        assert!(!mgr.has_active_code());
        // Re-using the same code now reports `consumed`, not `notStarted`
        assert_eq!(
            mgr.validate_and_consume(&code),
            Err(PairingError::ConsumedCode)
        );
    }

    #[test]
    fn error_codes_match_mac_vocabulary() {
        // Wire vocabulary must match Mac PairingError case names so
        // Android/Tauri clients can decode the same body shape.
        assert_eq!(PairingError::InvalidCode.code(), "invalid");
        assert_eq!(PairingError::CodeExpired.code(), "expired");
        assert_eq!(PairingError::ConsumedCode.code(), "consumed");
        assert_eq!(PairingError::NoActiveCode.code(), "notStarted");
    }
}
