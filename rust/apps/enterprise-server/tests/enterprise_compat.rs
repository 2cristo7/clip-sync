//! Integration tests for enterprise server backward compatibility.
//!
//! Phase 2.14 — proves:
//! 1. Legacy Android/Mac clients (no Hello frame) work with ReadWrite default.
//! 2. Enterprise clients get the full Hello/Welcome handshake path.
//! 3. Policy enforcement survives reconnect for all 5 policy modes.
//! 4. Broadcast multicast delivers identical bytes to multiple clients.
//!
//! These tests spin up a lightweight axum WS server in-process that
//! replicates the enterprise server's handshake logic using the same
//! public crate types.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse;
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};

use base64::Engine;
use clipsync_policy::Policy;
use clipsync_protocol::handshake::{
    HandshakeError, Hello, Welcome, CURRENT_PROTOCOL_VERSION,
};
use clipsync_protocol::protocol::{ClipPayload, ClipType, DeviceRole};

// ---------------------------------------------------------------------------
// Minimal test server (replicates enterprise WS handler logic)
// ---------------------------------------------------------------------------

/// Per-client state in the test hub.
struct TestClient {
    device: String,
    tx: mpsc::UnboundedSender<String>,
}

/// Minimal hub for tracking connected clients and broadcasting.
#[derive(Default)]
struct TestHub {
    clients: RwLock<HashMap<String, TestClient>>,
}

impl TestHub {
    async fn register(&self, device: String, tx: mpsc::UnboundedSender<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.clients
            .write()
            .await
            .insert(id.clone(), TestClient { device, tx });
        id
    }

    async fn unregister(&self, id: &str) {
        self.clients.write().await.remove(id);
    }

    async fn broadcast_with_policy(
        &self,
        json: &str,
        exclude: Option<&str>,
        from_device: &str,
        policies: &RwLock<HashMap<String, Policy>>,
    ) {
        let clients = self.clients.read().await;
        let pol = policies.read().await;
        for (id, client) in clients.iter() {
            if exclude == Some(id.as_str()) {
                continue;
            }
            let device_policy = pol.get(&client.device).cloned().unwrap_or_default();
            if !device_policy.can_receive(from_device) {
                continue;
            }
            let _ = client.tx.send(json.to_string());
        }
    }
}

/// Shared state for the test server.
struct TestState {
    hub: TestHub,
    policies: RwLock<HashMap<String, Policy>>,
}

impl Default for TestState {
    fn default() -> Self {
        Self {
            hub: TestHub::default(),
            policies: RwLock::new(HashMap::new()),
        }
    }
}

/// WS handler that replicates the enterprise handshake logic.
async fn test_ws_handler(socket: WebSocket, state: Arc<TestState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Phase 1: Handshake (5s timeout)
    let device_label: String;
    let mut first_payload: Option<ClipPayload> = None;

    let first_frame = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(t) => return Some(t.to_string()),
                Message::Close(_) => return None,
                _ => continue,
            }
        }
        None
    })
    .await;

    match first_frame {
        Ok(Some(text)) => {
            if let Ok(hello) = serde_json::from_str::<Hello>(&text) {
                // Enterprise client
                if hello.protocol_version > CURRENT_PROTOCOL_VERSION {
                    let err = HandshakeError {
                        code: "unsupported_version".to_string(),
                        message: format!(
                            "server supports protocol version {} but client sent {}",
                            CURRENT_PROTOCOL_VERSION, hello.protocol_version,
                        ),
                    };
                    let _ = ws_tx
                        .send(Message::Text(serde_json::to_string(&err).unwrap().into()))
                        .await;
                    let _ = ws_tx.close().await;
                    return;
                }

                device_label = hello.device_id.clone();

                let device_policy = state
                    .policies
                    .read()
                    .await
                    .get(&hello.device_id)
                    .cloned()
                    .unwrap_or_default();

                let welcome = Welcome {
                    server_id: "test-server".to_string(),
                    server_capabilities: vec![
                        "broadcast".to_string(),
                        "policy".to_string(),
                        "audit".to_string(),
                    ],
                    your_policy: device_policy.to_string(),
                    protocol_version: CURRENT_PROTOCOL_VERSION,
                };

                if ws_tx
                    .send(Message::Text(serde_json::to_string(&welcome).unwrap().into()))
                    .await
                    .is_err()
                {
                    return;
                }
            } else {
                // Legacy client — not a Hello frame
                device_label = "legacy-client".to_string();
                if let Ok(payload) = serde_json::from_str::<ClipPayload>(&text) {
                    first_payload = Some(payload);
                }
            }
        }
        Ok(None) | Err(_) => {
            device_label = "legacy-client".to_string();
        }
    }

    // Phase 2: Register + message loop
    let client_id = state.hub.register(device_label.clone(), tx).await;

    // Deliver first payload from legacy client
    if let Some(payload) = first_payload {
        if let Ok(json) = serde_json::to_string(&payload) {
            // Check push permission
            let can_push = state
                .policies
                .read()
                .await
                .get(&device_label)
                .cloned()
                .unwrap_or_default()
                .can_push();
            if can_push {
                state
                    .hub
                    .broadcast_with_policy(&json, Some(&client_id), &device_label, &state.policies)
                    .await;
            }
        }
    }

    // Outbound: hub -> client
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Inbound: client -> hub
    let cid = client_id.clone();
    let dev = device_label.clone();
    let st = state.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(_payload) = serde_json::from_str::<ClipPayload>(&text) {
                        let can_push = st
                            .policies
                            .read()
                            .await
                            .get(&dev)
                            .cloned()
                            .unwrap_or_default()
                            .can_push();
                        if !can_push {
                            continue;
                        }
                        st.hub
                            .broadcast_with_policy(&text, Some(&cid), &dev, &st.policies)
                            .await;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    state.hub.unregister(&client_id).await;
}

async fn ws_upgrade(
    axum::extract::State(state): axum::extract::State<Arc<TestState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| test_ws_handler(socket, state))
}

/// Start a test server on a random port and return the address.
async fn start_test_server(state: Arc<TestState>) -> SocketAddr {
    let app = axum::Router::new()
        .route("/ws", get(ws_upgrade))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    addr
}

/// Connect a WS client to the test server.
async fn connect_ws(
    addr: SocketAddr,
) -> (
    futures::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        tokio_tungstenite::tungstenite::Message,
    >,
    futures::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) {
    let url = format!("ws://127.0.0.1:{}/ws", addr.port());
    let (stream, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    stream.split()
}

/// Helper to build a minimal ClipPayload.
fn make_payload(text: &str) -> ClipPayload {
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;

    ClipPayload {
        clip_type: ClipType::Text,
        mime: "text/plain".to_string(),
        data: BASE64.encode(text.as_bytes()),
        ts: 1714000000000,
        nonce: uuid::Uuid::new_v4().to_string(),
        name: None,
        policy: None,
        origin_role: None,
    }
}

/// Send a text frame via tokio-tungstenite.
async fn send_text(
    tx: &mut futures::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        tokio_tungstenite::tungstenite::Message,
    >,
    text: &str,
) {
    tx.send(tokio_tungstenite::tungstenite::Message::Text(text.into()))
        .await
        .unwrap();
}

/// Receive a text frame with timeout.
async fn recv_text_timeout(
    rx: &mut futures::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    timeout_ms: u64,
) -> Option<String> {
    match tokio::time::timeout(Duration::from_millis(timeout_ms), rx.next()).await {
        Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t)))) => Some(t.to_string()),
        _ => None,
    }
}

// ===========================================================================
// Test 1: Legacy Android client (no Hello, raw ClipPayload)
// ===========================================================================

#[tokio::test]
async fn legacy_android_compat() {
    let state = Arc::new(TestState::default());
    let addr = start_test_server(state.clone()).await;

    // Connect two clients: "legacy android" and an observer
    let (mut android_tx, mut _android_rx) = connect_ws(addr).await;
    let (mut observer_tx, mut observer_rx) = connect_ws(addr).await;

    // Observer sends Hello to identify itself
    let hello = Hello {
        device_id: "observer".to_string(),
        role: DeviceRole::Client,
        capabilities: vec![],
        protocol_version: CURRENT_PROTOCOL_VERSION,
    };
    send_text(&mut observer_tx, &serde_json::to_string(&hello).unwrap()).await;

    // Observer should get Welcome back
    let welcome_text = recv_text_timeout(&mut observer_rx, 2000).await;
    assert!(welcome_text.is_some(), "observer should receive Welcome");
    let welcome: Welcome = serde_json::from_str(&welcome_text.unwrap()).unwrap();
    assert_eq!(welcome.your_policy, "read_write");

    // Give server time to register both clients
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Legacy Android sends a raw ClipPayload (no Hello first)
    let payload = make_payload("hello from android");
    send_text(&mut android_tx, &serde_json::to_string(&payload).unwrap()).await;

    // Observer should receive the clipboard payload (legacy client gets
    // ReadWrite by default, so it can push)
    let received = recv_text_timeout(&mut observer_rx, 2000).await;
    assert!(received.is_some(), "observer should receive clipboard from legacy android");

    let received_payload: ClipPayload = serde_json::from_str(&received.unwrap()).unwrap();
    assert_eq!(received_payload.clip_type, ClipType::Text);
    assert_eq!(received_payload.data, payload.data);
}

// ===========================================================================
// Test 2: Legacy Mac Swift client (no Hello, raw ClipPayload)
// ===========================================================================

#[tokio::test]
async fn legacy_mac_swift_compat() {
    let state = Arc::new(TestState::default());
    let addr = start_test_server(state.clone()).await;

    // Mac Swift client: sends ClipPayload as first frame (no Hello)
    let (mut mac_tx, mut _mac_rx) = connect_ws(addr).await;
    let (mut observer_tx, mut observer_rx) = connect_ws(addr).await;

    // Observer identifies itself
    let hello = Hello {
        device_id: "observer-mac".to_string(),
        role: DeviceRole::Peer,
        capabilities: vec!["broadcast".to_string()],
        protocol_version: CURRENT_PROTOCOL_VERSION,
    };
    send_text(&mut observer_tx, &serde_json::to_string(&hello).unwrap()).await;
    let _ = recv_text_timeout(&mut observer_rx, 2000).await; // consume Welcome

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Mac sends a clipboard payload with image type (typical Mac behavior)
    let payload = ClipPayload {
        clip_type: ClipType::Image,
        mime: "image/png".to_string(),
        data: base64::engine::general_purpose::STANDARD.encode(b"fake-png-bytes"),
        ts: 1714000000000,
        nonce: uuid::Uuid::new_v4().to_string(),
        name: None,
        policy: None,
        origin_role: None,
    };
    send_text(&mut mac_tx, &serde_json::to_string(&payload).unwrap()).await;

    // Observer receives it — legacy Mac treated as ReadWrite
    let received = recv_text_timeout(&mut observer_rx, 2000).await;
    assert!(
        received.is_some(),
        "observer should receive clipboard from legacy Mac Swift client"
    );

    let received_payload: ClipPayload = serde_json::from_str(&received.unwrap()).unwrap();
    assert_eq!(received_payload.clip_type, ClipType::Image);
    assert_eq!(received_payload.data, payload.data);
}

// ===========================================================================
// Test 3: Enterprise client full Hello/Welcome path
// ===========================================================================

#[tokio::test]
async fn enterprise_client_full_path() {
    let state = Arc::new(TestState::default());

    // Pre-set a policy for the enterprise device
    state
        .policies
        .write()
        .await
        .insert("enterprise-laptop-1".to_string(), Policy::ReadWrite);

    let addr = start_test_server(state.clone()).await;

    // Enterprise client connects with Hello
    let (mut client_tx, mut client_rx) = connect_ws(addr).await;

    let hello = Hello {
        device_id: "enterprise-laptop-1".to_string(),
        role: DeviceRole::Client,
        capabilities: vec![
            "broadcast".to_string(),
            "policy".to_string(),
            "audit".to_string(),
        ],
        protocol_version: CURRENT_PROTOCOL_VERSION,
    };
    send_text(&mut client_tx, &serde_json::to_string(&hello).unwrap()).await;

    // Should receive Welcome with server capabilities and policy
    let welcome_text = recv_text_timeout(&mut client_rx, 2000).await;
    assert!(welcome_text.is_some(), "enterprise client should receive Welcome");

    let welcome: Welcome = serde_json::from_str(&welcome_text.unwrap()).unwrap();
    assert_eq!(welcome.server_id, "test-server");
    assert!(welcome.server_capabilities.contains(&"broadcast".to_string()));
    assert!(welcome.server_capabilities.contains(&"policy".to_string()));
    assert!(welcome.server_capabilities.contains(&"audit".to_string()));
    assert_eq!(welcome.your_policy, "read_write");
    assert_eq!(welcome.protocol_version, CURRENT_PROTOCOL_VERSION);

    // Now connect an observer
    let (mut obs_tx, mut obs_rx) = connect_ws(addr).await;
    let obs_hello = Hello {
        device_id: "observer-ent".to_string(),
        role: DeviceRole::Client,
        capabilities: vec![],
        protocol_version: CURRENT_PROTOCOL_VERSION,
    };
    send_text(&mut obs_tx, &serde_json::to_string(&obs_hello).unwrap()).await;
    let _ = recv_text_timeout(&mut obs_rx, 2000).await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Enterprise client sends clipboard — should be received by observer
    let payload = make_payload("enterprise secret clipboard");
    send_text(&mut client_tx, &serde_json::to_string(&payload).unwrap()).await;

    let received = recv_text_timeout(&mut obs_rx, 2000).await;
    assert!(
        received.is_some(),
        "observer should receive clipboard from enterprise client with ReadWrite policy"
    );
}

// ===========================================================================
// Test 4: Policy enforcement across reconnects (all 5 policies)
// ===========================================================================

#[tokio::test]
async fn policy_enforcement_read_only_across_reconnect() {
    let state = Arc::new(TestState::default());
    state
        .policies
        .write()
        .await
        .insert("ro-device".to_string(), Policy::ReadOnly);

    let addr = start_test_server(state.clone()).await;

    for attempt in 0..2 {
        let (mut tx, mut rx) = connect_ws(addr).await;
        let hello = Hello {
            device_id: "ro-device".to_string(),
            role: DeviceRole::Client,
            capabilities: vec![],
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        send_text(&mut tx, &serde_json::to_string(&hello).unwrap()).await;

        let welcome_text = recv_text_timeout(&mut rx, 2000).await.unwrap();
        let welcome: Welcome = serde_json::from_str(&welcome_text).unwrap();
        assert_eq!(
            welcome.your_policy, "read_only",
            "attempt {attempt}: ReadOnly policy should persist across reconnect"
        );

        // ReadOnly device tries to push — connect an observer to check
        let (mut obs_tx, mut obs_rx) = connect_ws(addr).await;
        let obs_hello = Hello {
            device_id: format!("obs-ro-{attempt}"),
            role: DeviceRole::Client,
            capabilities: vec![],
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        send_text(&mut obs_tx, &serde_json::to_string(&obs_hello).unwrap()).await;
        let _ = recv_text_timeout(&mut obs_rx, 2000).await;

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Push from read-only device
        let payload = make_payload("should not arrive");
        send_text(&mut tx, &serde_json::to_string(&payload).unwrap()).await;

        // Observer should NOT receive it (push rejected)
        let received = recv_text_timeout(&mut obs_rx, 500).await;
        assert!(
            received.is_none(),
            "attempt {attempt}: ReadOnly device push should be rejected"
        );

        // Disconnect by dropping the connection
        drop(tx);
        drop(rx);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn policy_enforcement_write_only_across_reconnect() {
    let state = Arc::new(TestState::default());
    state
        .policies
        .write()
        .await
        .insert("wo-device".to_string(), Policy::WriteOnly);

    let addr = start_test_server(state.clone()).await;

    for attempt in 0..2 {
        // WriteOnly device connects
        let (mut wo_tx, mut wo_rx) = connect_ws(addr).await;
        let hello = Hello {
            device_id: "wo-device".to_string(),
            role: DeviceRole::Client,
            capabilities: vec![],
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        send_text(&mut wo_tx, &serde_json::to_string(&hello).unwrap()).await;

        let welcome_text = recv_text_timeout(&mut wo_rx, 2000).await.unwrap();
        let welcome: Welcome = serde_json::from_str(&welcome_text).unwrap();
        assert_eq!(
            welcome.your_policy, "write_only",
            "attempt {attempt}: WriteOnly policy should persist"
        );

        // Another device that can push
        let (mut pusher_tx, mut _pusher_rx) = connect_ws(addr).await;
        let pusher_hello = Hello {
            device_id: format!("pusher-wo-{attempt}"),
            role: DeviceRole::Client,
            capabilities: vec![],
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        send_text(&mut pusher_tx, &serde_json::to_string(&pusher_hello).unwrap()).await;

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Pusher sends clipboard — WriteOnly device should NOT receive
        let payload = make_payload("should not reach write-only");
        send_text(&mut pusher_tx, &serde_json::to_string(&payload).unwrap()).await;

        let received = recv_text_timeout(&mut wo_rx, 500).await;
        assert!(
            received.is_none(),
            "attempt {attempt}: WriteOnly device should not receive broadcasts"
        );

        drop(wo_tx);
        drop(wo_rx);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn policy_enforcement_muted_across_reconnect() {
    let state = Arc::new(TestState::default());
    state
        .policies
        .write()
        .await
        .insert("muted-device".to_string(), Policy::Muted);

    let addr = start_test_server(state.clone()).await;

    for attempt in 0..2 {
        let (mut tx, mut rx) = connect_ws(addr).await;
        let hello = Hello {
            device_id: "muted-device".to_string(),
            role: DeviceRole::Client,
            capabilities: vec![],
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        send_text(&mut tx, &serde_json::to_string(&hello).unwrap()).await;

        let welcome_text = recv_text_timeout(&mut rx, 2000).await.unwrap();
        let welcome: Welcome = serde_json::from_str(&welcome_text).unwrap();
        assert_eq!(
            welcome.your_policy, "muted",
            "attempt {attempt}: Muted policy should persist"
        );

        // Observer to check push rejection
        let (mut obs_tx, mut obs_rx) = connect_ws(addr).await;
        let obs_hello = Hello {
            device_id: format!("obs-muted-{attempt}"),
            role: DeviceRole::Client,
            capabilities: vec![],
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        send_text(&mut obs_tx, &serde_json::to_string(&obs_hello).unwrap()).await;
        let _ = recv_text_timeout(&mut obs_rx, 2000).await;

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Muted device tries to push — should be rejected
        let payload = make_payload("muted push attempt");
        send_text(&mut tx, &serde_json::to_string(&payload).unwrap()).await;

        let received = recv_text_timeout(&mut obs_rx, 500).await;
        assert!(
            received.is_none(),
            "attempt {attempt}: Muted device push should be rejected"
        );

        // Another device pushes — muted should not receive
        let payload2 = make_payload("broadcast to muted");
        send_text(&mut obs_tx, &serde_json::to_string(&payload2).unwrap()).await;

        let received2 = recv_text_timeout(&mut rx, 500).await;
        assert!(
            received2.is_none(),
            "attempt {attempt}: Muted device should not receive broadcasts"
        );

        drop(tx);
        drop(rx);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn policy_enforcement_follow_leader_across_reconnect() {
    let state = Arc::new(TestState::default());
    state.policies.write().await.insert(
        "follower-device".to_string(),
        Policy::FollowLeader {
            leader_device_id: "leader-device".to_string(),
        },
    );

    let addr = start_test_server(state.clone()).await;

    for attempt in 0..2 {
        // Follower connects
        let (mut follower_tx, mut follower_rx) = connect_ws(addr).await;
        let hello = Hello {
            device_id: "follower-device".to_string(),
            role: DeviceRole::Client,
            capabilities: vec![],
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        send_text(&mut follower_tx, &serde_json::to_string(&hello).unwrap()).await;

        let welcome_text = recv_text_timeout(&mut follower_rx, 2000).await.unwrap();
        let welcome: Welcome = serde_json::from_str(&welcome_text).unwrap();
        assert!(
            welcome.your_policy.contains("follow_leader"),
            "attempt {attempt}: FollowLeader policy should persist, got: {}",
            welcome.your_policy
        );

        // Leader connects
        let (mut leader_tx, mut _leader_rx) = connect_ws(addr).await;
        let leader_hello = Hello {
            device_id: "leader-device".to_string(),
            role: DeviceRole::Client,
            capabilities: vec![],
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        send_text(&mut leader_tx, &serde_json::to_string(&leader_hello).unwrap()).await;

        // Non-leader connects
        let (mut other_tx, mut _other_rx) = connect_ws(addr).await;
        let other_hello = Hello {
            device_id: format!("other-{attempt}"),
            role: DeviceRole::Client,
            capabilities: vec![],
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        send_text(&mut other_tx, &serde_json::to_string(&other_hello).unwrap()).await;

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Leader pushes — follower should receive
        let leader_payload = make_payload("from the leader");
        send_text(
            &mut leader_tx,
            &serde_json::to_string(&leader_payload).unwrap(),
        )
        .await;

        let received = recv_text_timeout(&mut follower_rx, 2000).await;
        assert!(
            received.is_some(),
            "attempt {attempt}: follower should receive from leader"
        );

        // Non-leader pushes — follower should NOT receive
        let other_payload = make_payload("from non-leader");
        send_text(
            &mut other_tx,
            &serde_json::to_string(&other_payload).unwrap(),
        )
        .await;

        let received2 = recv_text_timeout(&mut follower_rx, 500).await;
        assert!(
            received2.is_none(),
            "attempt {attempt}: follower should NOT receive from non-leader"
        );

        drop(follower_tx);
        drop(follower_rx);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn policy_enforcement_read_write_across_reconnect() {
    let state = Arc::new(TestState::default());
    state
        .policies
        .write()
        .await
        .insert("rw-device".to_string(), Policy::ReadWrite);

    let addr = start_test_server(state.clone()).await;

    for attempt in 0..2 {
        let (mut tx, mut rx) = connect_ws(addr).await;
        let hello = Hello {
            device_id: "rw-device".to_string(),
            role: DeviceRole::Client,
            capabilities: vec![],
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        send_text(&mut tx, &serde_json::to_string(&hello).unwrap()).await;

        let welcome_text = recv_text_timeout(&mut rx, 2000).await.unwrap();
        let welcome: Welcome = serde_json::from_str(&welcome_text).unwrap();
        assert_eq!(
            welcome.your_policy, "read_write",
            "attempt {attempt}: ReadWrite policy should persist"
        );

        // Observer
        let (mut obs_tx, mut obs_rx) = connect_ws(addr).await;
        let obs_hello = Hello {
            device_id: format!("obs-rw-{attempt}"),
            role: DeviceRole::Client,
            capabilities: vec![],
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        send_text(&mut obs_tx, &serde_json::to_string(&obs_hello).unwrap()).await;
        let _ = recv_text_timeout(&mut obs_rx, 2000).await;

        tokio::time::sleep(Duration::from_millis(100)).await;

        // RW device pushes — observer should receive
        let payload = make_payload("rw push");
        send_text(&mut tx, &serde_json::to_string(&payload).unwrap()).await;

        let received = recv_text_timeout(&mut obs_rx, 2000).await;
        assert!(
            received.is_some(),
            "attempt {attempt}: ReadWrite device push should be delivered"
        );

        // Observer pushes — RW device should receive
        let payload2 = make_payload("to rw device");
        send_text(&mut obs_tx, &serde_json::to_string(&payload2).unwrap()).await;

        let received2 = recv_text_timeout(&mut rx, 2000).await;
        assert!(
            received2.is_some(),
            "attempt {attempt}: ReadWrite device should receive broadcasts"
        );

        drop(tx);
        drop(rx);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

// ===========================================================================
// Test 5: Broadcast multicast — 3 clients, 1MB file, identical bytes
// ===========================================================================

#[tokio::test]
async fn broadcast_multicast_identical_bytes() {
    let state = Arc::new(TestState::default());
    let addr = start_test_server(state.clone()).await;

    // Connect 3 receivers + 1 sender
    let mut receivers = Vec::new();
    for i in 0..3 {
        let (mut tx, mut rx) = connect_ws(addr).await;
        let hello = Hello {
            device_id: format!("receiver-{i}"),
            role: DeviceRole::Client,
            capabilities: vec![],
            protocol_version: CURRENT_PROTOCOL_VERSION,
        };
        send_text(&mut tx, &serde_json::to_string(&hello).unwrap()).await;
        let _ = recv_text_timeout(&mut rx, 2000).await; // consume Welcome
        receivers.push((tx, rx));
    }

    let (mut sender_tx, mut _sender_rx) = connect_ws(addr).await;
    let sender_hello = Hello {
        device_id: "sender".to_string(),
        role: DeviceRole::Client,
        capabilities: vec!["broadcast".to_string()],
        protocol_version: CURRENT_PROTOCOL_VERSION,
    };
    send_text(&mut sender_tx, &serde_json::to_string(&sender_hello).unwrap()).await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Create a 1MB payload
    let large_data = vec![0xABu8; 1024 * 1024]; // 1 MB
    let payload = ClipPayload {
        clip_type: ClipType::File,
        mime: "application/octet-stream".to_string(),
        data: base64::engine::general_purpose::STANDARD.encode(&large_data),
        ts: 1714000000000,
        nonce: uuid::Uuid::new_v4().to_string(),
        name: Some("test-file.bin".to_string()),
        policy: None,
        origin_role: None,
    };
    let payload_json = serde_json::to_string(&payload).unwrap();

    // Sender broadcasts
    send_text(&mut sender_tx, &payload_json).await;

    // All 3 receivers should get identical bytes
    let mut received_payloads = Vec::new();
    for (i, (_tx, rx)) in receivers.iter_mut().enumerate() {
        let text = recv_text_timeout(rx, 5000)
            .await
            .unwrap_or_else(|| panic!("receiver-{i} should have received the broadcast"));
        received_payloads.push(text);
    }

    // Verify all 3 received identical content
    assert_eq!(received_payloads.len(), 3);
    assert_eq!(
        received_payloads[0], received_payloads[1],
        "receiver-0 and receiver-1 should receive identical bytes"
    );
    assert_eq!(
        received_payloads[1], received_payloads[2],
        "receiver-1 and receiver-2 should receive identical bytes"
    );

    // Verify the content matches the original
    let received: ClipPayload = serde_json::from_str(&received_payloads[0]).unwrap();
    assert_eq!(received.clip_type, ClipType::File);
    assert_eq!(received.data, payload.data);
    assert_eq!(received.name, Some("test-file.bin".to_string()));

    // Decode and verify the raw bytes match
    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&received.data)
        .unwrap();
    assert_eq!(decoded.len(), 1024 * 1024, "decoded size should be 1 MB");
    assert_eq!(decoded, large_data, "decoded bytes should match original");
}
