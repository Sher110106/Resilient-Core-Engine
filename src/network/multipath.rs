use crate::chunk::{Chunk, Priority};
use crate::network::error::{NetworkError, NetworkResult};
use crate::network::quic_transport::QuicTransport;
use crate::network::types::{NetworkPath, PathMetrics, PathStatus};
use parking_lot::RwLock;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

pub struct MultiPathManager {
    paths: Arc<RwLock<Vec<NetworkPath>>>,
    transport: Arc<QuicTransport>,
}

impl MultiPathManager {
    pub fn new(transport: Arc<QuicTransport>) -> Self {
        Self {
            paths: Arc::new(RwLock::new(Vec::new())),
            transport,
        }
    }

    /// Discover available network paths (simplified version)
    pub async fn discover_paths(&self, remote_addr: SocketAddr) -> NetworkResult<Vec<NetworkPath>> {
        let mut paths = Vec::new();

        // Get local addresses using socket2
        let local_addrs = Self::get_local_addresses()?;

        for (idx, local_ip) in local_addrs.iter().enumerate() {
            let local_addr = SocketAddr::new(*local_ip, 0);
            
            paths.push(NetworkPath {
                path_id: format!("path-{}", idx),
                interface: format!("if{}", idx),
                local_addr,
                remote_addr,
                status: PathStatus::Active,
                metrics: PathMetrics::default(),
            });
        }

        // Update internal paths
        {
            let mut paths_lock = self.paths.write();
            *paths_lock = paths.clone();
        }

        Ok(paths)
    }

    /// Get local IP addresses
    fn get_local_addresses() -> NetworkResult<Vec<IpAddr>> {
        use socket2::{Domain, Socket, Type};
        
        let mut addrs = Vec::new();
        
        // Try to get local addresses by connecting to a remote address
        // This doesn't actually send data, just determines routing
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, None)?;
        socket.set_nonblocking(true)?;
        
        // Try to connect to Google DNS (doesn't actually connect for UDP)
        let google_dns: SocketAddr = "8.8.8.8:80".parse().unwrap();
        let _ = socket.connect(&google_dns.into());
        
        if let Ok(local_addr) = socket.local_addr() {
            if let Some(addr) = local_addr.as_socket() {
                addrs.push(addr.ip());
            }
        }
        
        // Add localhost as fallback
        if addrs.is_empty() {
            addrs.push(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
        }

        Ok(addrs)
    }

    /// Select best path based on priority
    pub fn select_path(&self, priority: Priority) -> Option<NetworkPath> {
        let paths = self.paths.read();
        
        let active_paths: Vec<_> = paths
            .iter()
            .filter(|p| matches!(p.status, PathStatus::Active))
            .collect();

        if active_paths.is_empty() {
            return None;
        }

        match priority {
            Priority::Critical => {
                // Use lowest latency path
                active_paths
                    .iter()
                    .min_by_key(|p| p.metrics.rtt_ms)
                    .map(|p| (*p).clone())
            }
            Priority::High | Priority::Normal => {
                // Use highest bandwidth path
                active_paths
                    .iter()
                    .max_by_key(|p| p.metrics.bandwidth_bps)
                    .map(|p| (*p).clone())
            }
        }
    }

    /// Send chunks across multiple paths
    pub async fn send_multipath(
        &self,
        chunks: Vec<Chunk>,
        remote_addr: SocketAddr,
    ) -> NetworkResult<()> {
        use futures::stream::{self, StreamExt};

        stream::iter(chunks)
            .map(|chunk| {
                let transport = self.transport.clone();
                let manager = self.clone();
                async move {
                    // Select path based on chunk priority
                    if let Some(_path) = manager.select_path(chunk.metadata.priority) {
                        // Connect and send
                        let conn = transport.connect(remote_addr).await?;
                        transport.send_with_retry(&conn, &chunk, 3).await
                    } else {
                        Err(NetworkError::NoPathAvailable)
                    }
                }
            })
            .buffer_unordered(10)
            .collect::<Vec<_>>()
            .await;

        Ok(())
    }

    /// Measure RTT to remote address
    pub async fn measure_rtt(&self, remote_addr: SocketAddr) -> NetworkResult<u64> {
        let start = std::time::Instant::now();
        
        // Try to connect (this measures connection establishment time)
        let conn = self.transport.connect(remote_addr).await?;
        
        let rtt = start.elapsed().as_millis() as u64;
        
        // Close connection
        conn.close(0u32.into(), b"rtt measurement");
        
        Ok(rtt)
    }

    /// Update path metrics
    pub async fn update_path_metrics(&self, path_id: &str, metrics: PathMetrics) {
        let mut paths = self.paths.write();
        
        if let Some(path) = paths.iter_mut().find(|p| p.path_id == path_id) {
            // Update status based on metrics
            if metrics.loss_rate > 0.5 {
                path.status = PathStatus::Failed;
            } else if metrics.loss_rate > 0.2 {
                path.status = PathStatus::Degraded;
            } else {
                path.status = PathStatus::Active;
            }
            
            path.metrics = metrics;
        }
    }

    /// Monitor paths health (background task)
    pub async fn monitor_paths(&self, remote_addr: SocketAddr) {
        loop {
            let path_ids: Vec<String> = {
                let paths = self.paths.read();
                paths.iter().map(|p| p.path_id.clone()).collect()
            };

            for path_id in path_ids {
                // Measure RTT
                if let Ok(rtt) = self.measure_rtt(remote_addr).await {
                    let metrics = PathMetrics {
                        rtt_ms: rtt,
                        loss_rate: 0.0, // Would need actual packet loss measurement
                        bandwidth_bps: 1_000_000, // Would need bandwidth test
                        last_updated: chrono::Utc::now().timestamp(),
                    };
                    
                    self.update_path_metrics(&path_id, metrics).await;
                }
            }

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    /// Get all paths
    pub fn get_paths(&self) -> Vec<NetworkPath> {
        self.paths.read().clone()
    }

    /// Get path count
    pub fn path_count(&self) -> usize {
        self.paths.read().len()
    }

    /// Get active path count
    pub fn active_path_count(&self) -> usize {
        self.paths
            .read()
            .iter()
            .filter(|p| matches!(p.status, PathStatus::Active))
            .count()
    }
}

impl Clone for MultiPathManager {
    fn clone(&self) -> Self {
        Self {
            paths: self.paths.clone(),
            transport: self.transport.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::types::ConnectionConfig;

    #[tokio::test]
    async fn test_multipath_creation() {
        let config = ConnectionConfig::default();
        let transport = Arc::new(QuicTransport::new(config).await.unwrap());
        let manager = MultiPathManager::new(transport);
        
        assert_eq!(manager.path_count(), 0);
    }

    #[tokio::test]
    async fn test_discover_paths() {
        let config = ConnectionConfig::default();
        let transport = Arc::new(QuicTransport::new(config).await.unwrap());
        let manager = MultiPathManager::new(transport);
        
        let remote_addr = "127.0.0.1:8080".parse().unwrap();
        let paths = manager.discover_paths(remote_addr).await.unwrap();
        
        assert!(!paths.is_empty());
        assert_eq!(manager.path_count(), paths.len());
    }

    #[tokio::test]
    async fn test_select_path() {
        let config = ConnectionConfig::default();
        let transport = Arc::new(QuicTransport::new(config).await.unwrap());
        let manager = MultiPathManager::new(transport);
        
        let remote_addr = "127.0.0.1:8080".parse().unwrap();
        let _ = manager.discover_paths(remote_addr).await;
        
        // Should select a path for critical priority
        let path = manager.select_path(Priority::Critical);
        if manager.path_count() > 0 {
            assert!(path.is_some());
        }
    }

    #[tokio::test]
    async fn test_get_local_addresses() {
        let addrs = MultiPathManager::get_local_addresses().unwrap();
        assert!(!addrs.is_empty());
    }

    #[tokio::test]
    async fn test_update_metrics() {
        let config = ConnectionConfig::default();
        let transport = Arc::new(QuicTransport::new(config).await.unwrap());
        let manager = MultiPathManager::new(transport);
        
        let remote_addr = "127.0.0.1:8080".parse().unwrap();
        let paths = manager.discover_paths(remote_addr).await.unwrap();
        
        if let Some(path) = paths.first() {
            let new_metrics = PathMetrics {
                rtt_ms: 50,
                loss_rate: 0.1,
                bandwidth_bps: 1_000_000,
                last_updated: chrono::Utc::now().timestamp(),
            };
            
            manager.update_path_metrics(&path.path_id, new_metrics).await;
            
            let updated_paths = manager.get_paths();
            let updated_path = updated_paths.iter().find(|p| p.path_id == path.path_id);
            
            if let Some(p) = updated_path {
                assert_eq!(p.metrics.rtt_ms, 50);
            }
        }
    }

    #[tokio::test]
    async fn test_path_status_update() {
        let config = ConnectionConfig::default();
        let transport = Arc::new(QuicTransport::new(config).await.unwrap());
        let manager = MultiPathManager::new(transport);
        
        let remote_addr = "127.0.0.1:8080".parse().unwrap();
        let paths = manager.discover_paths(remote_addr).await.unwrap();
        
        if let Some(path) = paths.first() {
            // High loss rate should mark path as failed
            let failing_metrics = PathMetrics {
                rtt_ms: 200,
                loss_rate: 0.6,
                bandwidth_bps: 100_000,
                last_updated: chrono::Utc::now().timestamp(),
            };
            
            manager.update_path_metrics(&path.path_id, failing_metrics).await;
            
            let updated_paths = manager.get_paths();
            let updated_path = updated_paths.iter().find(|p| p.path_id == path.path_id);
            
            if let Some(p) = updated_path {
                assert_eq!(p.status, PathStatus::Failed);
            }
        }
    }
}
