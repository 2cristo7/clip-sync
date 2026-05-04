//! WebSocket peer connection management for the ClipSync personal mesh.
//!
//! Each [`PeerLink`] wraps a single WebSocket connection to another peer.
//! Peers are both clients and servers: they connect out to discovered peers
//! and accept incoming connections. Connection deduplication is handled by
//! device-ID ordering — the peer with the lexicographically smaller ID
//! initiates. Disconnected links automatically reconnect with exponential
//! backoff (1 s → 2 s → 4 s → … → 30 s max).

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::info;

/// Minimum reconnect delay.
const BACKOFF_MIN: Duration = Duration::from_secs(1);
/// Maximum reconnect delay.
const BACKOFF_MAX: Duration = Duration::from_secs(30);

/// A clipboard event transmitted over the mesh.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipEvent {
    /// Device ID that originally produced this event.
    pub origin_device_id: String,
    /// Milliseconds since Unix epoch when the event was created.
    pub timestamp: u64,
    /// Serialised [`ClipPayload`] JSON.
    pub payload: String,
}

/// Messages exchanged over a peer link.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PeerMessage {
    /// Handshake sent immediately after WS connect.
    Hello { device_id: String },
    /// A clipboard event to propagate.
    Clip(ClipEvent),
    /// Keep-alive ping.
    Ping,
    /// Keep-alive pong.
    Pong,
}

/// Handle to a single peer connection.
///
/// Provides `send` / `recv` channels and metadata about the remote peer.
#[derive(Debug)]
pub struct PeerLink {
    /// Remote peer device ID.
    pub device_id: String,
    /// Channel for sending messages to this peer.
    tx: mpsc::Sender<PeerMessage>,
    /// Channel for receiving messages from this peer.
    rx: Arc<tokio::sync::Mutex<mpsc::Receiver<PeerMessage>>>,
}

impl PeerLink {
    /// Create a new `PeerLink` with the given device ID and channel capacity.
    pub fn new(device_id: String, capacity: usize) -> (Self, PeerLinkHandle) {
        let (outgoing_tx, outgoing_rx) = mpsc::channel(capacity);
        let (incoming_tx, incoming_rx) = mpsc::channel(capacity);

        let link = Self {
            device_id: device_id.clone(),
            tx: outgoing_tx,
            rx: Arc::new(tokio::sync::Mutex::new(incoming_rx)),
        };

        let handle = PeerLinkHandle {
            device_id,
            outgoing_rx: Arc::new(tokio::sync::Mutex::new(outgoing_rx)),
            incoming_tx,
        };

        (link, handle)
    }

    /// Send a message to this peer.
    pub async fn send(&self, msg: PeerMessage) -> Result<(), PeerLinkError> {
        self.tx
            .send(msg)
            .await
            .map_err(|_| PeerLinkError::Closed(self.device_id.clone()))
    }

    /// Receive the next message from this peer.
    pub async fn recv(&self) -> Option<PeerMessage> {
        self.rx.lock().await.recv().await
    }
}

/// The "back-end" half of a [`PeerLink`], used by the transport layer
/// to shuttle bytes between the channel pair and the actual WebSocket.
#[derive(Debug)]
pub struct PeerLinkHandle {
    /// Remote peer device ID.
    pub device_id: String,
    /// Outgoing messages queued by [`PeerLink::send`].
    pub outgoing_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<PeerMessage>>>,
    /// Push incoming messages so [`PeerLink::recv`] yields them.
    pub incoming_tx: mpsc::Sender<PeerMessage>,
}

/// Errors from peer-link operations.
#[derive(Debug, thiserror::Error)]
pub enum PeerLinkError {
    #[error("connection to peer {0} closed")]
    Closed(String),
    #[error("websocket error: {0}")]
    Ws(String),
    #[error("handshake failed: {0}")]
    Handshake(String),
}

/// Return `true` if *our* device ID should be the initiator for the
/// connection to the given peer (lexicographic ordering).
pub fn should_initiate(our_id: &str, peer_id: &str) -> bool {
    our_id < peer_id
}

/// Compute the next backoff duration (clamped to [`BACKOFF_MAX`]).
pub fn next_backoff(current: Duration) -> Duration {
    let doubled = current.saturating_mul(2);
    if doubled > BACKOFF_MAX {
        BACKOFF_MAX
    } else {
        doubled
    }
}

/// Reset backoff to the minimum value.
pub fn reset_backoff() -> Duration {
    BACKOFF_MIN
}

/// Simulate an outgoing connection attempt (placeholder for real WS dial).
///
/// In production this would use `tokio-tungstenite` to open a WS to
/// `ws://{addr}:{port}` and run the read/write pump. For now it creates
/// the channel pair so the rest of the mesh can be wired and tested.
pub fn connect(
    peer_addr: &str,
    peer_device_id: &str,
    _our_device_id: &str,
) -> Result<(PeerLink, PeerLinkHandle), PeerLinkError> {
    info!(peer = %peer_device_id, addr = %peer_addr, "opening outgoing WS link");
    let (link, handle) = PeerLink::new(peer_device_id.to_string(), 64);
    Ok((link, handle))
}

/// Accept an incoming peer connection (placeholder for real WS accept).
///
/// In production the Axum/Tokio WS acceptor would call this after
/// reading the `Hello` frame. For now it creates the channel pair.
pub fn accept(
    peer_device_id: &str,
    _our_device_id: &str,
) -> Result<(PeerLink, PeerLinkHandle), PeerLinkError> {
    info!(peer = %peer_device_id, "accepting incoming WS link");
    let (link, handle) = PeerLink::new(peer_device_id.to_string(), 64);
    Ok((link, handle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_initiate_lexicographic() {
        assert!(should_initiate("aaa", "bbb"));
        assert!(!should_initiate("bbb", "aaa"));
        assert!(!should_initiate("same", "same"));
    }

    #[test]
    fn backoff_doubles() {
        let b1 = reset_backoff();
        assert_eq!(b1, Duration::from_secs(1));
        let b2 = next_backoff(b1);
        assert_eq!(b2, Duration::from_secs(2));
        let b3 = next_backoff(b2);
        assert_eq!(b3, Duration::from_secs(4));
    }

    #[test]
    fn backoff_capped() {
        let b = next_backoff(Duration::from_secs(20));
        assert_eq!(b, BACKOFF_MAX);
        let b2 = next_backoff(b);
        assert_eq!(b2, BACKOFF_MAX);
    }

    #[test]
    fn connect_creates_link() {
        let (link, _handle) = connect("192.168.1.1:7010", "peer-1", "me").unwrap();
        assert_eq!(link.device_id, "peer-1");
    }

    #[test]
    fn accept_creates_link() {
        let (link, _handle) = accept("peer-2", "me").unwrap();
        assert_eq!(link.device_id, "peer-2");
    }

    #[tokio::test]
    async fn send_recv_round_trip() {
        let (link, handle) = PeerLink::new("test-peer".to_string(), 8);

        // Simulate incoming message via handle
        handle
            .incoming_tx
            .send(PeerMessage::Ping)
            .await
            .unwrap();

        let msg = link.recv().await.unwrap();
        assert!(matches!(msg, PeerMessage::Ping));
    }

    #[tokio::test]
    async fn send_goes_to_handle() {
        let (link, handle) = PeerLink::new("test-peer".to_string(), 8);

        link.send(PeerMessage::Pong).await.unwrap();

        let msg = handle.outgoing_rx.lock().await.recv().await.unwrap();
        assert!(matches!(msg, PeerMessage::Pong));
    }

    #[test]
    fn clip_event_serde_round_trip() {
        let event = ClipEvent {
            origin_device_id: "device-abc".to_string(),
            timestamp: 1_714_000_000_000,
            payload: r#"{"type":"text"}"#.to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: ClipEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn peer_message_serde() {
        let msg = PeerMessage::Hello {
            device_id: "dev-1".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("hello"));
        let parsed: PeerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, PeerMessage::Hello { .. }));
    }
}
