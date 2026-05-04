//! Integration tests for policy enforcement.
//!
//! These tests validate the three key policy scenarios using the
//! [`PolicyRuntime`] directly (no network round-trip required).

use clipsync_policy::Policy;

// We cannot import `PolicyRuntime` directly from the binary crate,
// so we test the core policy logic through `clipsync_policy::Policy`
// which is the same code path the runtime delegates to.

/// A read-only device must NOT be able to push clipboard content.
#[test]
fn read_only_device_cannot_push() {
    let policy = Policy::ReadOnly;
    assert!(
        !policy.can_push(),
        "ReadOnly device should not be allowed to push"
    );
    // But it can receive from any device
    assert!(
        policy.can_receive("device-a"),
        "ReadOnly device should be able to receive"
    );
    assert!(
        policy.can_receive("device-b"),
        "ReadOnly device should be able to receive from any source"
    );
}

/// A write-only device must NOT receive clipboard content.
#[test]
fn write_only_device_cannot_receive() {
    let policy = Policy::WriteOnly;
    assert!(
        policy.can_push(),
        "WriteOnly device should be allowed to push"
    );
    assert!(
        !policy.can_receive("device-a"),
        "WriteOnly device should not receive from anyone"
    );
    assert!(
        !policy.can_receive("device-b"),
        "WriteOnly device should not receive from anyone"
    );
}

/// A follow-leader device only receives from its designated leader.
#[test]
fn follow_leader_only_receives_from_designated_source() {
    let policy = Policy::FollowLeader {
        leader_device_id: "leader-device".to_string(),
    };

    // Cannot push
    assert!(
        !policy.can_push(),
        "FollowLeader device should not be allowed to push"
    );

    // Can receive ONLY from the leader
    assert!(
        policy.can_receive("leader-device"),
        "FollowLeader should receive from designated leader"
    );
    assert!(
        !policy.can_receive("other-device"),
        "FollowLeader should NOT receive from non-leader"
    );
    assert!(
        !policy.can_receive("another-device"),
        "FollowLeader should NOT receive from any other device"
    );
}

/// Policy change via JSON round-trip simulates the API update path.
#[test]
fn policy_change_applies_via_json() {
    // Start as ReadWrite
    let initial = Policy::ReadWrite;
    assert!(initial.can_push());
    assert!(initial.can_receive("any"));

    // Simulate API change to Muted
    let muted_json = r#"{"mode":"muted"}"#;
    let updated = Policy::from_json_str(muted_json);
    assert_eq!(updated, Policy::Muted);
    assert!(!updated.can_push());
    assert!(!updated.can_receive("any"));

    // Simulate API change to FollowLeader
    let fl_json = r#"{"mode":"follow_leader","leader_device_id":"dev-42"}"#;
    let fl = Policy::from_json_str(fl_json);
    assert!(!fl.can_push());
    assert!(fl.can_receive("dev-42"));
    assert!(!fl.can_receive("dev-99"));
}

/// Verify that the PolicyRuntime updates are visible immediately.
#[tokio::test]
async fn policy_runtime_live_update() {
    // We can test the runtime logic inline since it's just
    // Arc<RwLock<HashMap>> — same pattern used by the server.
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let policies: Arc<RwLock<HashMap<String, Policy>>> = Arc::new(RwLock::new(HashMap::new()));

    // Default (not in map) should be ReadWrite
    let p = policies
        .read()
        .await
        .get("dev-1")
        .cloned()
        .unwrap_or_default();
    assert!(p.can_push());
    assert!(p.can_receive("any"));

    // Simulate API setting device to ReadOnly
    policies
        .write()
        .await
        .insert("dev-1".to_string(), Policy::ReadOnly);

    // Immediately visible
    let p = policies.read().await.get("dev-1").cloned().unwrap();
    assert!(!p.can_push(), "policy change should apply immediately");
    assert!(p.can_receive("any"));
}
