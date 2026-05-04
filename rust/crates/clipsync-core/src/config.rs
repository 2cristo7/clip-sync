use std::time::Duration;

pub const PORT: u16 = 7010;
pub const VERSION: &str = "0.1.0";
pub const MAX_PAYLOAD_BYTES: usize = 20 * 1024 * 1024;
pub const HMAC_MAX_SKEW_SECS: i64 = 60;
pub const PAIRING_CODE_TTL_SECS: u64 = 120;
pub const TLS_CERT_VALIDITY_DAYS: u32 = 365;
pub const MDNS_SERVICE_TYPE: &str = "_clipsync._tcp.local.";
pub const CLIPBOARD_POLL_MS: u64 = 500;

// --- Connection tuning ---
//
// These four constants must mirror the Mac/Android values committed in
// `fc9b1d38` ("fix[connection]: reduce ping interval 30s→5s, readTimeout
// 60s→15s, health check 15s→10s, failures 3→2"). Keeping them in this
// shared module is the single source of truth: both `clipsync-server`
// (WS hub ping loop) and `clipsync-client` (connector read timeout +
// healthcheck poll) import from here. Do not duplicate these values in
// either crate — drift between client and server tuning was the original
// bug that caused stuck-but-alive WebSockets on flaky LAN/Tailscale
// links. See `docs/plans/master-plan-rust-fork.md` Phase 1.7.

/// Interval at which the server sends WebSocket Ping frames to each
/// connected client. Mirrors Mac `WebSocketHub.swift` parity commit
/// `fc9b1d38` (5s).
pub const WS_PING_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum time the client waits for any frame (data or pong) on the
/// WebSocket before treating the link as stalled. Mirrors Android
/// `ClipClient.kt` `OkHttpClient.readTimeout` from `fc9b1d38` (15s).
pub const WS_READ_TIMEOUT: Duration = Duration::from_secs(15);

/// Cadence at which the client polls `GET /health` to detect a
/// half-open or unresponsive server independent of the WS link.
/// Mirrors Android `ClipForegroundService.HEALTH_CHECK_MS` from
/// `fc9b1d38` (10s).
pub const HEALTHCHECK_POLL_INTERVAL: Duration = Duration::from_secs(10);

/// Number of consecutive WS read-timeout or `/health` failures that the
/// client tolerates before declaring the connection dead and forcing a
/// reconnect. Mirrors Android `ClipForegroundService` consecutive-failure
/// threshold from `fc9b1d38` (2).
pub const CONSECUTIVE_FAILURE_THRESHOLD: u32 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin the four connection-tuning constants to their Mac/Android
    /// `fc9b1d38` values so any accidental drift on the Rust side (or
    /// well-meaning "let's bump the timeout" change) is caught at CI.
    /// If product wants different values, update Mac+Android FIRST,
    /// then update this test.
    #[test]
    fn connection_tuning_matches_fc9b1d38() {
        assert_eq!(WS_PING_INTERVAL, Duration::from_secs(5));
        assert_eq!(WS_READ_TIMEOUT, Duration::from_secs(15));
        assert_eq!(HEALTHCHECK_POLL_INTERVAL, Duration::from_secs(10));
        assert_eq!(CONSECUTIVE_FAILURE_THRESHOLD, 2);
    }
}
