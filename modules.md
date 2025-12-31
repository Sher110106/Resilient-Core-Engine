# Smart File Transfer System - Backend Engineering Blueprint

## 📐 1. High-Level Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          API Gateway Layer                               │
│  ┌─────────────────┐  ┌──────────────────┐  ┌────────────────────────┐ │
│  │  REST API       │  │  WebSocket API   │  │  gRPC Service (Agent)  │ │
│  │  (Actix-web)    │  │  (Tungstenite)   │  │  (Tonic)               │ │
│  └────────┬────────┘  └─────────┬────────┘  └──────────┬─────────────┘ │
└───────────┼────────────────────┼──────────────────────┼────────────────┘
            │                    │                       │
            ▼                    ▼                       ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      Orchestrator Service                                │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │  Transfer Coordinator (Tokio async runtime)                       │  │
│  │  • Session Management  • Priority Scheduling  • State Machine     │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│         │                    │                    │                      │
│         ▼                    ▼                    ▼                      │
│  ┌─────────────┐    ┌──────────────┐    ┌─────────────────────┐       │
│  │  Priority   │    │   Session    │    │  Metrics Collector  │       │
│  │  Queue Mgr  │    │   Store      │    │  (Prometheus)       │       │
│  └─────────────┘    └──────────────┘    └─────────────────────┘       │
└─────────────────────────────────────────────────────────────────────────┘
            │                                        │
            ▼                                        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      Transfer Engine Layer                               │
│  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────────┐   │
│  │  Chunk Manager   │  │  Network Engine  │  │  Integrity Module  │   │
│  │  • Split/Merge   │  │  • QUIC/TCP      │  │  • Checksum        │   │
│  │  • Erasure Code  │  │  • Multi-path    │  │  • Verification    │   │
│  └──────────────────┘  └──────────────────┘  └────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
            │                                        │
            ▼                                        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      Storage & Persistence Layer                         │
│  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────────┐   │
│  │  Metadata DB     │  │  Chunk Cache     │  │  File I/O Handler  │   │
│  │  (SQLite)        │  │  (In-memory/LRU) │  │  (Async Tokio FS)  │   │
│  └──────────────────┘  └──────────────────┘  └────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
            │
            ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      Network Intelligence Layer                          │
│  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────────┐   │
│  │  RTT Monitor     │  │  Bandwidth Probe │  │  Path Selector     │   │
│  │  • Latency Track │  │  • Throughput    │  │  • Multi-interface │   │
│  └──────────────────┘  └──────────────────┘  └────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 🧩 2. Modules Breakdown

### **Technology Stack Decision: Rust with Pragmatic Compromises**

**Core Choice: Rust**
- **Why**: Memory safety, fearless concurrency, zero-cost abstractions, excellent for network programming
- **Hackathon Reality Check**: Steeper learning curve, longer compile times, more verbose
- **Mitigation**: Use high-level libraries, avoid complex lifetimes, focus on tokio ecosystem

**Key Libraries:**
```toml
[dependencies]
# Async runtime
tokio = { version = "1.35", features = ["full"] }

# Network protocols
quinn = "0.10"  # QUIC implementation
tokio-tungstenite = "0.21"  # WebSocket
tonic = "0.11"  # gRPC
prost = "0.12"  # Protocol buffers

# Web framework
actix-web = "4.4"
actix-cors = "0.7"

# Erasure coding
reed-solomon-erasure = "6.0"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Database
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-native-tls"] }

# Crypto & hashing
sha2 = "0.10"
blake3 = "1.5"  # Faster than SHA-256

# Utilities
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = "0.4"
bytes = "1.5"
dashmap = "5.5"  # Concurrent HashMap
parking_lot = "0.12"  # Better mutexes

# Monitoring
prometheus = "0.13"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Testing
mockall = "0.12"
rstest = "0.18"
```

---

### **Module 1: Chunk Manager** (`src/chunk/`)

**Responsibility**: File splitting, erasure encoding, chunk assembly

**Data Structures:**
```rust
// src/chunk/types.rs
use bytes::Bytes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub chunk_id: u64,
    pub file_id: String,
    pub sequence_number: u32,
    pub total_chunks: u32,
    pub data_size: usize,
    pub checksum: [u8; 32],  // BLAKE3 hash
    pub is_parity: bool,
    pub priority: Priority,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Critical = 0,
    High = 1,
    Normal = 2,
}

#[derive(Debug)]
pub struct Chunk {
    pub metadata: ChunkMetadata,
    pub data: Bytes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileManifest {
    pub file_id: String,
    pub filename: String,
    pub total_size: u64,
    pub chunk_size: usize,
    pub total_chunks: u32,
    pub data_chunks: u32,
    pub parity_chunks: u32,
    pub priority: Priority,
    pub checksum: [u8; 32],  // File-level checksum
}
```

**Core APIs:**
```rust
// src/chunk/manager.rs
pub struct ChunkManager {
    erasure_coder: ErasureCoder,
    chunk_size: usize,
}

impl ChunkManager {
    /// Split file into chunks with erasure coding
    pub async fn split_file(
        &self,
        file_path: &Path,
        file_id: String,
        priority: Priority,
    ) -> Result<(FileManifest, Vec<Chunk>)> {
        // 1. Read file in streaming fashion
        // 2. Split into chunks (e.g., 256KB each)
        // 3. Calculate chunk checksums
        // 4. Apply Reed-Solomon encoding (e.g., 10+3 scheme)
        // 5. Generate parity chunks
        // 6. Create manifest
        // 7. Return manifest + all chunks (data + parity)
    }

    /// Reconstruct file from chunks (even with missing chunks)
    pub async fn reconstruct_file(
        &self,
        manifest: &FileManifest,
        chunks: Vec<Chunk>,
        output_path: &Path,
    ) -> Result<()> {
        // 1. Validate we have enough chunks (data + parity)
        // 2. Apply Reed-Solomon decoding if chunks missing
        // 3. Assemble chunks in order
        // 4. Verify file-level checksum
        // 5. Write to disk
    }

    /// Adaptive chunk sizing based on network conditions
    pub fn calculate_optimal_chunk_size(&self, rtt_ms: u64, loss_rate: f32) -> usize {
        // Algorithm:
        // - High RTT (>200ms) → smaller chunks (64KB)
        // - High loss (>10%) → smaller chunks
        // - Stable network → larger chunks (1MB)
        match (rtt_ms, loss_rate) {
            (rtt, loss) if rtt > 200 || loss > 0.1 => 64 * 1024,
            (rtt, loss) if rtt > 100 || loss > 0.05 => 256 * 1024,
            _ => 1024 * 1024,
        }
    }
}
```

**Erasure Coding Implementation:**
```rust
// src/chunk/erasure.rs
use reed_solomon_erasure::galois_8::ReedSolomon;

pub struct ErasureCoder {
    data_shards: usize,   // e.g., 10
    parity_shards: usize, // e.g., 3
}

impl ErasureCoder {
    pub fn new(data_shards: usize, parity_shards: usize) -> Result<Self> {
        Ok(Self { data_shards, parity_shards })
    }

    /// Encode chunks with parity
    pub fn encode(&self, data_chunks: Vec<Bytes>) -> Result<Vec<Bytes>> {
        let rs = ReedSolomon::new(self.data_shards, self.parity_shards)?;
        
        // Pad last chunk if needed
        let mut shards = self.prepare_shards(data_chunks);
        rs.encode(&mut shards)?;
        
        Ok(shards.into_iter().map(Bytes::from).collect())
    }

    /// Decode chunks even with missing data
    pub fn decode(&self, chunks: Vec<Option<Bytes>>) -> Result<Vec<Bytes>> {
        let rs = ReedSolomon::new(self.data_shards, self.parity_shards)?;
        
        let mut shards: Vec<Option<Vec<u8>>> = chunks.into_iter()
            .map(|c| c.map(|b| b.to_vec()))
            .collect();
        
        rs.reconstruct(&mut shards)?;
        
        // Return only data shards
        Ok(shards.into_iter()
            .take(self.data_shards)
            .filter_map(|s| s.map(Bytes::from))
            .collect())
    }
}
```

**Testing Strategy:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_split_and_reconstruct() {
        // Create test file (10MB)
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.bin");
        create_random_file(&file_path, 10 * 1024 * 1024).await;

        let manager = ChunkManager::new(256 * 1024, 10, 3);
        
        // Split
        let (manifest, chunks) = manager
            .split_file(&file_path, "test-123".into(), Priority::Normal)
            .await
            .unwrap();
        
        assert_eq!(chunks.len(), manifest.total_chunks as usize);
        
        // Reconstruct
        let output_path = temp_dir.path().join("reconstructed.bin");
        manager.reconstruct_file(&manifest, chunks, &output_path)
            .await
            .unwrap();
        
        // Verify
        assert_files_equal(&file_path, &output_path).await;
    }

    #[tokio::test]
    async fn test_erasure_recovery() {
        // Test reconstruction with 3 missing chunks
        let manager = ChunkManager::new(256 * 1024, 10, 3);
        let (manifest, mut chunks) = /* ... */;
        
        // Remove 3 random data chunks
        chunks.remove(2);
        chunks.remove(5);
        chunks.remove(7);
        
        // Should still reconstruct successfully
        let output_path = /* ... */;
        manager.reconstruct_file(&manifest, chunks, &output_path)
            .await
            .unwrap();
        
        assert_files_equal(/* ... */);
    }

    #[test]
    fn test_adaptive_chunk_sizing() {
        let manager = ChunkManager::new(256 * 1024, 10, 3);
        
        // Good network
        assert_eq!(manager.calculate_optimal_chunk_size(50, 0.01), 1024 * 1024);
        
        // Poor network
        assert_eq!(manager.calculate_optimal_chunk_size(300, 0.15), 64 * 1024);
    }
}
```

---

### **Module 2: Integrity Module** (`src/integrity/`)

**Responsibility**: Checksum calculation, verification, tamper detection

**Data Structures:**
```rust
// src/integrity/types.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityCheck {
    pub checksum_type: ChecksumType,
    pub value: Vec<u8>,
    pub verified_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChecksumType {
    Blake3,
    Sha256,
}
```

**Core APIs:**
```rust
// src/integrity/verifier.rs
use blake3::Hasher;

pub struct IntegrityVerifier;

impl IntegrityVerifier {
    /// Calculate checksum for byte slice
    pub fn calculate_checksum(data: &[u8]) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(data);
        *hasher.finalize().as_bytes()
    }

    /// Calculate checksum for file (streaming)
    pub async fn calculate_file_checksum(path: &Path) -> Result<[u8; 32]> {
        use tokio::io::AsyncReadExt;
        
        let mut file = tokio::fs::File::open(path).await?;
        let mut hasher = Hasher::new();
        let mut buffer = vec![0u8; 8192];
        
        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 { break; }
            hasher.update(&buffer[..n]);
        }
        
        Ok(*hasher.finalize().as_bytes())
    }

    /// Verify chunk integrity
    pub fn verify_chunk(chunk: &Chunk) -> Result<()> {
        let calculated = Self::calculate_checksum(&chunk.data);
        if calculated != chunk.metadata.checksum {
            return Err(IntegrityError::ChecksumMismatch {
                expected: chunk.metadata.checksum,
                actual: calculated,
            });
        }
        Ok(())
    }

    /// Batch verify multiple chunks
    pub async fn verify_chunks_parallel(chunks: &[Chunk]) -> Vec<Result<()>> {
        use futures::stream::{self, StreamExt};
        
        stream::iter(chunks)
            .map(|chunk| async move { Self::verify_chunk(chunk) })
            .buffer_unordered(num_cpus::get())
            .collect()
            .await
    }
}
```

**Testing:**
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_checksum_calculation() {
        let data = b"Hello, World!";
        let checksum = IntegrityVerifier::calculate_checksum(data);
        assert_eq!(checksum.len(), 32);
        
        // Same data should produce same checksum
        let checksum2 = IntegrityVerifier::calculate_checksum(data);
        assert_eq!(checksum, checksum2);
    }

    #[test]
    fn test_chunk_verification_success() {
        let data = Bytes::from_static(b"test data");
        let checksum = IntegrityVerifier::calculate_checksum(&data);
        
        let chunk = Chunk {
            metadata: ChunkMetadata {
                checksum,
                /* ... */
            },
            data,
        };
        
        assert!(IntegrityVerifier::verify_chunk(&chunk).is_ok());
    }

    #[test]
    fn test_chunk_verification_failure() {
        let data = Bytes::from_static(b"test data");
        let wrong_checksum = [0u8; 32];  // Wrong checksum
        
        let chunk = Chunk {
            metadata: ChunkMetadata {
                checksum: wrong_checksum,
                /* ... */
            },
            data,
        };
        
        assert!(IntegrityVerifier::verify_chunk(&chunk).is_err());
    }

    #[tokio::test]
    async fn test_parallel_verification() {
        let chunks = create_test_chunks(100);
        let results = IntegrityVerifier::verify_chunks_parallel(&chunks).await;
        
        assert!(results.iter().all(|r| r.is_ok()));
    }
}
```

---

### **Module 3: Network Engine** (`src/network/`)

**Responsibility**: QUIC/TCP transport, multi-path routing, connection management

**Data Structures:**
```rust
// src/network/types.rs
#[derive(Debug, Clone)]
pub struct NetworkPath {
    pub path_id: String,
    pub interface: String,  // "eth0", "wlan0", "ppp0"
    pub local_addr: SocketAddr,
    pub remote_addr: SocketAddr,
    pub status: PathStatus,
    pub metrics: PathMetrics,
}

#[derive(Debug, Clone)]
pub enum PathStatus {
    Active,
    Degraded,
    Failed,
}

#[derive(Debug, Clone, Default)]
pub struct PathMetrics {
    pub rtt_ms: u64,
    pub loss_rate: f32,
    pub bandwidth_bps: u64,
    pub last_updated: i64,
}

#[derive(Debug)]
pub struct TransferSession {
    pub session_id: String,
    pub file_id: String,
    pub direction: TransferDirection,
    pub paths: Vec<NetworkPath>,
    pub bytes_transferred: u64,
    pub chunks_completed: u32,
    pub status: SessionStatus,
}

#[derive(Debug, Clone)]
pub enum TransferDirection {
    Send,
    Receive,
}

#[derive(Debug, Clone)]
pub enum SessionStatus {
    Initializing,
    Active,
    Paused,
    Completed,
    Failed(String),
}
```

**Core APIs:**
```rust
// src/network/quic_transport.rs
use quinn::{Endpoint, Connection, SendStream, RecvStream};

pub struct QuicTransport {
    endpoint: Endpoint,
    connections: DashMap<String, Connection>,
}

impl QuicTransport {
    pub async fn new(bind_addr: SocketAddr) -> Result<Self> {
        let mut server_config = quinn::ServerConfig::with_single_cert(
            vec![cert],
            key,
        )?;
        
        // Tune for unreliable networks
        let mut transport_config = quinn::TransportConfig::default();
        transport_config
            .max_idle_timeout(Some(Duration::from_secs(60).try_into()?))
            .keep_alive_interval(Some(Duration::from_secs(5)))
            .max_concurrent_bidi_streams(100u32.into());
        
        server_config.transport = Arc::new(transport_config);
        
        let endpoint = Endpoint::server(server_config, bind_addr)?;
        
        Ok(Self {
            endpoint,
            connections: DashMap::new(),
        })
    }

    pub async fn connect(&self, remote_addr: SocketAddr) -> Result<Connection> {
        let conn = self.endpoint.connect(remote_addr, "localhost")?.await?;
        Ok(conn)
    }

    /// Send chunk over QUIC stream
    pub async fn send_chunk(
        &self,
        conn: &Connection,
        chunk: &Chunk,
    ) -> Result<()> {
        let mut send_stream = conn.open_uni().await?;
        
        // Send metadata
        let metadata_bytes = bincode::serialize(&chunk.metadata)?;
        send_stream.write_u32(metadata_bytes.len() as u32).await?;
        send_stream.write_all(&metadata_bytes).await?;
        
        // Send data
        send_stream.write_all(&chunk.data).await?;
        send_stream.finish().await?;
        
        Ok(())
    }

    /// Receive chunk from QUIC stream
    pub async fn receive_chunk(
        &self,
        mut recv_stream: RecvStream,
    ) -> Result<Chunk> {
        // Read metadata
        let metadata_len = recv_stream.read_u32().await?;
        let mut metadata_bytes = vec![0u8; metadata_len as usize];
        recv_stream.read_exact(&mut metadata_bytes).await?;
        let metadata: ChunkMetadata = bincode::deserialize(&metadata_bytes)?;
        
        // Read data
        let mut data = vec![0u8; metadata.data_size];
        recv_stream.read_exact(&mut data).await?;
        
        Ok(Chunk {
            metadata,
            data: Bytes::from(data),
        })
    }

    /// Send with automatic retry on failure
    pub async fn send_with_retry(
        &self,
        conn: &Connection,
        chunk: &Chunk,
        max_retries: u32,
    ) -> Result<()> {
        let mut attempts = 0;
        let mut backoff = Duration::from_millis(100);
        
        loop {
            match self.send_chunk(conn, chunk).await {
                Ok(_) => return Ok(()),
                Err(e) if attempts < max_retries => {
                    tracing::warn!(
                        "Send failed (attempt {}/{}): {}",
                        attempts + 1,
                        max_retries,
                        e
                    );
                    tokio::time::sleep(backoff).await;
                    backoff *= 2;
                    attempts += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
```

**Multi-Path Manager:**
```rust
// src/network/multipath.rs
pub struct MultiPathManager {
    paths: Arc<RwLock<Vec<NetworkPath>>>,
    path_selector: PathSelector,
}

impl MultiPathManager {
    /// Discover available network interfaces
    pub async fn discover_paths(&self) -> Result<Vec<NetworkPath>> {
        use pnet::datalink;
        
        let interfaces = datalink::interfaces();
        let mut paths = Vec::new();
        
        for iface in interfaces {
            if iface.is_up() && !iface.is_loopback() {
                for ip in iface.ips {
                    paths.push(NetworkPath {
                        path_id: format!("{}-{}", iface.name, ip),
                        interface: iface.name.clone(),
                        local_addr: SocketAddr::new(ip.ip(), 0),
                        remote_addr: /* ... */,
                        status: PathStatus::Active,
                        metrics: PathMetrics::default(),
                    });
                }
            }
        }
        
        Ok(paths)
    }

    /// Select best path for chunk transfer
    pub fn select_path(&self, priority: Priority) -> Option<NetworkPath> {
        let paths = self.paths.read().unwrap();
        
        match priority {
            Priority::Critical => {
                // Use lowest latency path
                paths.iter()
                    .filter(|p| matches!(p.status, PathStatus::Active))
                    .min_by_key(|p| p.metrics.rtt_ms)
                    .cloned()
            }
            Priority::High | Priority::Normal => {
                // Use highest bandwidth path
                paths.iter()
                    .filter(|p| matches!(p.status, PathStatus::Active))
                    .max_by_key(|p| p.metrics.bandwidth_bps)
                    .cloned()
            }
        }
    }

    /// Distribute chunks across multiple paths
    pub async fn send_multipath(
        &self,
        chunks: Vec<Chunk>,
        transport: &QuicTransport,
    ) -> Result<()> {
        use futures::stream::{self, StreamExt};
        
        stream::iter(chunks)
            .map(|chunk| {
                let transport = transport.clone();
                let manager = self.clone();
                async move {
                    if let Some(path) = manager.select_path(chunk.metadata.priority) {
                        let conn = transport.connect(path.remote_addr).await?;
                        transport.send_with_retry(&conn, &chunk, 3).await
                    } else {
                        Err(NetworkError::NoPathAvailable)
                    }
                }
            })
            .buffer_unordered(10)  // Parallel transfers
            .collect::<Vec<_>>()
            .await;
        
        Ok(())
    }

    /// Monitor path health and update metrics
    pub async fn monitor_paths(&self) {
        loop {
            let mut paths = self.paths.write().unwrap();
            
            for path in paths.iter_mut() {
                // Ping to measure RTT
                if let Ok(rtt) = self.measure_rtt(&path.remote_addr).await {
                    path.metrics.rtt_ms = rtt;
                }
                
                // Update status based on metrics
                if path.metrics.loss_rate > 0.5 {
                    path.status = PathStatus::Failed;
                } else if path.metrics.loss_rate > 0.2 {
                    path.status = PathStatus::Degraded;
                }
            }
            
            drop(paths);
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}
```

**Testing:**
```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_quic_send_receive() {
        let server = QuicTransport::new("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        
        let server_addr = server.endpoint.local_addr().unwrap();
        
        tokio::spawn(async move {
            let conn = server.endpoint.accept().await.unwrap();
            let stream = conn.accept_uni().await.unwrap();
            let chunk = server.receive_chunk(stream).await.unwrap();
            assert_eq!(chunk.data, b"test data");
        });
        
        let client = QuicTransport::new("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        
        let conn = client.connect(server_addr).await.unwrap();
        let chunk = create_test_chunk(b"test data");
        client.send_chunk(&conn, &chunk).await.unwrap();
    }

    #[tokio::test]
    async fn test_send_with_retry() {
        // Simulate unreliable connection
        let transport = /* ... */;
        let chunk = /* ... */;
        
        // Should succeed after retries
        transport.send_with_retry(&conn, &chunk, 5)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_multipath_discovery() {
        let manager = MultiPathManager::new();
        let paths = manager.discover_paths().await.unwrap();
        
        // Should find at least one interface
        assert!(!paths.is_empty());
    }
}
```

---

### **Module 4: Priority Queue Manager** (`src/priority/`)

**Responsibility**: Chunk prioritization, scheduling, fair bandwidth allocation

**Data Structures:**
```rust
// src/priority/queue.rs
use std::collections::BinaryHeap;

#[derive(Debug)]
pub struct PriorityQueue {
    queues: [Arc<Mutex<BinaryHeap<QueuedChunk>>>; 3],
    stats: Arc<RwLock<QueueStats>>,
}

#[derive(Debug, Eq, PartialEq)]
struct QueuedChunk {
    chunk: Chunk,
    enqueued_at: Instant,
    retry_count: u32,
}

impl Ord for QueuedChunk {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Earlier chunks have higher priority
        other.chunk.metadata.sequence_number
            .cmp(&self.chunk.metadata.sequence_number)
    }
}

#[derive(Debug, Default)]
pub struct QueueStats {
    pub critical_pending: usize,
    pub high_pending: usize,
    pub normal_pending: usize,
    pub total_processed: u64,
    pub avg_wait_time_ms: u64,
}
```

**Core APIs:**
```rust
impl PriorityQueue {
    pub fn new() -> Self {
        Self {
            queues: [
                Arc::new(Mutex::new(BinaryHeap::new())),
                Arc::new(Mutex::new(BinaryHeap::new())),
                Arc::new(Mutex::new(BinaryHeap::new())),
            ],
            stats: Arc::new(RwLock::new(QueueStats::default())),
        }
    }

    /// Enqueue chunk with priority
    pub async fn enqueue(&self, chunk: Chunk) -> Result<()> {
        let priority_idx = chunk.metadata.priority as usize;
        let mut queue = self.queues[priority_idx].lock().await;
        
        queue.push(QueuedChunk {
            chunk,
            enqueued_at: Instant::now(),
            retry_count: 0,
        });
        
        // Update stats
        let mut stats = self.stats.write().await;
        match priority_idx {
            0 => stats.critical_pending += 1,
            1 => stats.high_pending += 1,
            2 => stats.normal_pending += 1,
            _ => unreachable!(),
        }
        
        Ok(())
    }

    /// Dequeue next chunk (priority-ordered)
    pub async fn dequeue(&self) -> Option<Chunk> {
        // Try critical queue first
        for priority_idx in 0..3 {
            let mut queue = self.queues[priority_idx].lock().await;
            if let Some(queued) = queue.pop() {
                // Update stats
                let wait_time = queued.enqueued_at.elapsed().as_millis() as u64;
                let mut stats = self.stats.write().await;
                stats.total_processed += 1;
                stats.avg_wait_time_ms = 
                    (stats.avg_wait_time_ms + wait_time) / 2;
                
                match priority_idx {
                    0 => stats.critical_pending -= 1,
                    1 => stats.high_pending -= 1,
                    2 => stats.normal_pending -= 1,
                    _ => unreachable!(),
                }
                
                return Some(queued.chunk);
            }
        }
        None
    }

    /// Re-enqueue failed chunk with exponential backoff
    pub async fn requeue(&self, mut chunk: Chunk, retry_count: u32) -> Result<()> {
        if retry_count > 5 {
            return Err(QueueError::MaxRetriesExceeded);
        }
        
        // Exponential backoff
        let delay = Duration::from_millis(100 * 2u64.pow(retry_count));
        tokio::time::sleep(delay).await;
        
        self.enqueue(chunk).await
    }

    /// Get queue statistics
    pub async fn stats(&self) -> QueueStats {
        self.stats.read().await.clone()
    }

    /// Fair bandwidth allocation across priorities
    pub async fn allocate_bandwidth(&self, total_bps: u64) -> [u64; 3] {
        let stats = self.stats.read().await;
        
        // Critical: 50%, High: 30%, Normal: 20%
        let critical_bps = total_bps / 2;
        let high_bps = total_bps * 3 / 10;
        let normal_bps = total_bps / 5;
        
        // Redistribute unused bandwidth
        let mut allocation = [critical_bps, high_bps, normal_bps];
        
        if stats.critical_pending == 0 {
            allocation[1] += allocation[0] / 2;
            allocation[2] += allocation[0] / 2;
            allocation[0] = 0;
        }
        
        allocation
    }
}
```

**Testing:**
```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_priority_ordering() {
        let queue = PriorityQueue::new();
        
        // Enqueue in mixed order
        queue.enqueue(create_chunk(Priority::Normal)).await.unwrap();
        queue.enqueue(create_chunk(Priority::Critical)).await.unwrap();
        queue.enqueue(create_chunk(Priority::High)).await.unwrap();
        
        // Should dequeue in priority order
        let chunk1 = queue.dequeue().await.unwrap();
        assert_eq!(chunk1.metadata.priority, Priority::Critical);
        
        let chunk2 = queue.dequeue().await.unwrap();
        assert_eq!(chunk2.metadata.priority, Priority::High);
        
        let chunk3 = queue.dequeue().await.unwrap();
        assert_eq!(chunk3.metadata.priority, Priority::Normal);
    }

    #[tokio::test]
    async fn test_bandwidth_allocation() {
        let queue = PriorityQueue::new();
        
        // Add chunks to all priorities
        queue.enqueue(create_chunk(Priority::Critical)).await.unwrap();
        queue.enqueue(create_chunk(Priority::High)).await.unwrap();
        queue.enqueue(create_chunk(Priority::Normal)).await.unwrap();
        
        let allocation = queue.allocate_bandwidth(1_000_000).await;
        
        // 50%, 30%, 20%
        assert_eq!(allocation[0], 500_000);
        assert_eq!(allocation[1], 300_000);
        assert_eq!(allocation[2], 200_000);
    }

    #[tokio::test]
    async fn test_requeue_with_backoff() {
        let queue = PriorityQueue::new();
        let chunk = create_chunk(Priority::Normal);
        
        let start = Instant::now();
        queue.requeue(chunk, 2).await.unwrap();  // 2^2 * 100ms = 400ms
        let elapsed = start.elapsed();
        
        assert!(elapsed >= Duration::from_millis(400));
    }
}
```

---

### **Module 5: Session Store & State Management** (`src/session/`)

**Responsibility**: Transfer session persistence, resumability, crash recovery

**Data Structures:**
```rust
// src/session/store.rs
use sqlx::{SqlitePool, Row};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub session_id: String,
    pub file_id: String,
    pub manifest: FileManifest,
    pub completed_chunks: HashSet<u32>,
    pub failed_chunks: HashSet<u32>,
    pub status: SessionStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct SessionStore {
    pool: SqlitePool,
}
```

**Core APIs:**
```rust
impl SessionStore {
    pub async fn new(db_path: &str) -> Result<Self> {
        let pool = SqlitePool::connect(db_path).await?;
        
        // Initialize schema
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                file_id TEXT NOT NULL,
                manifest JSON NOT NULL,
                completed_chunks TEXT NOT NULL,
                failed_chunks TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#
        )
        .execute(&pool)
        .await?;
        
        Ok(Self { pool })
    }

    /// Save or update session state
    pub async fn save(&self, state: &SessionState) -> Result<()> {
        let completed = serde_json::to_string(&state.completed_chunks)?;
        let failed = serde_json::to_string(&state.failed_chunks)?;
        let manifest = serde_json::to_string(&state.manifest)?;
        let status = serde_json::to_string(&state.status)?;
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO sessions
            (session_id, file_id, manifest, completed_chunks, failed_chunks, status, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&state.session_id)
        .bind(&state.file_id)
        .bind(manifest)
        .bind(completed)
        .bind(failed)
        .bind(status)
        .bind(state.created_at)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    /// Load session state
    pub async fn load(&self, session_id: &str) -> Result<Option<SessionState>> {
        let row = sqlx::query(
            "SELECT * FROM sessions WHERE session_id = ?"
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row {
            Ok(Some(SessionState {
                session_id: row.try_get("session_id")?,
                file_id: row.try_get("file_id")?,
                manifest: serde_json::from_str(&row.try_get::<String, _>("manifest")?)?,
                completed_chunks: serde_json::from_str(&row.try_get::<String, _>("completed_chunks")?)?,
                failed_chunks: serde_json::from_str(&row.try_get::<String, _>("failed_chunks")?)?,
                status: serde_json::from_str(&row.try_get::<String, _>("status")?)?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Mark chunk as completed
    pub async fn mark_chunk_completed(
        &self,
        session_id: &str,
        chunk_number: u32,
    ) -> Result<()> {
        let mut state = self.load(session_id).await?
            .ok_or(SessionError::NotFound)?;
        
        state.completed_chunks.insert(chunk_number);
        state.failed_chunks.remove(&chunk_number);
        
        self.save(&state).await
    }

    /// Get resume information
    pub async fn get_resume_info(&self, session_id: &str) -> Result<ResumeInfo> {
        let state = self.load(session_id).await?
            .ok_or(SessionError::NotFound)?;
        
        let total_chunks = state.manifest.total_chunks;
        let completed = state.completed_chunks.len() as u32;
        let remaining = total_chunks - completed;
        
        Ok(ResumeInfo {
            total_chunks,
            completed_chunks: completed,
            remaining_chunks: remaining,
            progress_percent: (completed as f32 / total_chunks as f32) * 100.0,
            can_resume: matches!(state.status, SessionStatus::Paused | SessionStatus::Failed(_)),
        })
    }

    /// Clean up old sessions
    pub async fn cleanup_old_sessions(&self, days: i64) -> Result<u64> {
        let cutoff = chrono::Utc::now().timestamp() - (days * 86400);
        
        let result = sqlx::query(
            "DELETE FROM sessions WHERE updated_at < ? AND status IN ('Completed', 'Failed')"
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await?;
        
        Ok(result.rows_affected())
    }
}

#[derive(Debug, Serialize)]
pub struct ResumeInfo {
    pub total_chunks: u32,
    pub completed_chunks: u32,
    pub remaining_chunks: u32,
    pub progress_percent: f32,
    pub can_resume: bool,
}
```

**Testing:**
```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_session_persistence() {
        let store = SessionStore::new(":memory:").await.unwrap();
        
        let state = SessionState {
            session_id: "test-session".into(),
            file_id: "test-file".into(),
            manifest: create_test_manifest(),
            completed_chunks: HashSet::from([0, 1, 2]),
            failed_chunks: HashSet::new(),
            status: SessionStatus::Active,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };
        
        store.save(&state).await.unwrap();
        
        let loaded = store.load("test-session").await.unwrap().unwrap();
        assert_eq!(loaded.session_id, state.session_id);
        assert_eq!(loaded.completed_chunks.len(), 3);
    }

    #[tokio::test]
    async fn test_resume_after_failure() {
        let store = SessionStore::new(":memory:").await.unwrap();
        
        // Simulate partial transfer
        let mut state = create_test_session();
        state.completed_chunks = HashSet::from([0, 1, 2, 3, 4]);
        state.status = SessionStatus::Failed("Connection lost".into());
        store.save(&state).await.unwrap();
        
        // Get resume info
        let resume_info = store.get_resume_info(&state.session_id).await.unwrap();
        
        assert_eq!(resume_info.completed_chunks, 5);
        assert!(resume_info.can_resume);
    }
}
```

---

### **Module 6: Transfer Coordinator (Orchestrator)** (`src/coordinator/`)

**Responsibility**: High-level transfer orchestration, state machine, event handling

**State Machine:**
```rust
// src/coordinator/state_machine.rs
#[derive(Debug, Clone)]
pub enum TransferState {
    Idle,
    Preparing,
    Transferring { progress: f32 },
    Paused { reason: String },
    Completing,
    Completed,
    Failed { error: String },
}

pub struct TransferStateMachine {
    state: Arc<RwLock<TransferState>>,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
}

#[derive(Debug, Clone)]
pub enum TransferEvent {
    Start { file_path: PathBuf, priority: Priority },
    ChunkCompleted { chunk_number: u32 },
    ChunkFailed { chunk_number: u32, error: String },
    Pause,
    Resume,
    Cancel,
    NetworkFailure { path_id: String },
    NetworkRecovered { path_id: String },
}

impl TransferStateMachine {
    pub async fn transition(&self, event: TransferEvent) -> Result<()> {
        let mut state = self.state.write().await;
        
        *state = match (&*state, event) {
            (TransferState::Idle, TransferEvent::Start { .. }) => {
                TransferState::Preparing
            }
            (TransferState::Preparing, TransferEvent::ChunkCompleted { .. }) => {
                TransferState::Transferring { progress: 0.0 }
            }
            (TransferState::Transferring { .. }, TransferEvent::ChunkCompleted { .. }) => {
                // Update progress
                TransferState::Transferring { progress: /* calculate */ }
            }
            (TransferState::Transferring { .. }, TransferEvent::Pause) => {
                TransferState::Paused { reason: "User requested".into() }
            }
            (TransferState::Paused { .. }, TransferEvent::Resume) => {
                TransferState::Transferring { progress: /* restore */ }
            }
            // ... other transitions
            _ => return Err(StateError::InvalidTransition),
        };
        
        Ok(())
    }
}
```

**Core Coordinator:**
```rust
// src/coordinator/mod.rs
pub struct TransferCoordinator {
    chunk_manager: Arc<ChunkManager>,
    network_engine: Arc<QuicTransport>,
    priority_queue: Arc<PriorityQueue>,
    session_store: Arc<SessionStore>,
    multipath_manager: Arc<MultiPathManager>,
    state_machine: Arc<TransferStateMachine>,
}

impl TransferCoordinator {
    /// Initiate file transfer
    pub async fn send_file(
        &self,
        file_path: PathBuf,
        remote_addr: SocketAddr,
        priority: Priority,
    ) -> Result<String> {
        // 1. Create session
        let session_id = Uuid::new_v4().to_string();
        let file_id = Uuid::new_v4().to_string();
        
        // 2. Split file into chunks
        let (manifest, chunks) = self.chunk_manager
            .split_file(&file_path, file_id.clone(), priority)
            .await?;
        
        // 3. Save session state
        let state = SessionState {
            session_id: session_id.clone(),
            file_id,
            manifest: manifest.clone(),
            completed_chunks: HashSet::new(),
            failed_chunks: HashSet::new(),
            status: SessionStatus::Initializing,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };
        self.session_store.save(&state).await?;
        
        // 4. Enqueue chunks
        for chunk in chunks {
            self.priority_queue.enqueue(chunk).await?;
        }
        
        // 5. Start transfer worker
        tokio::spawn({
            let coordinator = self.clone();
            let session_id = session_id.clone();
            async move {
                coordinator.transfer_worker(session_id, remote_addr).await;
            }
        });
        
        Ok(session_id)
    }

    /// Transfer worker (runs async per session)
    async fn transfer_worker(
        &self,
        session_id: String,
        remote_addr: SocketAddr,
    ) {
        let conn = match self.network_engine.connect(remote_addr).await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to connect: {}", e);
                return;
            }
        };
        
        // Process chunks from queue
        loop {
            let chunk = match self.priority_queue.dequeue().await {
                Some(c) => c,
                None => {
                    // Check if transfer complete
                    if self.is_transfer_complete(&session_id).await {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };
            
            // Send chunk
            match self.network_engine.send_with_retry(&conn, &chunk, 3).await {
                Ok(_) => {
                    self.session_store
                        .mark_chunk_completed(&session_id, chunk.metadata.sequence_number)
                        .await
                        .ok();
                    
                    self.state_machine
                        .transition(TransferEvent::ChunkCompleted {
                            chunk_number: chunk.metadata.sequence_number,
                        })
                        .await
                        .ok();
                }
                Err(e) => {
                    tracing::error!("Failed to send chunk: {}", e);
                    self.priority_queue.requeue(chunk, 1).await.ok();
                }
            }
        }
        
        // Mark session complete
        let mut state = self.session_store.load(&session_id).await.unwrap().unwrap();
        state.status = SessionStatus::Completed;
        self.session_store.save(&state).await.ok();
    }

    /// Resume interrupted transfer
    pub async fn resume_transfer(&self, session_id: String) -> Result<()> {
        let state = self.session_store.load(&session_id).await?
            ```rust
            .ok_or(SessionError::NotFound)?;
        
        // Validate can resume
        if !matches!(state.status, SessionStatus::Paused | SessionStatus::Failed(_)) {
            return Err(CoordinatorError::CannotResume);
        }
        
        // Calculate remaining chunks
        let remaining_chunks: Vec<u32> = (0..state.manifest.total_chunks)
            .filter(|n| !state.completed_chunks.contains(n))
            .collect();
        
        tracing::info!(
            "Resuming transfer {} with {} remaining chunks",
            session_id,
            remaining_chunks.len()
        );
        
        // Re-split file and enqueue only remaining chunks
        let (_, all_chunks) = self.chunk_manager
            .split_file(
                &PathBuf::from(&state.manifest.filename),
                state.file_id.clone(),
                state.manifest.priority,
            )
            .await?;
        
        for chunk in all_chunks {
            if remaining_chunks.contains(&chunk.metadata.sequence_number) {
                self.priority_queue.enqueue(chunk).await?;
            }
        }
        
        // Update state to active
        let mut state = state;
        state.status = SessionStatus::Active;
        self.session_store.save(&state).await?;
        
        Ok(())
    }

    /// Pause active transfer
    pub async fn pause_transfer(&self, session_id: String) -> Result<()> {
        let mut state = self.session_store.load(&session_id).await?
            .ok_or(SessionError::NotFound)?;
        
        state.status = SessionStatus::Paused;
        self.session_store.save(&state).await?;
        
        self.state_machine
            .transition(TransferEvent::Pause)
            .await?;
        
        Ok(())
    }

    /// Check if transfer is complete
    async fn is_transfer_complete(&self, session_id: &str) -> bool {
        if let Ok(Some(state)) = self.session_store.load(session_id).await {
            state.completed_chunks.len() as u32 == state.manifest.data_chunks
        } else {
            false
        }
    }

    /// Get transfer progress
    pub async fn get_progress(&self, session_id: &str) -> Result<TransferProgress> {
        let state = self.session_store.load(session_id).await?
            .ok_or(SessionError::NotFound)?;
        
        let completed = state.completed_chunks.len() as u32;
        let total = state.manifest.total_chunks;
        let bytes_completed = completed as u64 * state.manifest.chunk_size as u64;
        let bytes_total = state.manifest.total_size;
        
        Ok(TransferProgress {
            session_id: session_id.to_string(),
            completed_chunks: completed,
            total_chunks: total,
            bytes_transferred: bytes_completed,
            total_bytes: bytes_total,
            progress_percent: (completed as f32 / total as f32) * 100.0,
            status: state.status,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct TransferProgress {
    pub session_id: String,
    pub completed_chunks: u32,
    pub total_chunks: u32,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub progress_percent: f32,
    pub status: SessionStatus,
}
```

**Testing:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_complete_transfer_flow() {
        let coordinator = setup_test_coordinator().await;
        let test_file = create_test_file(1024 * 1024); // 1MB
        
        // Start transfer
        let session_id = coordinator
            .send_file(
                test_file.clone(),
                "127.0.0.1:8000".parse().unwrap(),
                Priority::Normal,
            )
            .await
            .unwrap();
        
        // Wait for completion
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Check progress
        let progress = coordinator.get_progress(&session_id).await.unwrap();
        assert_eq!(progress.progress_percent, 100.0);
        assert!(matches!(progress.status, SessionStatus::Completed));
    }

    #[tokio::test]
    async fn test_pause_and_resume() {
        let coordinator = setup_test_coordinator().await;
        let test_file = create_test_file(10 * 1024 * 1024); // 10MB
        
        // Start transfer
        let session_id = coordinator
            .send_file(test_file, "127.0.0.1:8000".parse().unwrap(), Priority::Normal)
            .await
            .unwrap();
        
        // Wait a bit
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Pause
        coordinator.pause_transfer(session_id.clone()).await.unwrap();
        
        let progress_paused = coordinator.get_progress(&session_id).await.unwrap();
        let chunks_at_pause = progress_paused.completed_chunks;
        
        // Wait (should not progress)
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        let progress_still_paused = coordinator.get_progress(&session_id).await.unwrap();
        assert_eq!(progress_still_paused.completed_chunks, chunks_at_pause);
        
        // Resume
        coordinator.resume_transfer(session_id.clone()).await.unwrap();
        
        // Wait for completion
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        let progress_final = coordinator.get_progress(&session_id).await.unwrap();
        assert!(progress_final.completed_chunks > chunks_at_pause);
    }

    #[tokio::test]
    async fn test_network_failure_recovery() {
        let coordinator = setup_test_coordinator().await;
        let test_file = create_test_file(5 * 1024 * 1024);
        
        let session_id = coordinator
            .send_file(test_file, "127.0.0.1:8000".parse().unwrap(), Priority::Normal)
            .await
            .unwrap();
        
        // Simulate network failure after some chunks
        tokio::time::sleep(Duration::from_millis(500)).await;
        coordinator.state_machine
            .transition(TransferEvent::NetworkFailure { path_id: "test".into() })
            .await
            .unwrap();
        
        // Simulate recovery
        tokio::time::sleep(Duration::from_secs(1)).await;
        coordinator.state_machine
            .transition(TransferEvent::NetworkRecovered { path_id: "test".into() })
            .await
            .unwrap();
        
        // Should complete eventually
        tokio::time::sleep(Duration::from_secs(5)).await;
        let progress = coordinator.get_progress(&session_id).await.unwrap();
        assert!(matches!(progress.status, SessionStatus::Completed));
    }
}
```

---

### **Module 7: API Layer** (`src/api/`)

**Responsibility**: REST API, WebSocket for real-time updates, gRPC for agent communication

**REST API:**
```rust
// src/api/rest.rs
use actix_web::{web, App, HttpResponse, HttpServer};
use actix_cors::Cors;

pub struct ApiState {
    pub coordinator: Arc<TransferCoordinator>,
    pub ws_broadcaster: Arc<WsBroadcaster>,
}

#[derive(Debug, Deserialize)]
pub struct SendFileRequest {
    pub file_path: String,
    pub remote_addr: String,
    pub priority: Priority,
}

#[derive(Debug, Serialize)]
pub struct SendFileResponse {
    pub session_id: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct SessionIdParam {
    pub session_id: String,
}

// POST /api/transfer/send
async fn send_file(
    state: web::Data<ApiState>,
    req: web::Json<SendFileRequest>,
) -> Result<HttpResponse, ApiError> {
    let session_id = state
        .coordinator
        .send_file(
            PathBuf::from(&req.file_path),
            req.remote_addr.parse()?,
            req.priority,
        )
        .await?;
    
    Ok(HttpResponse::Ok().json(SendFileResponse {
        session_id: session_id.clone(),
        message: format!("Transfer initiated: {}", session_id),
    }))
}

// GET /api/transfer/{session_id}/progress
async fn get_progress(
    state: web::Data<ApiState>,
    path: web::Path<SessionIdParam>,
) -> Result<HttpResponse, ApiError> {
    let progress = state
        .coordinator
        .get_progress(&path.session_id)
        .await?;
    
    Ok(HttpResponse::Ok().json(progress))
}

// POST /api/transfer/{session_id}/pause
async fn pause_transfer(
    state: web::Data<ApiState>,
    path: web::Path<SessionIdParam>,
) -> Result<HttpResponse, ApiError> {
    state
        .coordinator
        .pause_transfer(path.session_id.clone())
        .await?;
    
    Ok(HttpResponse::Ok().json(json!({
        "message": "Transfer paused"
    })))
}

// POST /api/transfer/{session_id}/resume
async fn resume_transfer(
    state: web::Data<ApiState>,
    path: web::Path<SessionIdParam>,
) -> Result<HttpResponse, ApiError> {
    state
        .coordinator
        .resume_transfer(path.session_id.clone())
        .await?;
    
    Ok(HttpResponse::Ok().json(json!({
        "message": "Transfer resumed"
    })))
}

// GET /api/sessions
async fn list_sessions(
    state: web::Data<ApiState>,
) -> Result<HttpResponse, ApiError> {
    let sessions = state
        .coordinator
        .session_store
        .list_all()
        .await?;
    
    Ok(HttpResponse::Ok().json(sessions))
}

// GET /api/metrics
async fn get_metrics(
    state: web::Data<ApiState>,
) -> Result<HttpResponse, ApiError> {
    let queue_stats = state.coordinator.priority_queue.stats().await;
    
    let metrics = json!({
        "queue": queue_stats,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    
    Ok(HttpResponse::Ok().json(metrics))
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .route("/transfer/send", web::post().to(send_file))
            .route("/transfer/{session_id}/progress", web::get().to(get_progress))
            .route("/transfer/{session_id}/pause", web::post().to(pause_transfer))
            .route("/transfer/{session_id}/resume", web::post().to(resume_transfer))
            .route("/sessions", web::get().to(list_sessions))
            .route("/metrics", web::get().to(get_metrics))
    );
}

pub async fn start_api_server(
    coordinator: Arc<TransferCoordinator>,
    bind_addr: &str,
) -> Result<()> {
    let ws_broadcaster = Arc::new(WsBroadcaster::new());
    
    let state = web::Data::new(ApiState {
        coordinator,
        ws_broadcaster,
    });
    
    HttpServer::new(move || {
        let cors = Cors::permissive();
        
        App::new()
            .wrap(cors)
            .app_data(state.clone())
            .configure(configure_routes)
    })
    .bind(bind_addr)?
    .run()
    .await?;
    
    Ok(())
}
```

**WebSocket for Real-Time Updates:**
```rust
// src/api/websocket.rs
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use tokio::sync::broadcast;

pub struct WsBroadcaster {
    tx: broadcast::Sender<WsMessage>,
}

impl WsBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1000);
        Self { tx }
    }
    
    pub fn broadcast(&self, msg: WsMessage) {
        let _ = self.tx.send(msg);
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<WsMessage> {
        self.tx.subscribe()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WsMessage {
    TransferProgress {
        session_id: String,
        progress: TransferProgress,
    },
    ChunkCompleted {
        session_id: String,
        chunk_number: u32,
    },
    TransferCompleted {
        session_id: String,
    },
    NetworkEvent {
        event_type: String,
        details: serde_json::Value,
    },
}

struct WsConnection {
    rx: broadcast::Receiver<WsMessage>,
}

impl Actor for WsConnection {
    type Context = ws::WebsocketContext<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        let rx = self.rx.resubscribe();
        ctx.run_interval(Duration::from_millis(100), move |act, ctx| {
            while let Ok(msg) = rx.try_recv() {
                let json = serde_json::to_string(&msg).unwrap();
                ctx.text(json);
            }
        });
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(_)) => {
                // Handle client messages if needed
            }
            Ok(ws::Message::Close(_)) => ctx.stop(),
            _ => (),
        }
    }
}

// GET /ws
pub async fn websocket_handler(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<ApiState>,
) -> Result<HttpResponse, actix_web::Error> {
    let rx = state.ws_broadcaster.subscribe();
    ws::start(WsConnection { rx }, &req, stream)
}
```

**gRPC Service for Agent Communication:**
```rust
// src/api/grpc.rs
// proto/transfer.proto needs to be compiled first

tonic::include_proto!("transfer");

pub struct TransferService {
    coordinator: Arc<TransferCoordinator>,
}

#[tonic::async_trait]
impl transfer_service_server::TransferService for TransferService {
    async fn send_chunk(
        &self,
        request: tonic::Request<ChunkRequest>,
    ) -> Result<tonic::Response<ChunkResponse>, tonic::Status> {
        let req = request.into_inner();
        
        // Reconstruct chunk from protobuf
        let chunk = Chunk {
            metadata: ChunkMetadata {
                chunk_id: req.chunk_id,
                file_id: req.file_id,
                sequence_number: req.sequence_number,
                total_chunks: req.total_chunks,
                data_size: req.data.len(),
                checksum: req.checksum.try_into().unwrap(),
                is_parity: req.is_parity,
                priority: Priority::from_i32(req.priority).unwrap(),
                created_at: req.created_at,
            },
            data: Bytes::from(req.data),
        };
        
        // Verify integrity
        IntegrityVerifier::verify_chunk(&chunk)
            .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?;
        
        // Enqueue for processing
        self.coordinator
            .priority_queue
            .enqueue(chunk)
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        
        Ok(tonic::Response::new(ChunkResponse {
            success: true,
            message: "Chunk received".into(),
        }))
    }
    
    async fn get_transfer_status(
        &self,
        request: tonic::Request<StatusRequest>,
    ) -> Result<tonic::Response<StatusResponse>, tonic::Status> {
        let session_id = request.into_inner().session_id;
        
        let progress = self.coordinator
            .get_progress(&session_id)
            .await
            .map_err(|e| tonic::Status::not_found(e.to_string()))?;
        
        Ok(tonic::Response::new(StatusResponse {
            session_id,
            completed_chunks: progress.completed_chunks,
            total_chunks: progress.total_chunks,
            bytes_transferred: progress.bytes_transferred,
            total_bytes: progress.total_bytes,
            progress_percent: progress.progress_percent,
        }))
    }
}

pub async fn start_grpc_server(
    coordinator: Arc<TransferCoordinator>,
    bind_addr: &str,
) -> Result<()> {
    let service = TransferService { coordinator };
    
    tonic::transport::Server::builder()
        .add_service(transfer_service_server::TransferServiceServer::new(service))
        .serve(bind_addr.parse()?)
        .await?;
    
    Ok(())
}
```

**Proto Definition:**
```protobuf
// proto/transfer.proto
syntax = "proto3";

package transfer;

service TransferService {
  rpc SendChunk(ChunkRequest) returns (ChunkResponse);
  rpc GetTransferStatus(StatusRequest) returns (StatusResponse);
}

message ChunkRequest {
  uint64 chunk_id = 1;
  string file_id = 2;
  uint32 sequence_number = 3;
  uint32 total_chunks = 4;
  bytes data = 5;
  bytes checksum = 6;
  bool is_parity = 7;
  int32 priority = 8;
  int64 created_at = 9;
}

message ChunkResponse {
  bool success = 1;
  string message = 2;
}

message StatusRequest {
  string session_id = 1;
}

message StatusResponse {
  string session_id = 1;
  uint32 completed_chunks = 2;
  uint32 total_chunks = 3;
  uint64 bytes_transferred = 4;
  uint64 total_bytes = 5;
  float progress_percent = 6;
}
```

---

### **Module 8: Metrics & Monitoring** (`src/metrics/`)

**Responsibility**: Performance tracking, Prometheus metrics, logging

```rust
// src/metrics/mod.rs
use prometheus::{
    Registry, Counter, Gauge, Histogram, HistogramOpts,
    register_counter_with_registry,
    register_gauge_with_registry,
    register_histogram_with_registry,
};

pub struct MetricsCollector {
    registry: Registry,
    
    // Counters
    chunks_sent: Counter,
    chunks_received: Counter,
    chunks_failed: Counter,
    bytes_transferred: Counter,
    
    // Gauges
    active_sessions: Gauge,
    queue_size: Gauge,
    network_bandwidth: Gauge,
    
    // Histograms
    chunk_transfer_duration: Histogram,
    chunk_size: Histogram,
}

impl MetricsCollector {
    pub fn new() -> Result<Self> {
        let registry = Registry::new();
        
        Ok(Self {
            chunks_sent: register_counter_with_registry!(
                "chunks_sent_total",
                "Total chunks sent",
                registry
            )?,
            chunks_received: register_counter_with_registry!(
                "chunks_received_total",
                "Total chunks received",
                registry
            )?,
            chunks_failed: register_counter_with_registry!(
                "chunks_failed_total",
                "Total chunks failed",
                registry
            )?,
            bytes_transferred: register_counter_with_registry!(
                "bytes_transferred_total",
                "Total bytes transferred",
                registry
            )?,
            active_sessions: register_gauge_with_registry!(
                "active_sessions",
                "Number of active transfer sessions",
                registry
            )?,
            queue_size: register_gauge_with_registry!(
                "priority_queue_size",
                "Number of chunks in priority queue",
                registry
            )?,
            network_bandwidth: register_gauge_with_registry!(
                "network_bandwidth_bps",
                "Current network bandwidth in bits per second",
                registry
            )?,
            chunk_transfer_duration: register_histogram_with_registry!(
                HistogramOpts::new(
                    "chunk_transfer_duration_seconds",
                    "Time to transfer a chunk"
                ),
                registry
            )?,
            chunk_size: register_histogram_with_registry!(
                HistogramOpts::new(
                    "chunk_size_bytes",
                    "Size of chunks in bytes"
                ).buckets(vec![
                    64.0 * 1024.0,
                    256.0 * 1024.0,
                    512.0 * 1024.0,
                    1024.0 * 1024.0,
                ]),
                registry
            )?,
            registry,
        })
    }
    
    pub fn record_chunk_sent(&self, size: usize, duration: Duration) {
        self.chunks_sent.inc();
        self.bytes_transferred.inc_by(size as u64);
        self.chunk_transfer_duration.observe(duration.as_secs_f64());
        self.chunk_size.observe(size as f64);
    }
    
    pub fn record_chunk_failed(&self) {
        self.chunks_failed.inc();
    }
    
    pub fn update_queue_size(&self, size: usize) {
        self.queue_size.set(size as f64);
    }
    
    pub fn update_bandwidth(&self, bps: u64) {
        self.network_bandwidth.set(bps as f64);
    }
    
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}

// Prometheus endpoint
pub async fn metrics_handler(
    metrics: web::Data<Arc<MetricsCollector>>,
) -> Result<HttpResponse, actix_web::Error> {
    use prometheus::Encoder;
    
    let encoder = prometheus::TextEncoder::new();
    let metric_families = metrics.registry().gather();
    
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    
    Ok(HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(buffer))
}
```

---

## 📋 3. Implementation Plan (Development Order)

### **Phase 1: Foundation (Hours 0-8)**

**Priority 1: Core Data Structures & Integrity Module**
```bash
Day 1, Hours 0-2
├── Create project structure
├── Define all data types (chunk, manifest, session)
├── Implement IntegrityVerifier
└── Write unit tests for checksums
```

**Priority 2: Chunk Manager (Without Erasure Coding)**
```bash
Day 1, Hours 2-5
├── Implement file splitting
├── Implement chunk assembly
├── Add checksum calculation per chunk
└── Test with various file sizes
```

**Priority 3: Storage Layer**
```bash
Day 1, Hours 5-8
├── Setup SQLite with sqlx
├── Implement SessionStore
├── Add session persistence
└── Test save/load operations
```

---

### **Phase 2: Network & Transfer (Hours 8-18)**

**Priority 4: QUIC Transport**
```bash
Day 1, Hours 8-12
├── Setup Quinn QUIC endpoint
├── Implement send_chunk / receive_chunk
├── Add connection management
├── Test local transfers
└── Add retry logic
```

**Priority 5: Priority Queue**
```bash
Day 1, Hours 12-14
├── Implement 3-tier priority queue
├── Add enqueue/dequeue logic
├── Implement fair bandwidth allocation
└── Test priority ordering
```

**Priority 6: Transfer Coordinator**
```bash
Day 1, Hours 14-18
├── Implement send_file workflow
├── Add transfer worker
├── Implement pause/resume
├── Connect all modules
└── End-to-end test
```

---

### **Phase 3: Advanced Features (Hours 18-28)**

**Priority 7: Erasure Coding**
```bash
Day 2, Hours 18-22
├── Integrate reed-solomon-erasure
├── Add encoding to chunk split
├── Add decoding to reconstruction
├── Test with missing chunks (KEY DEMO FEATURE)
```

**Priority 8: Multi-Path Support**
```bash
Day 2, Hours 22-26
├── Implement path discovery
├── Add path selection logic
├── Implement parallel sending
└── Test failover scenarios
```

**Priority 9: Network Intelligence**
```bash
Day 2, Hours 26-28
├── Add RTT monitoring
├── Implement adaptive chunk sizing
├── Add bandwidth measurement
```

---

### **Phase 4: API & Integration (Hours 28-36)**

**Priority 10: REST API**
```bash
Day 2, Hours 28-32
├── Setup Actix-web server
├── Implement all endpoints
├── Add CORS support
└── API testing with curl/Postman
```

**Priority 11: WebSocket**
```bash
Day 2, Hours 32-34
├── Implement WsBroadcaster
├── Add real-time progress updates
└── Test with websocket client
```

**Priority 12: Metrics**
```bash
Day 2, Hours 34-36
├── Setup Prometheus metrics
├── Add metric collection points
├── Expose /metrics endpoint
```

---

### **Phase 5: Demo Preparation (Hours 36-40)**

**Priority 13: Testing & Polish**
```bash
Day 2, Hours 36-38
├── Integration test suite
├── Network simulation tests
├── Load testing
└── Bug fixes
```

**Priority 14: Demo Setup**
```bash
Day 2, Hours 38-40
├── Prepare demo scenarios
├── Setup network simulation
├── Create sample files
└── Document API usage
```

---

## 🧪 4. Testing Strategy

### **Unit Tests (Per Module)**

```rust
// tests/unit/chunk_manager_test.rs
#[cfg(test)]
mod chunk_manager_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_split_small_file() {
        // Test file smaller than chunk size
    }
    
    #[tokio::test]
    async fn test_split_exact_chunks() {
        // Test file that divides evenly into chunks
    }
    
    #[tokio::test]
    async fn test_split_with_remainder() {
        // Test file with partial last chunk
    }
    
    #[tokio::test]
    async fn test_reconstruct_all_chunks() {
        // Test perfect reconstruction
    }
    
    #[tokio::test]
    async fn test_reconstruct_missing_chunks() {
        // Test erasure coding recovery
    }
}

// tests/unit/integrity_test.rs
#[cfg(test)]
mod integrity_tests {
    #[test]
    fn test_checksum_consistency() { }
    
    #[test]
    fn test_corrupted_data_detection() { }
    
    #[tokio::test]
    async fn test_parallel_verification() { }
}

// tests/unit/priority_queue_test.rs
#[cfg(test)]
mod priority_queue_tests {
    #[tokio::test]
    async fn test_fifo_within_priority() { }
    
    #[tokio::test]
    async fn test_priority_ordering() { }
    
    #[tokio::test]
    async fn test_bandwidth_allocation() { }
}
```

### **Integration Tests**

```rust
// tests/integration/transfer_flow_test.rs
#[tokio::test]
async fn test_end_to_end_transfer() {
    // Setup
    let coordinator = setup_test_coordinator().await;
    let test_file = create_large_test_file(10 * 1024 * 1024); // 10MB
    
    // Execute
    let session_id = coordinator
        .send_file(test_file.path(), "127.0.0.1:8000".parse().unwrap(), Priority::Normal)
        .await
        .unwrap();
    
    // Wait for completion
    wait_for_completion(&coordinator, &session_id, Duration::from_secs(30)).await;
    
    // Verify
    let progress = coordinator.get_progress(&session_id).await.unwrap();
    assert_eq!(progress.progress_percent, 100.0);
    assert!(matches!(progress.status, SessionStatus::Completed));
    
    // Verify file integrity
    assert_files_equal(&test_file, &output_file);
}

#[tokio::test]
async fn test_transfer_with_erasure_recovery() {
    // Create 10MB file, split into chunks with 10+3 erasure coding
    // Simulate loss of 3 random chunks
    // Verify reconstruction succeeds
}

#[tokio::test]
async fn test_pause_resume_transfer() {
    // Start transfer, pause midway, verify no progress
    // Resume, verify completion
}

#[tokio::test]
async fn test_multipath_failover() {
    // Start transfer on multiple paths
    // Kill one path midway
    // Verify transfer continues on remaining paths
}
```

### **Network Simulation Tests**

```rust
// tests/network_sim/unreliable_network_test.rs
use network_simulator::NetworkSimulator;

#[tokio::test]
async fn test_high_packet_loss() {
    let sim = NetworkSimulator::new()
        .with_packet_loss(0.3)  // 30% loss
        .with_latency(200)      // 200ms RTT
        .build();
    
    let coordinator = setup_coordinator_with_sim(sim).await;
    
    // Should still complete transfer
    let session_id = coordinator.send_file(/* ... */).await.unwrap();
    wait_for_completion(&coordinator, &session_id, Duration::from_secs(60)).await;
    
    // Verify file integrity
    assert_files_equal(/* ... */);
}

#[tokio::test]
async fn test_intermittent_connectivity() {
    let sim = NetworkSimulator::new()
        .with_random_disconnects(5, Duration::from_secs(2))  // 5 disconnects, 2s each
        .build();
    
    // Should handle disconnects and resume
}

#[tokio::test]
async fn test_bandwidth_throttling() {
    let sim = NetworkSimulator::new()
        .with_bandwidth_limit(1_000_000)  // 1 Mbps
        .build();
    
    // Should adapt chunk size and complete transfer
}
```

### **Load Tests**

```rust
// tests/load/concurrent_transfers_test.rs
#[tokio::test]
async fn test_100_concurrent_transfers() {
    let coordinator = setup_test_coordinator().await;
    let mut handles = vec![];
    
    for i in 0..100 {
        let coordinator = coordinator.clone();
        handles.push(tokio::spawn(async move {
            let file = create_test_file(1024 * 1024);
            coordinator.send_file(file, /* ... */).await
        }));
    }
    
    // Wait for all transfers
    let results = futures::future::join_all(handles).await;
    
    // Verify all succeeded
    assert!(results.iter().all(|r| r.is_ok()));
}
```

### **Test Utilities**

```rust
// tests/common/mod.rs
pub async fn setup_test_coordinator() -> Arc<TransferCoordinator> {
    // Setup in-memory database, test configuration
}

pub fn create_test_file(size: usize) -> TempFile {
    // Create random test file
}

pub async fn wait_for_completion(
    coordinator: &TransferCoordinator,
    session_id: &str,
    timeout: Duration,
) {
    // Poll until complete or timeout
}

pub fn assert_files_equal(path1: &Path, path2: &Path) {
    // Compare file contents
}

pub struct NetworkSimulator {
    // Mock unreliable network conditions
}
```

---

## 🔧 5. Development Workflow & Best Practices

### **Project Structure**

```
smart-file-transfer/
├── Cargo.toml
├── build.rs              # For protobuf compilation
├── proto/
│   └── transfer.proto
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── chunk/
│   │   ├── mod.rs
│   │   ├── manager.rs
│   │   ├── erasure.rs
│   │   └── types.rs
│   ├── integrity/
│   │   ├── mod.rs
│   │   ├── verifier.rs
│   │   └── types.rs
│   ├── network/
│   │   ├── mod.rs
│   │   ├── quic_transport.rs
│   │   ├── multipath.rs
│   │   └── types.rs
│   ├── priority/
│   │   ├── mod.rs
│   │   └── queue.rs
│   ├── session/
│   │   ├── mod.rs
│   │   └── store.rs
│   ├── coordinator/
│   │   ├── mod.rs
│   │   ├── state_machine.rs
│   │   └── types.rs
│   ├── api/
│   │   ├── mod.rs
│   │   ├── rest.rs
│   │   ├── websocket.rs
│   │   └── grpc.rs
│   ├── metrics/
│   │   └── mod.rs
│   └── error.rs          # Centralized error types
├── tests/
│   ├── common/
│   │   └── mod.rs
│   ├── unit/
│   ├── integration/
│   ├── network_sim/
│   └── load/
├── examples/
│   ├── simple_transfer.rs
│   └── demo_scenario.rs
└── scripts/
    ├── setup_network_sim.sh
    └── run_demo.sh
```

### **Error Handling Strategy**

```rust
// src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransferError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),
    
    #[error("Integrity check failed: expected {expected:?}, got {actual:?}")]
    IntegrityCheckFailed {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    
    #[error("Erasure coding error: {0}")]
    ErasureCoding(String),
    
    #[error("Invalid state transition")]
    InvalidStateTransition,
    
    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,
}

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("No network path available")]
    NoPathAvailable,
    
    #[error("QUIC error: {0}")]
    Quic(#[from] quinn::ConnectionError),
}

pub type Result<T> = std::result::Result<T, TransferError>;
```

### **Logging Strategy**

```rust
// src/main.rs
use tracing::{info, warn, error, debug};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn setup_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "smart_file_transfer=debug,quinn=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

// Usage throughout code:
info!(session_id = %session_id, "Starting file transfer");
warn!(chunk_number = chunk.metadata.sequence_number, "Chunk send failed, retrying");
error!(error = %e, "Critical failure in transfer worker");
debug!(path_id = %path.path_id, rtt_ms = path.metrics.rtt_ms, "Path metrics updated");
```

### **Configuration Management**

```rust
// src/config.rs
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub chunk_size: usize,
    pub data_shards: usize,
    pub parity_shards: usize,
    pub max_retries: u32,
    pub quic_bind_addr: String,
    pub api_bind_addr: String,
    pub grpc_bind_addr: String,
    pub database_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            chunk_size: 256 * 1024,  // 256KB
            data_shards: 10,
            parity_shards: 3,
            max_retries: 5,
            quic_bind_addr: "0.0.0.0:5000".into(),
            api_bind_addr: "0.0.0.0:8080".into(),
            grpc_bind_addr: "0.0.0.0:50051".into(),
            database_path: "./data/sessions.db".into(),
        }
    }
}

// Load from file or environment
pub fn load_config() -> Result<Config> {
    let config = config::Config::builder()
        .add_source(config::File::with_name("config"))
        .add_source(config::Environment::with_prefix("SFT"))
        .build()?;
    
    Ok(config.try_deserialize()?)
}
```


```bash
# Quick iteration
cargo watch -x 'test --lib'

# Run specific test
cargo test test_erasure_recovery -- --nocapture

# Run with logging
RUST_LOG=debug cargo test

# Benchmarks
cargo bench

# Network simulation demo
./scripts/setup_network_sim.sh
cargo run --example demo_scenario
```

### **Demo Preparation Script**

```bash
#!/bin/bash
# scripts/run_demo.sh

echo "Setting up demo environment..."

# Create test files
dd if=/dev/urandom of=test_10mb.bin bs=1M count=10
dd if=/dev/urandom of=test_100mb.bin bs=1M count=100

# Start server
cargo run --release -- server &
SERVER_PID=$!

sleep 2

# Simulate poor network (30% loss, 200ms latency)
sudo tc qdisc add dev lo root netem loss 30% delay 200ms

echo "Starting demo transfer..."
cargo run --release -- send test_10mb.bin --priority critical

# Wait for completion
sleep 30

# Reset network
sudo tc qdisc del dev lo root

# Cleanup
kill $SERVER_PID

echo "Demo complete! Check output files."
```

---

## 🎯 Key Success Metrics
