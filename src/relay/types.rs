//! Relay types and configuration

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use thiserror::Error;

/// Result type for relay operations
pub type RelayResult<T> = Result<T, RelayError>;

/// Relay-specific errors
#[derive(Debug, Error)]
pub enum RelayError {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Chunk not found: {0}")]
    ChunkNotFound(String),

    #[error("Route not found to destination: {0}")]
    NoRoute(String),

    #[error("Storage capacity exceeded")]
    CapacityExceeded,

    #[error("Chunk expired: {0}")]
    ChunkExpired(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Configuration for a relay node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    /// Unique identifier for this relay node
    pub node_id: String,

    /// Address to listen on
    pub listen_addr: SocketAddr,

    /// Maximum storage capacity in bytes
    pub max_storage_bytes: u64,

    /// Maximum time to hold chunks before expiry
    pub max_hold_time: Duration,

    /// How often to attempt forwarding stored chunks
    pub forward_interval: Duration,

    /// Maximum retry attempts for forwarding
    pub max_forward_retries: u32,

    /// Known peer relay nodes
    pub peers: Vec<PeerInfo>,

    /// Forwarding policy
    pub policy: ForwardingPolicy,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            node_id: uuid::Uuid::new_v4().to_string(),
            listen_addr: "0.0.0.0:9000".parse().unwrap(),
            max_storage_bytes: 1024 * 1024 * 1024, // 1GB
            max_hold_time: Duration::from_secs(24 * 60 * 60), // 24 hours
            forward_interval: Duration::from_secs(30),
            max_forward_retries: 10,
            peers: Vec::new(),
            policy: ForwardingPolicy::default(),
        }
    }
}

/// Information about a peer relay node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer's unique identifier
    pub node_id: String,

    /// Peer's address
    pub addr: SocketAddr,

    /// Priority (lower = preferred)
    pub priority: u8,

    /// Whether this peer is currently reachable
    #[serde(skip)]
    pub reachable: bool,

    /// Last successful contact time
    #[serde(skip)]
    pub last_contact: Option<std::time::Instant>,
}

impl PeerInfo {
    /// Create a new peer info
    pub fn new(node_id: impl Into<String>, addr: SocketAddr) -> Self {
        Self {
            node_id: node_id.into(),
            addr,
            priority: 100,
            reachable: false,
            last_contact: None,
        }
    }

    /// Set priority
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
}

/// Policy for forwarding chunks through the relay network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardingPolicy {
    /// Forward immediately upon receipt (vs batch)
    pub forward_immediately: bool,

    /// Maximum hops allowed (0 = unlimited)
    pub max_hops: u8,

    /// Prefer direct delivery over relay
    pub prefer_direct: bool,

    /// Forward critical priority chunks first
    pub priority_aware: bool,

    /// Minimum delay between forward attempts to same destination
    pub retry_cooldown: Duration,
}

impl Default for ForwardingPolicy {
    fn default() -> Self {
        Self {
            forward_immediately: true,
            max_hops: 5,
            prefer_direct: true,
            priority_aware: true,
            retry_cooldown: Duration::from_secs(5),
        }
    }
}

/// Routing information for a chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    /// Source node
    pub source: String,

    /// Final destination address
    pub destination: SocketAddr,

    /// Transfer ID
    pub transfer_id: String,

    /// Nodes this chunk has passed through
    pub hops: Vec<String>,

    /// Original priority
    pub priority: u8,

    /// Time-to-live in hops
    pub ttl: u8,
}

impl RouteInfo {
    /// Create new route info
    pub fn new(
        source: impl Into<String>,
        destination: SocketAddr,
        transfer_id: impl Into<String>,
        priority: u8,
    ) -> Self {
        Self {
            source: source.into(),
            destination,
            transfer_id: transfer_id.into(),
            hops: Vec::new(),
            priority,
            ttl: 10,
        }
    }

    /// Add a hop to the route
    pub fn add_hop(&mut self, node_id: &str) {
        self.hops.push(node_id.to_string());
        if self.ttl > 0 {
            self.ttl -= 1;
        }
    }

    /// Check if TTL is exhausted
    pub fn is_expired(&self) -> bool {
        self.ttl == 0
    }

    /// Get hop count
    pub fn hop_count(&self) -> usize {
        self.hops.len()
    }
}

/// Statistics for a relay node
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelayStats {
    /// Total chunks received
    pub chunks_received: u64,

    /// Total chunks forwarded successfully
    pub chunks_forwarded: u64,

    /// Total chunks expired (not delivered in time)
    pub chunks_expired: u64,

    /// Total chunks dropped (storage full, TTL exhausted, etc.)
    pub chunks_dropped: u64,

    /// Total bytes received
    pub bytes_received: u64,

    /// Total bytes forwarded
    pub bytes_forwarded: u64,

    /// Current storage usage in bytes
    pub storage_used: u64,

    /// Current number of stored chunks
    pub stored_chunks: u64,

    /// Number of active peer connections
    pub active_peers: u64,

    /// Average forward latency in milliseconds
    pub avg_forward_latency_ms: u64,
}

impl RelayStats {
    /// Calculate forwarding success rate
    pub fn success_rate(&self) -> f64 {
        let total = self.chunks_forwarded + self.chunks_expired + self.chunks_dropped;
        if total == 0 {
            return 100.0;
        }
        self.chunks_forwarded as f64 / total as f64 * 100.0
    }

    /// Calculate storage utilization percentage
    pub fn storage_utilization(&self, max_bytes: u64) -> f64 {
        if max_bytes == 0 {
            return 0.0;
        }
        self.storage_used as f64 / max_bytes as f64 * 100.0
    }
}

impl std::fmt::Display for RelayStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Relay: {} fwd, {} exp, {} drop ({:.1}% success), {} stored",
            self.chunks_forwarded,
            self.chunks_expired,
            self.chunks_dropped,
            self.success_rate(),
            self.stored_chunks
        )
    }
}

/// Message types for relay protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelayMessage {
    /// Store a chunk for forwarding
    Store {
        chunk_id: String,
        route: RouteInfo,
        data: Vec<u8>,
    },

    /// Acknowledge receipt of a chunk
    Ack { chunk_id: String, node_id: String },

    /// Request chunk status
    Query { chunk_id: String },

    /// Response to status query
    Status {
        chunk_id: String,
        stored: bool,
        forwarded: bool,
    },

    /// Peer discovery
    Hello { node_id: String, addr: SocketAddr },

    /// Peer list exchange
    PeerList { peers: Vec<PeerInfo> },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_config_default() {
        let config = RelayConfig::default();
        assert!(config.max_storage_bytes > 0);
        assert!(config.max_hold_time > Duration::ZERO);
    }

    #[test]
    fn test_route_info() {
        let mut route = RouteInfo::new(
            "source-node",
            "127.0.0.1:8000".parse().unwrap(),
            "transfer-1",
            1,
        );

        assert_eq!(route.hop_count(), 0);
        assert!(!route.is_expired());

        route.add_hop("relay-1");
        route.add_hop("relay-2");

        assert_eq!(route.hop_count(), 2);
        assert_eq!(route.ttl, 8);
    }

    #[test]
    fn test_relay_stats() {
        let mut stats = RelayStats::default();
        stats.chunks_forwarded = 90;
        stats.chunks_expired = 5;
        stats.chunks_dropped = 5;

        assert!((stats.success_rate() - 90.0).abs() < 0.1);
    }

    #[test]
    fn test_peer_info() {
        let peer = PeerInfo::new("peer-1", "192.168.1.100:9000".parse().unwrap()).with_priority(50);

        assert_eq!(peer.node_id, "peer-1");
        assert_eq!(peer.priority, 50);
        assert!(!peer.reachable);
    }
}
