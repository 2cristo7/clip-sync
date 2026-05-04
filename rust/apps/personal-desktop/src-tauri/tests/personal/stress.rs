//! Stress and network partition tests for the personal mesh hub.

use std::collections::HashSet;

use clipsync_personal_tauri_lib::mesh_hub::MeshHub;
use clipsync_personal_tauri_lib::peer_link::{ClipEvent, PeerLink, PeerMessage};

/// Helper: create a ClipEvent.
fn make_clip(origin: &str, ts: u64, text: &str) -> ClipEvent {
    ClipEvent {
        origin_device_id: origin.to_string(),
        timestamp: ts,
        payload: format!(r#"{{"type":"text","data":"{}"}}"#, text),
    }
}

// ─── 3-peer mesh stress test: 100 events ─────────────────────────────────

#[tokio::test]
async fn stress_100_events_zero_duplicates() {
    // 3-peer mesh: hub-A, hub-B, hub-C. We simulate by using one MeshHub
    // with 3 peer links representing the 3 nodes.
    let hub = MeshHub::new("hub-central".to_string());

    let (link_a, handle_a) = PeerLink::new("peer-a".to_string(), 256);
    let (link_b, handle_b) = PeerLink::new("peer-b".to_string(), 256);
    let (link_c, handle_c) = PeerLink::new("peer-c".to_string(), 256);
    hub.add_peer(link_a).await;
    hub.add_peer(link_b).await;
    hub.add_peer(link_c).await;

    let origins = ["peer-a", "peer-b", "peer-c"];
    let mut all_events: Vec<(String, u64)> = Vec::new();

    // Send 100 events round-robin from the 3 peers.
    for i in 0u64..100 {
        let origin = origins[(i % 3) as usize];
        let ts = 1_700_000_000_000 + i;
        let event = make_clip(origin, ts, &format!("event-{}", i));
        all_events.push((origin.to_string(), ts));

        let result = hub.on_receive(event).await;
        assert!(result.is_some(), "event {} should not be suppressed", i);
    }

    // Verify echo suppression: re-sending the same events should all be dropped.
    let mut duplicates_passed = 0u64;
    for (origin, ts) in &all_events {
        let event = make_clip(origin, *ts, "duplicate-attempt");
        if hub.on_receive(event).await.is_some() {
            duplicates_passed += 1;
        }
    }
    assert_eq!(duplicates_passed, 0, "echo suppression must drop all 100 duplicates");

    // Verify each non-originator peer received forwarded events.
    // peer-a originated events 0, 3, 6, ... (34 events). So peer-a should
    // receive the other 66 events (from peer-b and peer-c).
    let mut a_count = 0u64;
    while handle_a.outgoing_rx.lock().await.try_recv().is_ok() {
        a_count += 1;
    }
    // peer-a should receive events NOT originated by peer-a = 66 or 67
    assert!(a_count >= 66, "peer-a should receive at least 66 forwarded events, got {}", a_count);

    let mut b_count = 0u64;
    while handle_b.outgoing_rx.lock().await.try_recv().is_ok() {
        b_count += 1;
    }
    assert!(b_count >= 66, "peer-b should receive at least 66 forwarded events, got {}", b_count);

    let mut c_count = 0u64;
    while handle_c.outgoing_rx.lock().await.try_recv().is_ok() {
        c_count += 1;
    }
    assert!(c_count >= 66, "peer-c should receive at least 66 forwarded events, got {}", c_count);
}

#[tokio::test]
async fn stress_unique_origin_ts_pairs() {
    // Verify each of the 100 events has a unique (origin_device_id, timestamp) pair.
    let mut seen_keys: HashSet<(String, u64)> = HashSet::new();
    let origins = ["peer-a", "peer-b", "peer-c"];

    for i in 0u64..100 {
        let origin = origins[(i % 3) as usize].to_string();
        let ts = 1_700_000_000_000 + i;
        let inserted = seen_keys.insert((origin, ts));
        assert!(inserted, "event {} must have unique (origin, ts)", i);
    }
    assert_eq!(seen_keys.len(), 100);
}

// ─── Network partition test ──────────────────────────────────────────────

#[tokio::test]
async fn network_partition_peer_disconnect() {
    let hub = MeshHub::new("hub-main".to_string());

    let (link_a, _handle_a) = PeerLink::new("peer-a".to_string(), 32);
    let (link_b, handle_b) = PeerLink::new("peer-b".to_string(), 32);
    hub.add_peer(link_a).await;
    hub.add_peer(link_b).await;

    assert_eq!(hub.connected_peers().await.len(), 2);

    // Simulate peer-a disconnects (WS drop).
    let removed = hub.remove_peer("peer-a").await;
    assert!(removed, "peer-a should be removable");
    assert_eq!(hub.connected_peers().await.len(), 1);

    // Events should still flow to peer-b.
    let event = make_clip("hub-main", 1_700_000_100_000, "after-partition");
    hub.broadcast(event).await;

    let msg = handle_b.outgoing_rx.lock().await.recv().await.unwrap();
    assert!(matches!(msg, PeerMessage::Clip(_)));
}

#[tokio::test]
async fn network_partition_peer_reconnects() {
    let hub = MeshHub::new("hub-main".to_string());

    let (link_a, _handle_a) = PeerLink::new("peer-a".to_string(), 32);
    hub.add_peer(link_a).await;

    // Disconnect.
    hub.remove_peer("peer-a").await;
    assert!(hub.connected_peers().await.is_empty());

    // Reconnect with a fresh link (simulates new WS connection).
    let (link_a_new, handle_a_new) = PeerLink::new("peer-a".to_string(), 32);
    hub.add_peer(link_a_new).await;
    assert_eq!(hub.connected_peers().await.len(), 1);

    // Mesh reforms — broadcasts work again.
    let event = make_clip("hub-main", 1_700_000_200_000, "reconnected");
    hub.broadcast(event).await;

    let msg = handle_a_new.outgoing_rx.lock().await.recv().await.unwrap();
    match msg {
        PeerMessage::Clip(ev) => assert!(ev.payload.contains("reconnected")),
        _ => panic!("expected Clip after reconnect"),
    }
}
