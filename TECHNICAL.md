# ChunkStream Pro - Technical Documentation

This document provides in-depth technical details about the ChunkStream Pro architecture, implementation, and design decisions.

## Table of Contents

1. [System Architecture](#system-architecture)
2. [Core Modules](#core-modules)
3. [Data Flow](#data-flow)
4. [Network Protocol](#network-protocol)
5. [State Management](#state-management)
6. [Error Handling](#error-handling)
7. [Performance Optimizations](#performance-optimizations)
8. [Security](#security)

---

## System Architecture

### High-Level Design

ChunkStream Pro follows a modular, layered architecture:

```
┌─────────────────────────────────────────────────────────────┐
│                    Presentation Layer                        │
│              React SPA (Sender/Receiver UI)                  │
└──────────────────────┬──────────────────────────────────────┘
                       │ HTTP/WS (Axum)
┌──────────────────────┴──────────────────────────────────────┐
│                    Application Layer                         │
│  ┌────────────────────────────────────────────────────────┐ │
│  │         Transfer Coordinator (Orchestration)           │ │
│  └────────────────────────────────────────────────────────┘ │
└──────────────────────┬──────────────────────────────────────┘
                       │
┌──────────────────────┴──────────────────────────────────────┐
│                    Business Logic Layer                      │
│  ┌─────────────┐ ┌──────────────┐ ┌────────────────────┐   │
│  │Chunk Manager│ │Priority Queue│ │Session Store       │   │
│  └─────────────┘ └──────────────┘ └────────────────────┘   │
│  ┌─────────────┐ ┌──────────────┐ ┌────────────────────┐   │
│  │  Verifier   │ │State Machine │ │Integrity Checker   │   │
│  └─────────────┘ └──────────────┘ └────────────────────┘   │
└──────────────────────┬──────────────────────────────────────┘
                       │
┌──────────────────────┴──────────────────────────────────────┐
│                    Transport Layer                           │
│              QUIC Transport (quinn/rustls)                   │
└─────────────────────────────────────────────────────────────┘
```

### Technology Stack

**Backend:**
- **Language**: Rust 2021 Edition
- **Async Runtime**: Tokio 1.35
- **HTTP Framework**: Axum 0.7
- **Transport**: QUIC (quinn 0.11) over TLS 1.3
- **Cryptography**: rustls 0.23 with ring provider
- **Database**: SQLx 0.8 with SQLite
- **Serialization**: serde + bincode

**Frontend:**
- **Framework**: React 18.2
- **HTTP Client**: Axios 1.6
- **Build Tool**: Create React App 5.0

---

## Core Modules

### 1. Chunk Manager (`src/chunk/`)

**Purpose**: File splitting, reassembly, and erasure coding.

#### Key Components

**`ChunkManager`**
```rust
pub struct ChunkManager {
    chunk_size: usize,           // Base chunk size (e.g., 512KB)
    data_shards: usize,          // Number of data shards (e.g., 50)
    parity_shards: usize,        // Number of parity shards (e.g., 10)
    encoder: ReedSolomon,        // Reed-Solomon codec
}
```

**Responsibilities:**
- Split files into fixed-size chunks
- Apply Reed-Solomon erasure coding (10 data + 3 parity by default)
- Reconstruct files from partial chunks
- Calculate optimal chunk size based on network conditions

**Algorithm:**

```
File Split Process:
1. Read file in chunk_size blocks
2. Pad last block if needed
3. For each group of data_shards blocks:
   a. Encode using Reed-Solomon (generates parity_shards)
   b. Create chunk metadata (hash, sequence, etc.)
   c. Return data chunks + parity chunks

File Reconstruction:
1. Collect received chunks (minimum: data_shards)
2. Sort by sequence number
3. If any missing: Use Reed-Solomon to recover
4. Concatenate data shards
5. Verify final hash
```

**Adaptive Chunk Sizing:**

```rust
pub fn calculate_optimal_chunk_size(&self, rtt_ms: u32, loss_rate: f32) -> usize {
    let base = self.chunk_size as f32;
    
    // Scale down for high RTT (reduce retransmission impact)
    let rtt_factor = 1.0 - (rtt_ms as f32 / 1000.0).min(0.5);
    
    // Scale down for high loss (more frequent retransmissions)
    let loss_factor = 1.0 - loss_rate.min(0.3);
    
    (base * rtt_factor * loss_factor) as usize
}
```

---

### 2. Network Module (`src/network/`)

**Purpose**: QUIC-based transport with multipath support.

#### QUIC Transport

**`QuicTransport`**
```rust
pub struct QuicTransport {
    endpoint: Endpoint,
    connections: Arc<DashMap<String, Connection>>,
    stats: Arc<RwLock<NetworkStats>>,
}
```

**Why QUIC?**
- Built on UDP (better for lossy networks)
- Multiplexed streams (no head-of-line blocking)
- 0-RTT connection resumption
- Built-in encryption (TLS 1.3)
- Connection migration support

**Chunk Transmission Protocol:**

```
Chunk Wire Format:
┌──────────────┬──────────────┬────────────────┐
│ Metadata Len │   Metadata   │  Chunk Data    │
│   (4 bytes)  │ (variable)   │  (variable)    │
└──────────────┴──────────────┴────────────────┘

Metadata (bincode-serialized):
- chunk_id: u32
- file_id: String
- sequence_number: u32
- total_chunks: u32
- data_size: usize
- checksum: [u8; 32]
- is_parity: bool
- priority: Priority
- created_at: i64
```

**Connection Management:**

```rust
// Server accepts connections
pub async fn accept(&self) -> NetworkResult<Connection> {
    let incoming = self.endpoint.accept().await
        .ok_or(NetworkError::ConnectionClosed)?;
    let conn = incoming.await?;
    self.connections.insert(conn.remote_address().to_string(), conn.clone());
    Ok(conn)
}

// Client initiates connections
pub async fn connect(&self, remote_addr: SocketAddr) -> NetworkResult<Connection> {
    let endpoint = Self::make_client_endpoint()?;
    let conn = endpoint.connect(remote_addr, "localhost")?.await?;
    self.connections.insert(remote_addr.to_string(), conn.clone());
    Ok(conn)
}
```

---

### 3. Priority Queue (`src/priority/`)

**Purpose**: Manage chunk transmission order based on priority levels.

**`PriorityQueue`**
```rust
pub struct PriorityQueue {
    critical: VecDeque<Chunk>,  // P0: Critical
    high: VecDeque<Chunk>,      // P1: High
    normal: VecDeque<Chunk>,    // P2: Normal
    capacity: usize,
    lock: Mutex<()>,
}
```

**Scheduling Algorithm:**

```
Priority Round-Robin:
1. Always serve Critical queue first (if non-empty)
2. Then High queue (2x weight)
3. Then Normal queue (1x weight)

Example sequence: C, C, C, H, H, N, C, H, H, N, ...
```

**Implementation:**

```rust
pub async fn dequeue(&self) -> Option<Chunk> {
    let _lock = self.lock.lock().await;
    
    // Try Critical first
    if let Some(chunk) = self.critical.pop_front() {
        return Some(chunk);
    }
    
    // Then High (weighted)
    if let Some(chunk) = self.high.pop_front() {
        return Some(chunk);
    }
    
    // Finally Normal
    self.normal.pop_front()
}
```

---

### 4. Integrity Verifier (`src/integrity/`)

**Purpose**: Cryptographic verification using BLAKE3.

**Why BLAKE3?**
- Faster than SHA-256 (10x on modern CPUs)
- Parallelizable (uses SIMD)
- Tree-mode hashing (enables partial verification)
- 256-bit output (same as SHA-256)

**Verification Process:**

```rust
pub fn verify_chunk(&self, chunk: &Chunk) -> bool {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&chunk.data);
    let computed_hash = hasher.finalize();
    
    computed_hash.as_bytes() == &chunk.metadata.checksum
}

pub fn verify_file(&self, file_path: &Path, expected_hash: &[u8; 32]) -> Result<bool> {
    let file = std::fs::File::open(file_path)?;
    let mut hasher = blake3::Hasher::new();
    let mut reader = std::io::BufReader::new(file);
    std::io::copy(&mut reader, &mut hasher)?;
    
    Ok(hasher.finalize().as_bytes() == expected_hash)
}
```

---

### 5. Session Store (`src/session/`)

**Purpose**: Persist transfer state in SQLite database.

**Schema:**

```sql
CREATE TABLE sessions (
    session_id TEXT PRIMARY KEY,
    file_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    status TEXT NOT NULL,
    priority TEXT NOT NULL,
    total_chunks INTEGER NOT NULL,
    completed_chunks INTEGER NOT NULL,
    bytes_transferred INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    chunk_id INTEGER NOT NULL,
    sequence_number INTEGER NOT NULL,
    status TEXT NOT NULL,
    checksum BLOB NOT NULL,
    is_parity INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(session_id) REFERENCES sessions(session_id)
);
```

**Session Lifecycle:**

```
Created → Preparing → Transferring → [Paused] → Completing → Completed
                                    ↓
                                 Failed
```

---

### 6. Transfer Coordinator (`src/coordinator/`)

**Purpose**: Orchestrate all modules for end-to-end file transfer.

**State Machine:**

```rust
pub enum TransferState {
    Idle,
    Preparing,
    Transferring { progress: f32 },
    Paused { at_chunk: u32 },
    Completing,
    Completed,
    Failed { error: String },
}
```

**Transfer Flow:**

```
┌─────────────────────────────────────────────────────────────┐
│                    1. send_file()                            │
│  ├─ Validate file exists                                     │
│  ├─ Create session in store                                  │
│  ├─ Transition: Idle → Preparing                             │
│  └─ Spawn transfer task                                      │
└──────────────────────┬──────────────────────────────────────┘
                       ↓
┌─────────────────────────────────────────────────────────────┐
│                 2. process_transfer()                        │
│  ├─ Split file into chunks (ChunkManager)                    │
│  ├─ Verify each chunk (IntegrityVerifier)                    │
│  ├─ Enqueue chunks (PriorityQueue)                           │
│  ├─ Transition: Preparing → Transferring                     │
│  └─ Update session store                                     │
└──────────────────────┬──────────────────────────────────────┘
                       ↓
┌─────────────────────────────────────────────────────────────┐
│                  3. send_chunks()                            │
│  ├─ Establish QUIC connection (QuicTransport)                │
│  ├─ Dequeue chunks by priority                               │
│  ├─ Send via QUIC streams                                    │
│  ├─ Handle retries (max 3 attempts)                          │
│  └─ Update progress continuously                             │
└──────────────────────┬──────────────────────────────────────┘
                       ↓
┌─────────────────────────────────────────────────────────────┐
│                   4. complete()                              │
│  ├─ Transition: Transferring → Completing                    │
│  ├─ Wait for receiver acknowledgment                         │
│  ├─ Transition: Completing → Completed                       │
│  └─ Update session store                                     │
└─────────────────────────────────────────────────────────────┘
```

---

## Data Flow

### Upload & Transfer Flow

```
User Browser → Frontend → Backend API → Coordinator → Network
     │              │           │             │            │
     │ 1. POST      │           │             │            │
     │   /upload    │           │             │            │
     ├─────────────>│           │             │            │
     │              │ 2. Save   │             │            │
     │              │   to disk │             │            │
     │              ├──────────>│             │            │
     │              │           │ 3. Create   │            │
     │              │           │   session   │            │
     │              │           ├────────────>│            │
     │              │           │             │ 4. Split & │
     │              │           │             │   encode   │
     │              │           │             ├───────────>│
     │              │           │             │            │
     │              │           │             │ 5. Send    │
     │              │           │             │   chunks   │
     │              │           │             │───────────>│
     │              │           │             │     QUIC   │
     │ 6. WS        │           │             │            │
     │   progress   │           │             │            │
     │<─────────────┴───────────┴─────────────┤            │
```

### Chunk Reception Flow (Receiver)

```
Network → QuicTransport → Verifier → ChunkManager → Disk
   │            │             │           │           │
   │ 1. Accept  │             │           │           │
   │   connection            │           │           │
   ├───────────>│             │           │           │
   │            │ 2. Receive  │           │           │
   │            │   chunks    │           │           │
   │            ├────────────>│           │           │
   │            │             │ 3. Verify │           │
   │            │             │   hash    │           │
   │            │             ├──────────>│           │
   │            │             │           │ 4. Collect│
   │            │             │           │   & decode│
   │            │             │           ├──────────>│
   │            │             │           │           │
   │            │             │           │ 5. Write  │
   │            │             │           │   file    │
   │            │             │           ├──────────>│
```

---

## Network Protocol

### Connection Establishment

```
Sender                                    Receiver
  │                                          │
  │  1. QUIC Initial (TLS ClientHello)       │
  ├─────────────────────────────────────────>│
  │                                          │
  │  2. QUIC Handshake (TLS ServerHello)     │
  │<─────────────────────────────────────────┤
  │                                          │
  │  3. QUIC 1-RTT (Encrypted)               │
  ├─────────────────────────────────────────>│
  │                                          │
  │  4. Send Chunks (Unidirectional streams) │
  ├═════════════════════════════════════════>│
  ├═════════════════════════════════════════>│
  ├═════════════════════════════════════════>│
  │  (parallel streams)                      │
```

### Stream Management

- **Unidirectional Streams**: Used for chunk transmission
  - Each chunk sent on separate stream
  - Avoids head-of-line blocking
  - Allows parallel transmission

- **Stream Limits**: Configured to 100 concurrent streams
  - Balances throughput vs resource usage
  - Prevents receiver overload

---

## State Management

### Session States

```rust
pub enum SessionStatus {
    Created,       // Initial state
    Preparing,     // Chunking file
    Active,        // Transmitting
    Paused,        // User paused
    Completing,    // Finalizing
    Completed,     // Success
    Failed,        // Error occurred
}
```

### State Transitions

```
Valid transitions:
- Created → Preparing
- Preparing → Active
- Active → Paused
- Paused → Active
- Active → Completing
- Completing → Completed
- Any → Failed (on error)

Invalid transitions (rejected):
- Completed → Active
- Failed → Active
- Completed → Paused
```

---

## Error Handling

### Error Categories

**1. Network Errors**
```rust
pub enum NetworkError {
    ConnectionFailed(String),
    ConnectionClosed(String),
    SendFailed(String),
    ReceiveFailed(String),
    TimeoutError(String),
    MaxRetriesExceeded(u32),
}
```

**2. Chunk Errors**
```rust
pub enum ChunkError {
    FileTooLarge { size: u64, max: u64 },
    InvalidChunkSize,
    EncodingFailed(String),
    DecodingFailed(String),
    IoError(std::io::Error),
}
```

**3. Session Errors**
```rust
pub enum SessionError {
    SessionNotFound(String),
    DatabaseError(sqlx::Error),
    InvalidState { current: String, expected: String },
}
```

### Recovery Strategies

**Transient Failures**: Automatic retry with exponential backoff
```rust
async fn send_with_retry(&self, chunk: &Chunk, max_retries: u32) {
    let mut backoff = Duration::from_millis(100);
    for attempt in 0..max_retries {
        match self.send_chunk(chunk).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                if attempt == max_retries - 1 {
                    return Err(e);
                }
                tokio::time::sleep(backoff).await;
                backoff *= 2;  // Exponential backoff
            }
        }
    }
}
```

**Permanent Failures**: Mark session as Failed, notify user

---

## Performance Optimizations

### 1. Parallel Processing

- **Chunking**: Uses rayon for parallel file reads
- **Encoding**: Reed-Solomon parallelized across CPU cores
- **Network**: Multiple concurrent QUIC streams

### 2. Memory Management

- **Zero-copy**: Uses `bytes::Bytes` (Arc-backed)
- **Streaming**: Files processed in chunks, never fully in RAM
- **Pool**: Connection pooling via DashMap

### 3. Database

- **Batch Inserts**: Chunks inserted in transactions
- **Indexes**: session_id, chunk_id indexed
- **In-memory Mode**: Uses `:memory:` for testing

### 4. Async I/O

- **Tokio Runtime**: All I/O operations non-blocking
- **Buffering**: Uses `BufReader`/`BufWriter`
- **Channel Sizing**: Bounded channels prevent memory bloat

---

## Security

### Current Implementation (Development)

⚠️ **Not Production-Ready**

- Self-signed certificates (auto-generated)
- Certificate verification disabled on client
- No authentication/authorization
- No rate limiting

### Production Requirements

**Must Implement:**

1. **TLS Certificates**: CA-signed certificates
2. **Auth**: JWT or OAuth2 for API access
3. **RBAC**: Role-based access control
4. **Rate Limiting**: Prevent abuse
5. **Input Validation**: Sanitize all inputs
6. **Audit Logging**: Track all operations
7. **Encryption at Rest**: Encrypt stored files
8. **Secret Management**: Use vault for keys

---

## Dependencies

### Core Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| tokio | 1.35 | Async runtime |
| quinn | 0.11 | QUIC implementation |
| rustls | 0.23 | TLS 1.3 |
| axum | 0.7 | HTTP/WebSocket server |
| reed-solomon-erasure | 6.0 | Erasure coding |
| blake3 | 1.5 | Cryptographic hashing |
| sqlx | 0.8 | Database access |
| serde | 1.0 | Serialization |
| dashmap | 6.0 | Concurrent hashmap |

---

## Future Enhancements

1. **Compression**: Add zstd compression before chunking
2. **Deduplication**: Content-addressed storage
3. **Resumable Uploads**: Support browser disconnections
4. **Multi-receiver**: Broadcast to multiple receivers
5. **Path Selection**: Multi-path QUIC support
6. **Congestion Control**: BBR or CUBIC implementation
7. **Metrics**: Prometheus integration
8. **Distributed**: Multi-node coordination

---

## Debugging

### Enable Tracing

```rust
use tracing_subscriber;

tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();
```

### Network Diagnostics

```bash
# Monitor QUIC connections
netstat -an | grep UDP | grep 5001

# Check packet loss
ping -c 100 <receiver_ip>

# Trace routes
traceroute <receiver_ip>
```

### Performance Profiling

```bash
# CPU profiling with cargo-flamegraph
cargo install flamegraph
cargo flamegraph --bin chunkstream-server

# Memory profiling with valgrind
valgrind --tool=massif ./target/release/chunkstream-server
```

---

**Last Updated**: 2025-10-24
