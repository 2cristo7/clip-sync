//! Protocol-level constants shared across every transport.
//!
//! Connection-tuning values (WS ping interval, read timeout, etc.)
//! live in `clipsync-transport::config` instead — this module only
//! holds wire-protocol invariants.

pub const PORT: u16 = 7010;
pub const VERSION: &str = "0.1.0";
pub const MAX_PAYLOAD_BYTES: usize = 20 * 1024 * 1024;
pub const HMAC_MAX_SKEW_SECS: i64 = 60;
pub const PAIRING_CODE_TTL_SECS: u64 = 120;
pub const TLS_CERT_VALIDITY_DAYS: u32 = 365;
pub const MDNS_SERVICE_TYPE: &str = "_clipsync._tcp.local.";
