//! WebSocket handshake message exchanged on the opening frame.
//!
//! Phase 1.10 (extensible protocol) introduces a small JSON object that
//! peers exchange immediately after the WS upgrade so both sides can
//! advertise their identity, role, and the capabilities they speak.
//!
//! See `docs/plans/master-plan-rust-fork.md`.
//!
//! Personal builds populate `device_id` + `role` and ignore unknown
//! capability strings; enterprise builds will gate features (broadcast,
//! policy, audit) on this list.

use serde::{Deserialize, Serialize};

use crate::protocol::DeviceRole;

/// Opening-frame handshake announced by both peers on a new WS session.
///
/// Field shape is intentionally tolerant: receivers MUST NOT fail on
/// unknown capability strings, and additional fields may be appended in
/// future phases (decoders use `serde`'s default ignore-unknown
/// behavior).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handshake {
    /// Stable identifier of the device emitting this handshake (UUID or
    /// host-derived string — opaque to the protocol).
    pub device_id: String,
    /// Logical role this device plays in the session.
    pub role: DeviceRole,
    /// Capability tags this device advertises, e.g. `"broadcast"`,
    /// `"policy"`, `"audit"`. Order is not significant. Unknown
    /// capabilities MUST be ignored by the receiver.
    pub capabilities: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_round_trips() {
        let hs = Handshake {
            device_id: "device-abc-123".to_string(),
            role: DeviceRole::Client,
            capabilities: vec!["broadcast".to_string(), "policy".to_string()],
        };
        let json = serde_json::to_string(&hs).unwrap();
        let parsed: Handshake = serde_json::from_str(&json).unwrap();
        assert_eq!(hs, parsed);
    }

    #[test]
    fn handshake_serializes_role_lowercase() {
        let hs = Handshake {
            device_id: "d1".to_string(),
            role: DeviceRole::Server,
            capabilities: vec![],
        };
        let json = serde_json::to_string(&hs).unwrap();
        assert!(json.contains(r#""role":"server""#));
    }
}
