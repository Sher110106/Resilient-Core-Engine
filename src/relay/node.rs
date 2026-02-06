//! Relay node implementation
//!
//! A relay node stores and forwards chunks between disconnected parties.

use crate::relay::storage::RelayStorage;
use crate::relay::types::{
    ForwardingPolicy, PeerInfo, RelayConfig, RelayError, RelayMessage, RelayResult, RelayStats,
    RouteInfo,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// A store-and-forward relay node
pub struct RelayNode {
    /// Node configuration
    config: RelayConfig,

    /// Chunk storage
    storage: Arc<RelayStorage>,

    /// Node statistics
    stats: Arc<RelayStatsInner>,

    /// Known peers
    peers: RwLock<HashMap<String, PeerInfo>>,

    /// Running state
    running: AtomicBool,

    /// Event sender for async operations
    event_tx: Option<mpsc::Sender<RelayEvent>>,
}

struct RelayStatsInner {
    chunks_received: AtomicU64,
    chunks_forwarded: AtomicU64,
    chunks_expired: AtomicU64,
    chunks_dropped: AtomicU64,
    bytes_received: AtomicU64,
    bytes_forwarded: AtomicU64,
}

impl Default for RelayStatsInner {
    fn default() -> Self {
        Self {
            chunks_received: AtomicU64::new(0),
            chunks_forwarded: AtomicU64::new(0),
            chunks_expired: AtomicU64::new(0),
            chunks_dropped: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            bytes_forwarded: AtomicU64::new(0),
        }
    }
}

/// Events emitted by the relay node
#[derive(Debug)]
pub enum RelayEvent {
    /// Chunk received and stored
    ChunkStored { chunk_id: String, size: usize },

    /// Chunk forwarded successfully
    ChunkForwarded {
        chunk_id: String,
        destination: SocketAddr,
    },

    /// Chunk expired before delivery
    ChunkExpired { chunk_id: String },

    /// Peer connected
    PeerConnected { node_id: String },

    /// Peer disconnected
    PeerDisconnected { node_id: String },

    /// Error occurred
    Error { message: String },
}

impl RelayNode {
    /// Create a new relay node
    pub fn new(config: RelayConfig) -> RelayResult<Self> {
        let storage = Arc::new(RelayStorage::new(
            config.max_storage_bytes,
            config.max_hold_time,
        ));

        let mut peers = HashMap::new();
        for peer in &config.peers {
            peers.insert(peer.node_id.clone(), peer.clone());
        }

        Ok(Self {
            config,
            storage,
            stats: Arc::new(RelayStatsInner::default()),
            peers: RwLock::new(peers),
            running: AtomicBool::new(false),
            event_tx: None,
        })
    }

    /// Create with an event channel for monitoring
    pub fn with_events(mut self, tx: mpsc::Sender<RelayEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Get the node ID
    pub fn node_id(&self) -> &str {
        &self.config.node_id
    }

    /// Get the listen address
    pub fn listen_addr(&self) -> SocketAddr {
        self.config.listen_addr
    }

    /// Receive and store a chunk for forwarding
    pub async fn receive_chunk(
        &self,
        chunk_id: String,
        route: RouteInfo,
        data: Vec<u8>,
    ) -> RelayResult<()> {
        // Check TTL
        if route.is_expired() {
            self.stats.chunks_dropped.fetch_add(1, Ordering::Relaxed);
            return Err(RelayError::ChunkExpired(chunk_id));
        }

        // Check hop limit
        if route.hop_count() >= self.config.policy.max_hops as usize {
            self.stats.chunks_dropped.fetch_add(1, Ordering::Relaxed);
            return Err(RelayError::ChunkExpired(format!(
                "{}: max hops exceeded",
                chunk_id
            )));
        }

        let size = data.len();

        // Add this node to the route
        let mut route = route;
        route.add_hop(&self.config.node_id);

        // Store the chunk
        self.storage.store(chunk_id.clone(), route, data)?;

        // Update stats
        self.stats.chunks_received.fetch_add(1, Ordering::Relaxed);
        self.stats
            .bytes_received
            .fetch_add(size as u64, Ordering::Relaxed);

        // Emit event
        self.emit_event(RelayEvent::ChunkStored {
            chunk_id: chunk_id.clone(),
            size,
        })
        .await;

        // Forward immediately if policy allows
        if self.config.policy.forward_immediately {
            let _ = self.try_forward_chunk(&chunk_id).await;
        }

        Ok(())
    }

    /// Try to forward a specific chunk
    pub async fn try_forward_chunk(&self, chunk_id: &str) -> RelayResult<bool> {
        let chunk = match self.storage.get(chunk_id) {
            Some(c) => c,
            None => return Err(RelayError::ChunkNotFound(chunk_id.to_string())),
        };

        // Try direct delivery first if policy prefers it
        if self.config.policy.prefer_direct {
            if self.try_direct_delivery(&chunk).await? {
                return Ok(true);
            }
        }

        // Try relay through peers
        if self.try_relay_delivery(&chunk).await? {
            return Ok(true);
        }

        // Record attempt
        self.storage.record_attempt(chunk_id);

        Ok(false)
    }

    /// Attempt direct delivery to destination
    async fn try_direct_delivery(
        &self,
        chunk: &crate::relay::storage::StoredChunk,
    ) -> RelayResult<bool> {
        let destination = chunk.route.destination;

        // In a real implementation, this would attempt QUIC connection
        // For now, we simulate the attempt
        let success = self.simulate_connection(destination).await;

        if success {
            self.stats.chunks_forwarded.fetch_add(1, Ordering::Relaxed);
            self.stats
                .bytes_forwarded
                .fetch_add(chunk.size() as u64, Ordering::Relaxed);

            // Remove from storage
            self.storage.remove(&chunk.chunk_id);

            self.emit_event(RelayEvent::ChunkForwarded {
                chunk_id: chunk.chunk_id.clone(),
                destination,
            })
            .await;

            return Ok(true);
        }

        Ok(false)
    }

    /// Attempt relay through a peer
    async fn try_relay_delivery(
        &self,
        chunk: &crate::relay::storage::StoredChunk,
    ) -> RelayResult<bool> {
        let peers = self.peers.read();

        // Sort peers by priority
        let mut peer_list: Vec<_> = peers.values().collect();
        peer_list.sort_by_key(|p| p.priority);

        for peer in peer_list {
            // Skip peers we've already visited
            if chunk.route.hops.contains(&peer.node_id) {
                continue;
            }

            // Try to forward to this peer
            if self.forward_to_peer(peer, chunk).await? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Forward chunk to a peer relay
    async fn forward_to_peer(
        &self,
        peer: &PeerInfo,
        chunk: &crate::relay::storage::StoredChunk,
    ) -> RelayResult<bool> {
        // In a real implementation, this would send via QUIC
        let success = self.simulate_connection(peer.addr).await;

        if success {
            self.stats.chunks_forwarded.fetch_add(1, Ordering::Relaxed);
            self.stats
                .bytes_forwarded
                .fetch_add(chunk.size() as u64, Ordering::Relaxed);

            // Remove from storage
            self.storage.remove(&chunk.chunk_id);

            self.emit_event(RelayEvent::ChunkForwarded {
                chunk_id: chunk.chunk_id.clone(),
                destination: peer.addr,
            })
            .await;

            return Ok(true);
        }

        Ok(false)
    }

    /// Simulate a connection attempt (placeholder for real QUIC)
    async fn simulate_connection(&self, _addr: SocketAddr) -> bool {
        // In tests, always succeed
        // In real implementation, attempt actual connection
        true
    }

    /// Add a peer
    pub fn add_peer(&self, peer: PeerInfo) {
        self.peers.write().insert(peer.node_id.clone(), peer);
    }

    /// Remove a peer
    pub fn remove_peer(&self, node_id: &str) -> Option<PeerInfo> {
        self.peers.write().remove(node_id)
    }

    /// Get all known peers
    pub fn get_peers(&self) -> Vec<PeerInfo> {
        self.peers.read().values().cloned().collect()
    }

    /// Run cleanup and forwarding cycle
    pub async fn maintenance_cycle(&self) {
        // Clean up expired chunks
        let expired = self.storage.cleanup_expired();
        for chunk_id in expired {
            self.stats.chunks_expired.fetch_add(1, Ordering::Relaxed);
            self.emit_event(RelayEvent::ChunkExpired { chunk_id }).await;
        }

        // Try to forward pending chunks
        let pending = self
            .storage
            .get_pending(100, self.config.policy.retry_cooldown);

        for chunk in pending {
            if chunk.forward_attempts < self.config.max_forward_retries {
                let _ = self.try_forward_chunk(&chunk.chunk_id).await;
            } else {
                // Max retries exceeded, drop chunk
                self.storage.remove(&chunk.chunk_id);
                self.stats.chunks_dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Get current statistics
    pub fn stats(&self) -> RelayStats {
        let storage_stats = self.storage.stats();

        RelayStats {
            chunks_received: self.stats.chunks_received.load(Ordering::Relaxed),
            chunks_forwarded: self.stats.chunks_forwarded.load(Ordering::Relaxed),
            chunks_expired: self.stats.chunks_expired.load(Ordering::Relaxed),
            chunks_dropped: self.stats.chunks_dropped.load(Ordering::Relaxed),
            bytes_received: self.stats.bytes_received.load(Ordering::Relaxed),
            bytes_forwarded: self.stats.bytes_forwarded.load(Ordering::Relaxed),
            storage_used: storage_stats.used_bytes,
            stored_chunks: storage_stats.total_chunks,
            active_peers: self.peers.read().len() as u64,
            avg_forward_latency_ms: 0, // Would need timing tracking
        }
    }

    /// Get storage statistics
    pub fn storage_stats(&self) -> crate::relay::storage::StorageStats {
        self.storage.stats()
    }

    /// Handle incoming relay message
    pub async fn handle_message(&self, message: RelayMessage) -> RelayResult<Option<RelayMessage>> {
        match message {
            RelayMessage::Store {
                chunk_id,
                route,
                data,
            } => {
                self.receive_chunk(chunk_id.clone(), route, data).await?;
                Ok(Some(RelayMessage::Ack {
                    chunk_id,
                    node_id: self.config.node_id.clone(),
                }))
            }

            RelayMessage::Query { chunk_id } => {
                let stored = self.storage.get(&chunk_id).is_some();
                Ok(Some(RelayMessage::Status {
                    chunk_id,
                    stored,
                    forwarded: false, // Would need tracking
                }))
            }

            RelayMessage::Hello { node_id, addr } => {
                let peer = PeerInfo::new(node_id, addr);
                self.add_peer(peer);
                Ok(Some(RelayMessage::PeerList {
                    peers: self.get_peers(),
                }))
            }

            RelayMessage::PeerList { peers } => {
                for peer in peers {
                    if peer.node_id != self.config.node_id {
                        self.add_peer(peer);
                    }
                }
                Ok(None)
            }

            RelayMessage::Ack { .. } | RelayMessage::Status { .. } => Ok(None),
        }
    }

    /// Emit an event if there's a listener
    async fn emit_event(&self, event: RelayEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event).await;
        }
    }
}

/// Builder for relay nodes
pub struct RelayNodeBuilder {
    config: RelayConfig,
}

impl RelayNodeBuilder {
    pub fn new() -> Self {
        Self {
            config: RelayConfig::default(),
        }
    }

    pub fn node_id(mut self, id: impl Into<String>) -> Self {
        self.config.node_id = id.into();
        self
    }

    pub fn listen_addr(mut self, addr: SocketAddr) -> Self {
        self.config.listen_addr = addr;
        self
    }

    pub fn max_storage(mut self, bytes: u64) -> Self {
        self.config.max_storage_bytes = bytes;
        self
    }

    pub fn max_hold_time(mut self, duration: Duration) -> Self {
        self.config.max_hold_time = duration;
        self
    }

    pub fn add_peer(mut self, peer: PeerInfo) -> Self {
        self.config.peers.push(peer);
        self
    }

    pub fn policy(mut self, policy: ForwardingPolicy) -> Self {
        self.config.policy = policy;
        self
    }

    pub fn build(self) -> RelayResult<RelayNode> {
        RelayNode::new(self.config)
    }
}

impl Default for RelayNodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_node() -> RelayNode {
        RelayNodeBuilder::new()
            .node_id("test-node")
            .listen_addr("127.0.0.1:9000".parse().unwrap())
            .max_storage(1024 * 1024)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn test_receive_and_forward() {
        let node = create_test_node();

        let route = RouteInfo::new("source", "127.0.0.1:8000".parse().unwrap(), "transfer-1", 1);

        node.receive_chunk("chunk-1".into(), route, vec![1, 2, 3, 4])
            .await
            .unwrap();

        let stats = node.stats();
        assert_eq!(stats.chunks_received, 1);
        // With simulate_connection returning true, it should be forwarded
        assert_eq!(stats.chunks_forwarded, 1);
    }

    #[tokio::test]
    async fn test_peer_management() {
        let node = create_test_node();

        let peer = PeerInfo::new("peer-1", "192.168.1.100:9000".parse().unwrap());
        node.add_peer(peer);

        let peers = node.get_peers();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].node_id, "peer-1");

        node.remove_peer("peer-1");
        assert!(node.get_peers().is_empty());
    }

    #[tokio::test]
    async fn test_handle_message_store() {
        let node = create_test_node();

        let route = RouteInfo::new("source", "127.0.0.1:8000".parse().unwrap(), "transfer-1", 1);

        let message = RelayMessage::Store {
            chunk_id: "chunk-1".into(),
            route,
            data: vec![1, 2, 3, 4],
        };

        let response = node.handle_message(message).await.unwrap();

        assert!(matches!(response, Some(RelayMessage::Ack { .. })));
    }

    #[tokio::test]
    async fn test_handle_message_hello() {
        let node = create_test_node();

        let message = RelayMessage::Hello {
            node_id: "peer-1".into(),
            addr: "192.168.1.100:9000".parse().unwrap(),
        };

        let response = node.handle_message(message).await.unwrap();

        assert!(matches!(response, Some(RelayMessage::PeerList { .. })));
        assert_eq!(node.get_peers().len(), 1);
    }

    #[tokio::test]
    async fn test_ttl_enforcement() {
        let node = create_test_node();

        let mut route =
            RouteInfo::new("source", "127.0.0.1:8000".parse().unwrap(), "transfer-1", 1);

        // Exhaust TTL
        route.ttl = 0;

        let result = node
            .receive_chunk("chunk-1".into(), route, vec![1, 2, 3, 4])
            .await;

        assert!(matches!(result, Err(RelayError::ChunkExpired(_))));
    }

    #[test]
    fn test_builder() {
        let node = RelayNodeBuilder::new()
            .node_id("custom-node")
            .listen_addr("0.0.0.0:9999".parse().unwrap())
            .max_storage(500 * 1024 * 1024)
            .max_hold_time(Duration::from_secs(3600))
            .build()
            .unwrap();

        assert_eq!(node.node_id(), "custom-node");
        assert_eq!(
            node.listen_addr(),
            "0.0.0.0:9999".parse::<SocketAddr>().unwrap()
        );
    }
}
