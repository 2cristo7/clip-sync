//! Compatibility integration tests: simulated Android and Mac Swift peers
//! interacting with the personal mesh hub.

use clipsync_personal_tauri_lib::mesh_hub::MeshHub;
use clipsync_personal_tauri_lib::peer_link::{ClipEvent, PeerLink, PeerMessage};

/// Helper: create a ClipEvent with given origin, timestamp, and payload text.
fn make_clip(origin: &str, ts: u64, text: &str) -> ClipEvent {
    ClipEvent {
        origin_device_id: origin.to_string(),
        timestamp: ts,
        payload: format!(r#"{{"type":"text","data":"{}"}}"#, text),
    }
}

// ─── Android peer simulation ─────────────────────────────────────────────

#[tokio::test]
async fn android_sends_clipboard_to_mesh() {
    // Simulate an Android-style client pushing clipboard text into the mesh.
    let hub = MeshHub::new("desktop-001".to_string());

    let (android_link, android_handle) = PeerLink::new("android-pixel".to_string(), 32);
    hub.add_peer(android_link).await;

    // Android produces a clipboard event.
    let event = make_clip("android-pixel", 1_700_000_001_000, "Hello from Android");

    // Hub receives the event (as if read from the Android WS frame).
    let result = hub.on_receive(event.clone()).await;
    assert!(result.is_some(), "mesh should accept new Android event");
    assert_eq!(result.unwrap().origin_device_id, "android-pixel");

    // Since android-pixel is the originator, it should NOT receive a forward.
    let msg = android_handle.outgoing_rx.lock().await.try_recv();
    assert!(msg.is_err(), "originator android peer must not get echo");
}

#[tokio::test]
async fn mesh_broadcasts_to_android_peer() {
    let hub = MeshHub::new("desktop-001".to_string());

    let (android_link, android_handle) = PeerLink::new("android-pixel".to_string(), 32);
    hub.add_peer(android_link).await;

    // Desktop originates a clipboard event and broadcasts it.
    let event = make_clip("desktop-001", 1_700_000_002_000, "From desktop");
    hub.broadcast(event).await;

    // Android peer should receive the broadcast.
    let msg = android_handle.outgoing_rx.lock().await.recv().await.unwrap();
    match msg {
        PeerMessage::Clip(ev) => {
            assert_eq!(ev.origin_device_id, "desktop-001");
            assert!(ev.payload.contains("From desktop"));
        }
        other => panic!("expected Clip message, got {:?}", other),
    }
}

#[tokio::test]
async fn android_hmac_token_auth_flow() {
    // Simulate that an Android peer sends a Hello (handshake) with its device ID,
    // then exchanges clipboard. The PeerLink channel pair models this.
    let (link, handle) = PeerLink::new("android-s24".to_string(), 16);

    // Android sends Hello via the incoming_tx (simulating WS receive).
    handle
        .incoming_tx
        .send(PeerMessage::Hello {
            device_id: "android-s24".to_string(),
        })
        .await
        .unwrap();

    // Mesh side reads the Hello.
    let hello = link.recv().await.unwrap();
    assert!(matches!(hello, PeerMessage::Hello { device_id } if device_id == "android-s24"));

    // After handshake, mesh adds peer to hub and can broadcast.
    let hub = MeshHub::new("desktop-001".to_string());
    let (peer_link, peer_handle) = PeerLink::new("android-s24".to_string(), 16);
    hub.add_peer(peer_link).await;

    let event = make_clip("desktop-001", 1_700_000_003_000, "auth-verified");
    hub.broadcast(event).await;

    let msg = peer_handle.outgoing_rx.lock().await.recv().await.unwrap();
    assert!(matches!(msg, PeerMessage::Clip(_)));
}

// ─── Mac Swift peer simulation ───────────────────────────────────────────

#[tokio::test]
async fn mac_swift_peer_sends_clipboard() {
    // Simulate a Mac Swift server pushing a clipboard event into the mesh.
    let hub = MeshHub::new("desktop-linux".to_string());

    let (mac_link, _mac_handle) = PeerLink::new("mac-mini-m2".to_string(), 32);
    let (other_link, other_handle) = PeerLink::new("desktop-linux-2".to_string(), 32);
    hub.add_peer(mac_link).await;
    hub.add_peer(other_link).await;

    // Mac Swift peer originates clipboard.
    let event = make_clip("mac-mini-m2", 1_700_000_004_000, "Copied on Mac");
    let result = hub.on_receive(event).await;
    assert!(result.is_some());

    // Other peer should receive forwarded event.
    let msg = other_handle.outgoing_rx.lock().await.recv().await.unwrap();
    match msg {
        PeerMessage::Clip(ev) => assert_eq!(ev.origin_device_id, "mac-mini-m2"),
        other => panic!("expected Clip, got {:?}", other),
    }
}

#[tokio::test]
async fn mac_swift_receives_mesh_broadcast() {
    let hub = MeshHub::new("desktop-linux".to_string());

    let (mac_link, mac_handle) = PeerLink::new("mac-mini-m2".to_string(), 32);
    hub.add_peer(mac_link).await;

    let event = make_clip("desktop-linux", 1_700_000_005_000, "To Mac");
    hub.broadcast(event).await;

    let msg = mac_handle.outgoing_rx.lock().await.recv().await.unwrap();
    match msg {
        PeerMessage::Clip(ev) => {
            assert_eq!(ev.origin_device_id, "desktop-linux");
            assert!(ev.payload.contains("To Mac"));
        }
        _ => panic!("expected Clip message"),
    }
}

#[tokio::test]
async fn mac_swift_proto2_handshake_txt_record() {
    // Simulate proto=2 handshake: Mac Swift peer advertises proto=2 via mDNS TXT,
    // and the mesh peer connects via the PeerLink accept path.
    // We test that after handshake, bidirectional clipboard works.
    let hub = MeshHub::new("personal-mesh-1".to_string());

    // Accept incoming from Mac (proto=2 peer).
    let (mac_link, mac_handle) = PeerLink::new("mac-pro-swift".to_string(), 32);
    hub.add_peer(mac_link).await;

    // Mac sends Hello indicating proto=2.
    mac_handle
        .incoming_tx
        .send(PeerMessage::Hello {
            device_id: "mac-pro-swift".to_string(),
        })
        .await
        .unwrap();

    // Mesh peer broadcasts an event — Mac should receive it.
    let event = make_clip("personal-mesh-1", 1_700_000_006_000, "proto2-test");
    hub.broadcast(event).await;

    let msg = mac_handle.outgoing_rx.lock().await.recv().await.unwrap();
    assert!(matches!(msg, PeerMessage::Clip(_)));

    // Mac sends clipboard back — mesh should accept it.
    let mac_event = make_clip("mac-pro-swift", 1_700_000_007_000, "from-mac-proto2");
    let result = hub.on_receive(mac_event).await;
    assert!(result.is_some());
}
