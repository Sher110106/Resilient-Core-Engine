use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPath {
    pub path_id: String,
    pub interface: String,
    pub local_addr: SocketAddr,
    pub remote_addr: SocketAddr,
    pub status: PathStatus,
    pub metrics: PathMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PathStatus {
    Active,
    Degraded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathMetrics {
    pub rtt_ms: u64,
    pub loss_rate: f32,
    pub bandwidth_bps: u64,
    pub last_updated: i64,
}

impl Default for PathMetrics {
    fn default() -> Self {
        Self {
            rtt_ms: 0,
            loss_rate: 0.0,
            bandwidth_bps: 0,
            last_updated: chrono::Utc::now().timestamp(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransferSession {
    pub session_id: String,
    pub file_id: String,
    pub direction: TransferDirection,
    pub paths: Vec<NetworkPath>,
    pub bytes_transferred: u64,
    pub chunks_completed: u32,
    pub status: SessionStatus,
    pub started_at: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransferDirection {
    Send,
    Receive,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionStatus {
    Initializing,
    Active,
    Paused,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub bind_addr: SocketAddr,
    pub max_idle_timeout: Duration,
    pub keep_alive_interval: Duration,
    pub max_concurrent_streams: u32,
    pub initial_mtu: u16,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:0".parse().unwrap(),
            max_idle_timeout: Duration::from_secs(60),
            keep_alive_interval: Duration::from_secs(5),
            max_concurrent_streams: 100,
            initial_mtu: 1200,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub chunks_sent: u64,
    pub chunks_received: u64,
    pub retransmissions: u64,
    pub active_connections: usize,
}

impl Default for NetworkStats {
    fn default() -> Self {
        Self {
            total_bytes_sent: 0,
            total_bytes_received: 0,
            chunks_sent: 0,
            chunks_received: 0,
            retransmissions: 0,
            active_connections: 0,
        }
    }
}
