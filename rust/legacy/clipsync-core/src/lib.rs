//! Transitional re-export shell for the workspace split (Phase 1.9).
//!
//! Existing call-sites use paths like `clipsync_core::protocol::ClipPayload`
//! or `clipsync_core::config::WS_PING_INTERVAL`. The real code now lives in
//! `clipsync-protocol`, `clipsync-crypto`, `clipsync-transport` and
//! `clipsync-clipboard`; this crate just exposes them under the legacy
//! `clipsync_core::*` namespace so the still-living legacy `clipsync-server`
//! and `clipsync-client` keep compiling unchanged. New code should import
//! from the focused crates directly.

pub mod protocol {
    pub use clipsync_protocol::protocol::*;
}

pub mod pairing {
    pub use clipsync_protocol::pairing::*;
}

pub mod hmac {
    pub use clipsync_crypto::hmac::*;
}

pub mod tls {
    pub use clipsync_crypto::tls::*;
}

pub mod fingerprint {
    pub use clipsync_crypto::fingerprint::*;
}

pub mod token_store {
    pub use clipsync_crypto::token_store::*;
}

pub mod mdns {
    pub use clipsync_transport::mdns::*;
}

pub mod clipboard {
    pub use clipsync_clipboard::clipboard::*;
}

/// Aggregated config namespace mirroring the pre-split shape.
///
/// Constants now live in their owning crate (wire-protocol values in
/// `clipsync-protocol`, transport tuning in `clipsync-transport`,
/// clipboard polling in `clipsync-clipboard`); this module just glues
/// them back together so legacy paths like
/// `clipsync_core::config::WS_PING_INTERVAL` keep resolving.
pub mod config {
    pub use clipsync_clipboard::config::CLIPBOARD_POLL_MS;
    pub use clipsync_protocol::config::{
        HMAC_MAX_SKEW_SECS, MAX_PAYLOAD_BYTES, MDNS_SERVICE_TYPE, PAIRING_CODE_TTL_SECS, PORT,
        TLS_CERT_VALIDITY_DAYS, VERSION,
    };
    pub use clipsync_transport::config::{
        CONSECUTIVE_FAILURE_THRESHOLD, HEALTHCHECK_POLL_INTERVAL, WS_PING_INTERVAL, WS_READ_TIMEOUT,
    };
}
