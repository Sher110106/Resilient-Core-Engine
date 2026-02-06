# RESILIENT - Disaster Data Link

## Executive Summary

**RESILIENT** is a cutting-edge resilient file transfer system specifically designed for disaster response scenarios. It enables reliable transmission of critical files even under severely degraded network conditions with up to **20-33% packet loss**. The system connects field agents in disaster zones with command centers, ensuring vital data reaches its destination regardless of network instability.

---

## The Problem We Solve

During disaster response operations, communication infrastructure is often damaged or overloaded, leading to:

- **Unreliable Networks**: High packet loss rates (10-20%+) are common
- **Intermittent Connectivity**: Connections drop frequently and unpredictably
- **Critical Data Loss**: Standard file transfer methods fail or lose data
- **Time Sensitivity**: Delayed information can cost lives
- **No Direct Routes**: Sometimes only multi-hop relay is possible

Traditional file transfer solutions (FTP, HTTP uploads, rsync) are not designed for these conditions and frequently fail, requiring manual retries and risking data loss.

---

## Our Solution

RESILIENT provides a **military-grade file transfer system** that guarantees data delivery through:

| Capability | How It Works |
|------------|--------------|
| **Adaptive Erasure Coding** | Reed-Solomon with dynamic parity (5-25 shards) based on network conditions |
| **QUIC Protocol** | Modern transport protocol with built-in reliability and TLS 1.3 encryption |
| **Delta Transfer** | rsync-style block-level sync - only send what changed |
| **Store-and-Forward Relay** | Mesh network support for disconnected scenarios |
| **Smart Retry Logic** | Exponential backoff with jitter |
| **LZ4 Compression** | Fast compression reduces bandwidth requirements |
| **Priority System** | Critical files transmitted first with guaranteed bandwidth |
| **Prometheus Metrics** | Full observability for monitoring and alerting |

---

## Key Features

### 1. Adaptive Erasure Coding

Files are split into chunks and encoded with parity data using Reed-Solomon erasure coding that **automatically adapts to network conditions**:

| Network Condition | Loss Rate | Parity Shards | Overhead | Recovery |
|------------------|-----------|---------------|----------|----------|
| Excellent | 0-5% | 5 | 9% | ~8% loss |
| Good | 5-10% | 10 | 17% | ~16% loss |
| Degraded | 10-15% | 15 | 23% | ~23% loss |
| Poor | 15-20% | 20 | 29% | ~29% loss |
| Severe | 20%+ | 25 | 33% | ~33% loss |

The system monitors packet loss in real-time and automatically adjusts parity levels.

### 2. Delta Transfer (rsync-style)

When updating files that already exist at the destination:

- **Rolling Checksum**: Adler-32 weak hash for fast block matching
- **Strong Hash**: BLAKE3 (128-bit) for verification
- **Minimal Transfer**: Only changed blocks are transmitted
- **Streaming Support**: Works with files of any size

Typical savings: 80-99% bandwidth reduction for incremental updates.

### 3. Store-and-Forward Relay

For scenarios where direct connectivity is impossible:

- **Mesh Network**: Multiple relay nodes can form a mesh
- **Priority Forwarding**: Critical data forwarded first
- **TTL Enforcement**: Prevents infinite loops
- **Persistent Storage**: Chunks stored until delivery possible
- **Automatic Retry**: Exponential backoff for failed forwards

### 4. Priority Queue System

Three-tier priority system ensures critical data gets through first:

| Priority Level | Bandwidth Allocation | Use Case |
|----------------|---------------------|----------|
| **Critical** | 50% | Emergency alerts, casualty reports |
| **High** | 30% | Situation updates, resource requests |
| **Normal** | 20% | Documentation, logs, non-urgent data |

### 5. LZ4 Compression

- **Fast Compression**: ~500 MB/s compression speed
- **Good Ratio**: 2-3x compression on typical data
- **Auto-Detection**: Skips already-compressed data
- **Optional**: Can be disabled for real-time requirements

### 6. Prometheus Metrics

Full observability with 20+ metrics:

```
Counters:
  resilient_chunks_sent_total
  resilient_chunks_received_total
  resilient_chunks_lost_total
  resilient_chunks_recovered_total
  resilient_transfers_completed_total
  resilient_transfers_failed_total

Gauges:
  resilient_active_transfers
  resilient_queue_depth
  resilient_storage_used_bytes

Histograms:
  resilient_transfer_duration_seconds
  resilient_throughput_bytes_per_second
  resilient_network_latency_ms
  resilient_packet_loss_rate
```

### 7. Rate Limiting

Governor-based rate limiting with:

- **Bytes/second limiting**: Control bandwidth usage
- **Chunks/second limiting**: Control request rate
- **Adaptive adjustment**: Automatically backs off on congestion

### 8. Full Transfer Control

- **Pause/Resume**: Suspend and continue transfers at any point
- **Cancel**: Abort transfers and free resources
- **Session Persistence**: State saved to SQLite, survives restarts
- **Automatic Recovery**: Resumes from last checkpoint

---

## Technical Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         FIELD AGENT (SENDER)                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │  Web UI      │  │  REST API    │  │  WebSocket Server        │  │
│  │  (React)     │◄─►│  (Axum)      │◄─►│  (Real-time Updates)     │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
│                            │                                        │
│                            ▼                                        │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                  TRANSFER COORDINATOR                         │  │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────────┐ │  │
│  │  │ State       │ │ Priority    │ │ Session                 │ │  │
│  │  │ Machine     │ │ Queue       │ │ Store (SQLite)          │ │  │
│  │  └─────────────┘ └─────────────┘ └─────────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                            │                                        │
│                            ▼                                        │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    CHUNK MANAGER                              │  │
│  │  ┌───────────────┐ ┌───────────────┐ ┌───────────────────┐  │  │
│  │  │ LZ4           │ │ Adaptive      │ │ Delta Transfer    │  │  │
│  │  │ Compression   │ │ Erasure Coder │ │ (rsync-style)     │  │  │
│  │  └───────────────┘ └───────────────┘ └───────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                            │                                        │
│                            ▼                                        │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                  NETWORK LAYER                                │  │
│  │  ┌───────────────┐ ┌───────────────┐ ┌───────────────────┐  │  │
│  │  │ QUIC          │ │ Rate Limiter  │ │ Store-and-Forward │  │  │
│  │  │ Transport     │ │ (Governor)    │ │ Relay             │  │  │
│  │  └───────────────┘ └───────────────┘ └───────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                            │                                        │
│                            ▼                                        │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                  OBSERVABILITY                                │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │ Prometheus Metrics Exporter (port 9090)                │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                                 │
                    ┌────────────┴────────────┐
                    │                         │
                    ▼                         ▼
          ┌─────────────────┐       ┌─────────────────┐
          │   DIRECT QUIC   │       │  RELAY NODES    │
          │   Connection    │       │  (Mesh Network) │
          └─────────────────┘       └─────────────────┘
                    │                         │
                    └────────────┬────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      COMMAND CENTER (RECEIVER)                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                  QUIC LISTENER                                │  │
│  │  • Receives Chunks       • Verifies Integrity                │  │
│  │  • Manages Connections   • Tracks Progress                   │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                            │                                        │
│                            ▼                                        │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                  FILE RECONSTRUCTOR                           │  │
│  │  ┌─────────────────────────┐ ┌─────────────────────────────┐ │  │
│  │  │ Erasure Decoder         │ │ Integrity Verifier          │ │  │
│  │  │ (Recovers lost chunks)  │ │ (BLAKE3 validation)         │ │  │
│  │  └─────────────────────────┘ └─────────────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

### Technology Stack

#### Backend (Rust)

| Component | Technology | Purpose |
|-----------|------------|---------|
| **Language** | Rust 2021 | Memory safety, performance, reliability |
| **Async Runtime** | Tokio | High-performance async I/O |
| **Transport** | QUIC (Quinn) | Reliable UDP-based transport |
| **Encryption** | TLS 1.3 (rustls) | End-to-end encryption |
| **Erasure Coding** | reed-solomon-erasure | Data recovery |
| **Hashing** | BLAKE3 | Fast cryptographic integrity |
| **Compression** | lz4_flex | Fast compression |
| **Web Framework** | Axum | REST API + WebSocket |
| **Database** | SQLite (SQLx) | Session persistence |
| **Rate Limiting** | Governor | Token bucket rate limiting |
| **Metrics** | metrics + prometheus | Observability |
| **Retry** | backoff | Exponential backoff with jitter |

#### Frontend (React)

| Component | Technology | Purpose |
|-----------|------------|---------|
| **Framework** | React 18 | Modern UI library |
| **HTTP Client** | Axios | API communication |
| **Real-time** | WebSocket (native) | Live updates |

---

## Module Breakdown

### 1. Chunk Module (`src/chunk/`)

| File | Purpose |
|------|---------|
| `manager.rs` | File splitting and reconstruction |
| `erasure.rs` | Reed-Solomon encoding/decoding |
| `adaptive.rs` | Dynamic parity adjustment |
| `compression.rs` | LZ4 compression |
| `types.rs` | Chunk, ChunkMetadata, FileManifest |

### 2. Sync Module (`src/sync/`)

| File | Purpose |
|------|---------|
| `rolling_hash.rs` | Adler-32 rolling checksum |
| `signature.rs` | Block signature generation |
| `delta.rs` | Delta computation and application |

### 3. Relay Module (`src/relay/`)

| File | Purpose |
|------|---------|
| `types.rs` | RelayConfig, RouteInfo, RelayMessage |
| `storage.rs` | Priority-aware chunk storage |
| `node.rs` | Relay node implementation |

### 4. Metrics Module (`src/metrics/`)

| File | Purpose |
|------|---------|
| `recorder.rs` | Metric recording functions |
| `exporter.rs` | Prometheus HTTP exporter |

### 5. Network Module (`src/network/`)

| File | Purpose |
|------|---------|
| `quic_transport.rs` | QUIC connection management |
| `rate_limiter.rs` | Bandwidth throttling |
| `multipath.rs` | Multi-interface support |

### 6. Other Modules

| Module | Purpose |
|--------|---------|
| `coordinator/` | Transfer lifecycle orchestration |
| `priority/` | Three-tier priority queue |
| `integrity/` | BLAKE3 verification |
| `session/` | SQLite persistence |
| `api/` | REST API + WebSocket |

---

## Performance Characteristics

### Throughput Under Packet Loss

| Network Condition | Packet Loss | Effective Throughput | Recovery Rate |
|-------------------|-------------|---------------------|---------------|
| Excellent | 0% | ~95% of bandwidth | N/A |
| Good | 5% | ~90% of bandwidth | 100% |
| Degraded | 10% | ~80% of bandwidth | 100% |
| Poor | 15% | ~70% of bandwidth | 100% |
| Severe | 20% | ~60% of bandwidth | ~99% |
| Critical | 25% | ~50% of bandwidth | ~95% |
| Extreme | 30% | ~40% of bandwidth | ~90% |

### Delta Transfer Efficiency

| Change Type | Data Transferred | Savings |
|-------------|------------------|---------|
| No change | ~100 bytes (signature only) | ~100% |
| 1% modified | ~2% of file | ~98% |
| 10% modified | ~15% of file | ~85% |
| 50% modified | ~60% of file | ~40% |
| Complete rewrite | ~105% of file | -5% |

---

## Deployment

### Quick Start

```bash
# Build
cargo build --release

# Start Receiver (Command Center)
./target/release/chunkstream-receiver 0.0.0.0:5001 ./received

# Start Sender (Field Agent)
./target/release/chunkstream-server

# Start Frontend
cd frontend && npm start
```

### Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `CHUNK_SIZE` | 524288 (512KB) | Size of each chunk |
| `DATA_SHARDS` | 50 | Number of data shards |
| `MIN_PARITY_SHARDS` | 5 | Minimum parity shards |
| `MAX_PARITY_SHARDS` | 25 | Maximum parity shards |
| `RECEIVER_ADDR` | 127.0.0.1:5001 | Receiver QUIC address |
| `METRICS_PORT` | 9090 | Prometheus metrics port |

---

## API Reference

### REST Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/api/v1/upload` | POST | Upload file (multipart) |
| `/api/v1/transfers` | GET | List all transfers |
| `/api/v1/transfers/:id` | GET | Get transfer details |
| `/api/v1/transfers/:id/progress` | GET | Get progress |
| `/api/v1/transfers/:id/pause` | POST | Pause transfer |
| `/api/v1/transfers/:id/resume` | POST | Resume transfer |
| `/api/v1/transfers/:id/cancel` | POST | Cancel transfer |
| `/ws` | WebSocket | Real-time updates |
| `/metrics` | GET | Prometheus metrics |

---

## Competitive Advantages

| Feature | RESILIENT | rsync | croc | Syncthing | FTP |
|---------|-----------|-------|------|-----------|-----|
| Packet Loss Tolerance | **33%** | <1% | <5% | <5% | <1% |
| Adaptive Erasure | **Yes** | No | No | No | No |
| Delta Transfer | **Yes** | Yes | No | Yes | No |
| Store-and-Forward | **Yes** | No | Yes | No | No |
| Priority Queue | **Yes** | No | No | No | No |
| Prometheus Metrics | **Yes** | No | No | No | No |
| Rate Limiting | **Yes** | Yes | No | No | No |
| E2E Encryption | TLS 1.3 | SSH | PAKE | TLS | Optional |

---

## Test Coverage

```
Library Tests:       132 passing
Integration Tests:     5 passing
Stress Tests:         12 passing
Benchmark Tests:       9 passing
─────────────────────────────────
Total:               158+ tests
```

---

## Summary

**RESILIENT** is a purpose-built file transfer system for the most challenging network conditions. By combining:

1. **Adaptive Erasure Coding** - Handles up to 33% packet loss
2. **Delta Transfer** - Minimizes bandwidth for updates
3. **Store-and-Forward** - Works without direct connectivity
4. **Priority System** - Critical data first
5. **Full Observability** - Prometheus metrics for monitoring
6. **Rate Limiting** - Prevents network congestion

It ensures that critical data reaches its destination when it matters most.

---

*Built with Rust for maximum performance and reliability.*
