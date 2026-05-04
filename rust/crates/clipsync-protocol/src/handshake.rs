//! WebSocket handshake message exchanged on the opening frame.
//!
//! Phase 1.10 (extensible protocol) introduces a small JSON object that
//! peers exchange immediately after the WS upgrade so both sides can
//! advertise their identity, role, and the capabilities they speak.
//!
//! Phase 2.3 adds enterprise-grade [`Hello`] / [`Welcome`] frames with
//! protocol versioning and policy negotiation.  Legacy clients that send
//! the original [`Handshake`] struct are still accepted — receivers fall
//! back to personal-mode defaults.
//!
//! See `docs/plans/master-plan-rust-fork.md`.
//!
//! Personal builds populate `device_id` + `role` and ignore unknown
//! capability strings; enterprise builds will gate features (broadcast,
//! policy, audit) on this list.

use serde::{Deserialize, Serialize};

use crate::protocol::DeviceRole;

// ---------------------------------------------------------------------------
// Protocol version
// ---------------------------------------------------------------------------

/// Current protocol version spoken by this build.
///
/// * Version **1** — original `Handshake` frame (Phase 1.10).
/// * Version **2** — `Hello` / `Welcome` enterprise handshake (Phase 2.3).
pub const CURRENT_PROTOCOL_VERSION: u32 = 2;

// ---------------------------------------------------------------------------
// Legacy handshake (v1)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Enterprise handshake (v2) — Hello / Welcome / HandshakeError
// ---------------------------------------------------------------------------

/// Enterprise hello frame sent by a client immediately after WS upgrade.
///
/// Extends [`Handshake`] with an explicit `protocol_version` so the
/// server can route the connection to the right code path and reject
/// unsupported versions early.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    /// Stable device identifier (same semantics as [`Handshake::device_id`]).
    pub device_id: String,
    /// Logical role of this device in the session.
    pub role: DeviceRole,
    /// Capability tags advertised by this device.
    pub capabilities: Vec<String>,
    /// Protocol version spoken by the client.
    pub protocol_version: u32,
}

/// Server response to a valid [`Hello`] frame.
///
/// Contains the server's own identity, its capabilities, the policy
/// assigned to the connecting device, and the agreed protocol version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Welcome {
    /// Unique identifier of the server instance.
    pub server_id: String,
    /// Capability tags the server supports.
    pub server_capabilities: Vec<String>,
    /// Policy assigned to the connecting device (e.g. `"read_write"`).
    pub your_policy: String,
    /// Protocol version the server has agreed to.
    pub protocol_version: u32,
}

/// Error frame sent when the server cannot complete the handshake.
///
/// After sending this frame the server MUST close the WebSocket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeError {
    /// Machine-readable error code (e.g. `"unsupported_version"`).
    pub code: String,
    /// Human-readable explanation.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Legacy Handshake ---------------------------------------------------

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

    // -- Hello --------------------------------------------------------------

    #[test]
    fn hello_round_trips() {
        let hello = Hello {
            device_id: "dev-001".to_string(),
            role: DeviceRole::Client,
            capabilities: vec!["broadcast".to_string(), "audit".to_string()],
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        let json = serde_json::to_string(&hello).unwrap();
        let parsed: Hello = serde_json::from_str(&json).unwrap();
        assert_eq!(hello, parsed);
    }

    #[test]
    fn hello_serializes_protocol_version() {
        let hello = Hello {
            device_id: "d1".to_string(),
            role: DeviceRole::Client,
            capabilities: vec![],
            protocol_version: 2,
        };
        let json = serde_json::to_string(&hello).unwrap();
        assert!(json.contains(r#""protocol_version":2"#));
    }

    // -- Welcome ------------------------------------------------------------

    #[test]
    fn welcome_round_trips() {
        let welcome = Welcome {
            server_id: "srv-main".to_string(),
            server_capabilities: vec!["broadcast".to_string(), "policy".to_string()],
            your_policy: "read_write".to_string(),
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        let json = serde_json::to_string(&welcome).unwrap();
        let parsed: Welcome = serde_json::from_str(&json).unwrap();
        assert_eq!(welcome, parsed);
    }

    // -- HandshakeError -----------------------------------------------------

    #[test]
    fn handshake_error_round_trips() {
        let err = HandshakeError {
            code: "unsupported_version".to_string(),
            message: "protocol version 99 is not supported".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        let parsed: HandshakeError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, parsed);
    }

    // -- Cross-compat -------------------------------------------------------

    #[test]
    fn hello_is_superset_of_handshake() {
        // A Hello frame can be deserialized as a Handshake (ignoring
        // the extra `protocol_version` field) thanks to serde's default
        // deny-unknown = false.
        let hello = Hello {
            device_id: "d1".to_string(),
            role: DeviceRole::Client,
            capabilities: vec!["broadcast".to_string()],
            protocol_version: 2,
        };
        let json = serde_json::to_string(&hello).unwrap();
        let hs: Handshake = serde_json::from_str(&json).unwrap();
        assert_eq!(hs.device_id, hello.device_id);
        assert_eq!(hs.role, hello.role);
        assert_eq!(hs.capabilities, hello.capabilities);
    }

    #[test]
    fn current_protocol_version_is_two() {
        assert_eq!(CURRENT_PROTOCOL_VERSION, 2);
    }
}
