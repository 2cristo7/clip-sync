//! Device policy engine for ClipSync Enterprise.
//!
//! Defines 5 sync modes that control how clipboard data flows to/from
//! each device.  Policies are stored as JSON in the `devices.policy`
//! database column and checked by the WebSocket hub on every frame.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Policy enum
// ---------------------------------------------------------------------------

/// The five supported sync modes for a device.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Policy {
    /// Default — bidirectional clipboard sync.
    #[default]
    ReadWrite,
    /// Receives clipboard from others, cannot push its own.
    ReadOnly,
    /// Pushes its clipboard to others, does not receive.
    WriteOnly,
    /// Paired but no clipboard flow in either direction.
    Muted,
    /// Only receives clipboard from a specific device.
    FollowLeader { leader_device_id: String },
}

impl Policy {
    /// Whether this device is allowed to push (send) clipboard content.
    pub fn can_push(&self) -> bool {
        matches!(self, Self::ReadWrite | Self::WriteOnly)
    }

    /// Whether this device is allowed to receive clipboard content from
    /// the given `from_device_id`.
    pub fn can_receive(&self, from_device_id: &str) -> bool {
        match self {
            Self::ReadWrite | Self::ReadOnly => true,
            Self::WriteOnly | Self::Muted => false,
            Self::FollowLeader { leader_device_id } => leader_device_id == from_device_id,
        }
    }

    /// Parse a policy from its JSON string representation.
    /// Returns `ReadWrite` for empty/null strings for backward compat.
    pub fn from_json_str(s: &str) -> Self {
        if s.is_empty() || s == "null" {
            return Self::default();
        }
        // Support legacy plain-string values like "ReadWrite"
        match s {
            "ReadWrite" | "read_write" => return Self::ReadWrite,
            "ReadOnly" | "read_only" => return Self::ReadOnly,
            "WriteOnly" | "write_only" => return Self::WriteOnly,
            "Muted" | "muted" => return Self::Muted,
            _ => {}
        }
        serde_json::from_str(s).unwrap_or_default()
    }

    /// Serialize the policy to its JSON string representation.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).expect("Policy serialization cannot fail")
    }
}

impl std::fmt::Display for Policy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadWrite => write!(f, "read_write"),
            Self::ReadOnly => write!(f, "read_only"),
            Self::WriteOnly => write!(f, "write_only"),
            Self::Muted => write!(f, "muted"),
            Self::FollowLeader { leader_device_id } => {
                write!(f, "follow_leader({leader_device_id})")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    #[error("invalid policy JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_read_write() {
        assert_eq!(Policy::default(), Policy::ReadWrite);
    }

    #[test]
    fn can_push_variants() {
        assert!(Policy::ReadWrite.can_push());
        assert!(Policy::WriteOnly.can_push());
        assert!(!Policy::ReadOnly.can_push());
        assert!(!Policy::Muted.can_push());
        assert!(!Policy::FollowLeader {
            leader_device_id: "x".into()
        }
        .can_push());
    }

    #[test]
    fn can_receive_variants() {
        assert!(Policy::ReadWrite.can_receive("any"));
        assert!(Policy::ReadOnly.can_receive("any"));
        assert!(!Policy::WriteOnly.can_receive("any"));
        assert!(!Policy::Muted.can_receive("any"));

        let fl = Policy::FollowLeader {
            leader_device_id: "leader-1".into(),
        };
        assert!(fl.can_receive("leader-1"));
        assert!(!fl.can_receive("other-device"));
    }

    #[test]
    fn json_round_trip() {
        let cases = vec![
            Policy::ReadWrite,
            Policy::ReadOnly,
            Policy::WriteOnly,
            Policy::Muted,
            Policy::FollowLeader {
                leader_device_id: "dev-42".into(),
            },
        ];

        for policy in cases {
            let json = policy.to_json_string();
            let parsed = Policy::from_json_str(&json);
            assert_eq!(policy, parsed, "round-trip failed for {json}");
        }
    }

    #[test]
    fn from_legacy_strings() {
        assert_eq!(Policy::from_json_str("ReadWrite"), Policy::ReadWrite);
        assert_eq!(Policy::from_json_str("read_write"), Policy::ReadWrite);
        assert_eq!(Policy::from_json_str("read_only"), Policy::ReadOnly);
        assert_eq!(Policy::from_json_str(""), Policy::ReadWrite);
        assert_eq!(Policy::from_json_str("null"), Policy::ReadWrite);
    }

    #[test]
    fn serde_tagged_format() {
        let json = r#"{"mode":"follow_leader","leader_device_id":"abc"}"#;
        let p: Policy = serde_json::from_str(json).unwrap();
        assert_eq!(
            p,
            Policy::FollowLeader {
                leader_device_id: "abc".into()
            }
        );
    }
}
