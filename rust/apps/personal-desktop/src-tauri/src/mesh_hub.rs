//! Broadcast hub for the ClipSync personal peer-to-peer mesh.
//!
//! [`MeshHub`] maintains the set of active [`PeerLink`]s, broadcasts
//! clipboard events to every connected peer (excluding the originator),
//! and applies echo suppression so that events that have already been
//! seen are silently dropped.
//!
//! Echo suppression uses a sliding 30-second window keyed on
//! `(origin_device_id, timestamp)`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::peer_link::{ClipEvent, PeerLink, PeerMessage};

/// How long to keep an event fingerprint before pruning.
const DEDUP_WINDOW: Duration = Duration::from_secs(30);

/// Compact key for echo suppression.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct EventKey {
    origin_device_id: String,
    timestamp: u64,
}

/// A timestamped echo-suppression entry.
#[derive(Debug)]
struct SeenEntry {
    key: EventKey,
    seen_at: Instant,
}

/// The mesh broadcast hub.
///
/// Thread-safe: all interior state is behind `Arc<Mutex<_>>`.
pub struct MeshHub {
    /// Our own device ID (used to tag outgoing events).
    our_device_id: String,
    /// Active peer connections indexed by device ID.
    peers: Arc<Mutex<HashMap<String, PeerLink>>>,
    /// Recently seen events for echo suppression.
    seen: Arc<Mutex<Vec<SeenEntry>>>,
    /// Fast membership check for seen keys.
    seen_set: Arc<Mutex<HashSet<EventKey>>>,
}

impl MeshHub {
    /// Create a new mesh hub for the given local device ID.
    pub fn new(our_device_id: String) -> Self {
        Self {
            our_device_id,
            peers: Arc::new(Mutex::new(HashMap::new())),
            seen: Arc::new(Mutex::new(Vec::new())),
            seen_set: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Register a peer link. Replaces any existing link with the same ID.
    pub async fn add_peer(&self, link: PeerLink) {
        let id = link.device_id.clone();
        info!(peer = %id, "peer added to mesh hub");
        self.peers.lock().await.insert(id, link);
    }

    /// Remove a peer by device ID. Returns `true` if the peer existed.
    pub async fn remove_peer(&self, device_id: &str) -> bool {
        let removed = self.peers.lock().await.remove(device_id).is_some();
        if removed {
            info!(peer = %device_id, "peer removed from mesh hub");
        }
        removed
    }

    /// List device IDs of all currently connected peers.
    pub async fn connected_peers(&self) -> Vec<String> {
        self.peers.lock().await.keys().cloned().collect()
    }

    /// Broadcast a clipboard event to all connected peers except the
    /// originator. Also records the event in the echo-suppression set.
    pub async fn broadcast(&self, event: ClipEvent) {
        // Record in dedup set so we don't re-process our own broadcast.
        self.record_seen(&event).await;

        let msg = PeerMessage::Clip(event.clone());
        let peers = self.peers.lock().await;
        for (id, link) in peers.iter() {
            if *id == event.origin_device_id {
                debug!(peer = %id, "skipping originator");
                continue;
            }
            if let Err(e) = link.send(msg.clone()).await {
                warn!(peer = %id, %e, "failed to send to peer");
            }
        }
    }

    /// Handle an incoming clipboard event from the mesh.
    ///
    /// Returns `Some(event)` if the event is new and should be applied
    /// locally, or `None` if it was suppressed as a duplicate / echo.
    pub async fn on_receive(&self, event: ClipEvent) -> Option<ClipEvent> {
        // Prune stale entries first.
        self.prune_seen().await;

        let key = EventKey {
            origin_device_id: event.origin_device_id.clone(),
            timestamp: event.timestamp,
        };

        // Check echo suppression.
        {
            let set = self.seen_set.lock().await;
            if set.contains(&key) {
                debug!(
                    origin = %event.origin_device_id,
                    ts = event.timestamp,
                    "echo suppressed"
                );
                return None;
            }
        }

        // Not a duplicate — record it and propagate to other peers.
        self.record_seen(&event).await;

        // Forward to peers that are not the originator and not ourselves.
        let msg = PeerMessage::Clip(event.clone());
        let peers = self.peers.lock().await;
        for (id, link) in peers.iter() {
            if *id == event.origin_device_id {
                continue;
            }
            if let Err(e) = link.send(msg.clone()).await {
                warn!(peer = %id, %e, "forward failed");
            }
        }

        Some(event)
    }

    /// Our device ID.
    pub fn device_id(&self) -> &str {
        &self.our_device_id
    }

    // ── internal helpers ──────────────────────────────────────────

    /// Insert an event into the echo-suppression set.
    async fn record_seen(&self, event: &ClipEvent) {
        let key = EventKey {
            origin_device_id: event.origin_device_id.clone(),
            timestamp: event.timestamp,
        };
        let mut set = self.seen_set.lock().await;
        if set.insert(key.clone()) {
            self.seen.lock().await.push(SeenEntry {
                key,
                seen_at: Instant::now(),
            });
        }
    }

    /// Remove entries older than [`DEDUP_WINDOW`].
    async fn prune_seen(&self) {
        let cutoff = Instant::now() - DEDUP_WINDOW;
        let mut entries = self.seen.lock().await;
        let mut set = self.seen_set.lock().await;

        entries.retain(|entry| {
            if entry.seen_at < cutoff {
                set.remove(&entry.key);
                false
            } else {
                true
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer_link::PeerLink;

    fn make_event(origin: &str, ts: u64) -> ClipEvent {
        ClipEvent {
            origin_device_id: origin.to_string(),
            timestamp: ts,
            payload: r#"{"type":"text","data":"dGVzdA=="}"#.to_string(),
        }
    }

    #[tokio::test]
    async fn add_and_list_peers() {
        let hub = MeshHub::new("me".to_string());
        assert!(hub.connected_peers().await.is_empty());

        let (link_a, _ha) = PeerLink::new("peer-a".to_string(), 8);
        let (link_b, _hb) = PeerLink::new("peer-b".to_string(), 8);
        hub.add_peer(link_a).await;
        hub.add_peer(link_b).await;

        let mut peers = hub.connected_peers().await;
        peers.sort();
        assert_eq!(peers, vec!["peer-a", "peer-b"]);
    }

    #[tokio::test]
    async fn remove_peer() {
        let hub = MeshHub::new("me".to_string());
        let (link, _h) = PeerLink::new("peer-x".to_string(), 8);
        hub.add_peer(link).await;
        assert!(hub.remove_peer("peer-x").await);
        assert!(!hub.remove_peer("peer-x").await); // already gone
        assert!(hub.connected_peers().await.is_empty());
    }

    #[tokio::test]
    async fn echo_suppression_drops_duplicate() {
        let hub = MeshHub::new("me".to_string());

        let event = make_event("device-a", 1_000);
        // First receive should pass through.
        assert!(hub.on_receive(event.clone()).await.is_some());
        // Second receive of the same event should be suppressed.
        assert!(hub.on_receive(event).await.is_none());
    }

    #[tokio::test]
    async fn different_events_not_suppressed() {
        let hub = MeshHub::new("me".to_string());

        let e1 = make_event("device-a", 1_000);
        let e2 = make_event("device-a", 2_000);
        let e3 = make_event("device-b", 1_000);

        assert!(hub.on_receive(e1).await.is_some());
        assert!(hub.on_receive(e2).await.is_some());
        assert!(hub.on_receive(e3).await.is_some());
    }

    #[tokio::test]
    async fn broadcast_skips_originator() {
        let hub = MeshHub::new("me".to_string());

        let (link_a, handle_a) = PeerLink::new("peer-a".to_string(), 8);
        let (link_b, handle_b) = PeerLink::new("peer-b".to_string(), 8);
        hub.add_peer(link_a).await;
        hub.add_peer(link_b).await;

        // Broadcast from peer-a: peer-a should NOT receive, peer-b should.
        let event = make_event("peer-a", 5_000);
        hub.broadcast(event).await;

        // peer-b should have a message
        let msg = handle_b
            .outgoing_rx
            .lock()
            .await
            .try_recv();
        assert!(msg.is_ok(), "peer-b should receive the broadcast");

        // peer-a should NOT have a message (it was the originator)
        let msg = handle_a
            .outgoing_rx
            .lock()
            .await
            .try_recv();
        assert!(msg.is_err(), "peer-a (originator) should not receive");
    }

    #[tokio::test]
    async fn broadcast_records_in_dedup() {
        let hub = MeshHub::new("me".to_string());

        let event = make_event("me", 9_000);
        hub.broadcast(event.clone()).await;

        // The same event arriving via on_receive should be suppressed.
        assert!(hub.on_receive(event).await.is_none());
    }

    #[tokio::test]
    async fn on_receive_forwards_to_other_peers() {
        let hub = MeshHub::new("me".to_string());

        let (link_a, handle_a) = PeerLink::new("peer-a".to_string(), 8);
        let (link_b, handle_b) = PeerLink::new("peer-b".to_string(), 8);
        hub.add_peer(link_a).await;
        hub.add_peer(link_b).await;

        // Event from peer-a received: should forward to peer-b but not peer-a.
        let event = make_event("peer-a", 7_000);
        let result = hub.on_receive(event).await;
        assert!(result.is_some());

        let msg_b = handle_b.outgoing_rx.lock().await.try_recv();
        assert!(msg_b.is_ok(), "peer-b should get forwarded event");

        let msg_a = handle_a.outgoing_rx.lock().await.try_recv();
        assert!(msg_a.is_err(), "peer-a should not get its own event back");
    }

    #[tokio::test]
    async fn prune_removes_old_entries() {
        let hub = MeshHub::new("me".to_string());

        // Insert an entry manually with an old timestamp.
        {
            let key = EventKey {
                origin_device_id: "old-device".to_string(),
                timestamp: 100,
            };
            hub.seen_set.lock().await.insert(key.clone());
            hub.seen.lock().await.push(SeenEntry {
                key,
                seen_at: Instant::now() - Duration::from_secs(60), // 60s ago
            });
        }

        // Prune should remove it.
        hub.prune_seen().await;

        let set = hub.seen_set.lock().await;
        assert!(set.is_empty(), "stale entry should be pruned");
    }

    #[tokio::test]
    async fn device_id_accessor() {
        let hub = MeshHub::new("my-device".to_string());
        assert_eq!(hub.device_id(), "my-device");
    }

    #[tokio::test]
    async fn replace_peer_on_duplicate_add() {
        let hub = MeshHub::new("me".to_string());

        let (link1, _h1) = PeerLink::new("peer-dup".to_string(), 8);
        let (link2, _h2) = PeerLink::new("peer-dup".to_string(), 8);
        hub.add_peer(link1).await;
        hub.add_peer(link2).await;

        // Should only have one entry.
        assert_eq!(hub.connected_peers().await.len(), 1);
    }
}
