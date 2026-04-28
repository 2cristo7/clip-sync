use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use uuid::Uuid;

use clipsync_core::protocol::ClipPayload;

/// A connected WebSocket client.
pub struct WsClient {
    pub id: String,
    pub device: String,
    pub tx: tokio::sync::mpsc::UnboundedSender<String>,
}

/// Thread-safe WebSocket hub for managing connected clients and broadcasting.
#[derive(Clone)]
pub struct WsHub {
    clients: Arc<RwLock<HashMap<String, WsClient>>>,
}

impl Default for WsHub {
    fn default() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl WsHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new client, returning its unique ID.
    pub async fn register(
        &self,
        device: String,
        tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let client = WsClient {
            id: id.clone(),
            device,
            tx,
        };
        self.clients.write().await.insert(id.clone(), client);
        tracing::info!("WebSocket client registered: {id}");
        id
    }

    /// Unregister a client by ID.
    pub async fn unregister(&self, id: &str) {
        if self.clients.write().await.remove(id).is_some() {
            tracing::info!("WebSocket client unregistered: {id}");
        }
    }

    /// Broadcast a ClipPayload to all connected clients, optionally excluding one.
    pub async fn broadcast(&self, payload: &ClipPayload, exclude: Option<&str>) {
        let json = match serde_json::to_string(payload) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!("Failed to serialize payload for broadcast: {e}");
                return;
            }
        };
        self.broadcast_raw(&json, exclude).await;
    }

    /// Broadcast a raw JSON string to all connected clients.
    pub async fn broadcast_raw(&self, json: &str, exclude: Option<&str>) {
        let clients = self.clients.read().await;
        let mut stale = Vec::new();

        for (id, client) in clients.iter() {
            if exclude == Some(id.as_str()) {
                continue;
            }
            if client.tx.send(json.to_string()).is_err() {
                stale.push(id.clone());
            }
        }

        drop(clients);

        // Clean up stale clients
        if !stale.is_empty() {
            let mut clients = self.clients.write().await;
            for id in &stale {
                clients.remove(id);
                tracing::info!("Removed stale WebSocket client: {id}");
            }
        }
    }

    /// Get the number of connected clients.
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// List connected device names.
    pub async fn device_names(&self) -> Vec<String> {
        self.clients
            .read()
            .await
            .values()
            .map(|c| c.device.clone())
            .collect()
    }
}
