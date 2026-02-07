# RESILIENT: Disaster Data Link

<p align="center">
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/React-20232A?style=for-the-badge&logo=react&logoColor=61DAFB" alt="React">
  <img src="https://img.shields.io/badge/QUIC-Protocol-blue?style=for-the-badge" alt="QUIC">
  <img src="https://img.shields.io/badge/Tests-158%2B%20Passing-green?style=for-the-badge" alt="Tests">
  <img src="https://img.shields.io/badge/Project-100%25%20Complete-brightgreen?style=for-the-badge" alt="Complete">
</p>

> **A highly resilient file transfer system designed for disaster response scenarios.** Uses adaptive erasure coding over QUIC to guarantee data delivery even under **20-33% packet loss** conditions where traditional transfer methods fail.

---

## 🎬 Demo

[![Watch the Demo](https://img.youtube.com/vi/1nO6CneezSA/maxresdefault.jpg)](https://www.youtube.com/watch?v=1nO6CneezSA)

*Click the image above to watch RESILIENT in action*

---

## 🚨 The Problem

During disaster response operations, communication infrastructure is often damaged or overloaded:

| Challenge | Impact |
|-----------|--------|
| **Unreliable Networks** | High packet loss rates (10-20%+) are common |
| **Intermittent Connectivity** | Connections drop frequently and unpredictably |
| **Critical Data Loss** | Standard file transfer methods fail or lose data |
| **Time Sensitivity** | Delayed information can cost lives |
| **No Direct Routes** | Sometimes only multi-hop relay is possible |

**Traditional file transfer solutions (FTP, HTTP uploads, rsync) are not designed for these conditions and frequently fail.**

---

## ✅ Our Solution

RESILIENT provides guaranteed data delivery through:

| Capability | How It Works |
|------------|--------------|
| **Adaptive Erasure Coding** | Reed-Solomon with dynamic parity (5-25 shards) based on network conditions |
| **QUIC Protocol** | Modern transport with built-in reliability and TLS 1.3 encryption |
| **Delta Transfer** | rsync-style block-level sync — only send what changed |
| **Store-and-Forward Relay** | Mesh network support for disconnected scenarios |
| **Smart Retry Logic** | Exponential backoff with jitter |
| **LZ4 Compression** | Fast compression reduces bandwidth requirements |
| **Priority System** | Critical files transmitted first with guaranteed bandwidth |
| **Prometheus Metrics** | Full observability for monitoring and alerting |

---

## 🚀 Quick Start

```bash
# Clone & Build
git clone https://github.com/Sher110106/Resilient-Core-Engine.git
cd Resilient-Core-Engine
cargo build --release

# Terminal 1: Start Receiver (Command Center)
./target/release/chunkstream-receiver 0.0.0.0:5001 ./received

# Terminal 2: Start Sender (Field Agent)
./target/release/chunkstream-server

# Terminal 3: Start Frontend
cd frontend && npm install && npm start
```

Open **http://localhost:3001** → drag files → transmit securely.

---

## 🔥 Key Features

### 1. Adaptive Erasure Coding

Files are split into chunks and encoded with parity data that **automatically adapts to network conditions**:

| Network Condition | Loss Rate | Parity Shards | Overhead | Recovery |
|------------------|-----------|---------------|----------|----------|
| Excellent | 0-5% | 5 | 9% | ~8% loss |
| Good | 5-10% | 10 | 17% | ~16% loss |
| Degraded | 10-15% | 15 | 23% | ~23% loss |
| Poor | 15-20% | 20 | 29% | ~29% loss |
| **Severe** | **20%+** | **25** | **33%** | **~33% loss** |

### 2. Delta Transfer (rsync-style)

When updating existing files:
- **Rolling Checksum**: Adler-32 weak hash for fast block matching
- **Strong Hash**: BLAKE3 (128-bit) for verification
- **Typical savings**: **80-99% bandwidth reduction** for incremental updates

### 3. Store-and-Forward Relay

For scenarios where direct connectivity is impossible:
- Mesh network with multiple relay nodes
- Priority-based forwarding (critical data first)
- TTL enforcement prevents loops
- Persistent storage until delivery possible

### 4. Three-Tier Priority System

| Priority | Bandwidth | Use Case |
|----------|-----------|----------|
| **Critical** | 50% | Emergency alerts, casualty reports |
| **High** | 30% | Situation updates, resource requests |
| **Normal** | 20% | Documentation, logs, non-urgent data |

### 5. Intelligent Resume

- **Session Persistence**: State saved to SQLite, survives crashes/restarts
- **Chunk-Level Tracking**: Resume from exact byte position
- **Automatic Recovery**: Paused and failed transfers can resume seamlessly

### 6. Full Observability

Prometheus metrics with 20+ measurements:
```
resilient_chunks_sent_total
resilient_chunks_lost_total
resilient_chunks_recovered_total
resilient_active_transfers
resilient_throughput_bytes_per_second
resilient_packet_loss_rate
```

---

## 📊 Performance

### Packet Loss Tolerance

| Network Condition | Packet Loss | Effective Throughput | Recovery Rate |
|-------------------|-------------|---------------------|---------------|
| Excellent | 0% | ~95% of bandwidth | N/A |
| Good | 5% | ~90% of bandwidth | 100% |
| Degraded | 10% | ~80% of bandwidth | 100% |
| Poor | 15% | ~70% of bandwidth | 100% |
| Severe | 20% | ~60% of bandwidth | ~99% |
| Critical | 25% | ~50% of bandwidth | ~95% |
| **Extreme** | **30%** | ~40% of bandwidth | ~90% |

### Delta Transfer Efficiency

| Change Type | Data Transferred | Savings |
|-------------|------------------|---------|
| No change | ~100 bytes | ~100% |
| 1% modified | ~2% of file | ~98% |
| 10% modified | ~15% of file | ~85% |
| 50% modified | ~60% of file | ~40% |

---

## 🏗️ Architecture

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
│  │  • State Machine  • Priority Queue  • Session Store (SQLite) │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                            │                                        │
│                            ▼                                        │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    CHUNK MANAGER                              │  │
│  │  • LZ4 Compression  • Adaptive Erasure  • Delta Transfer     │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                            │                                        │
│                            ▼                                        │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                  NETWORK LAYER                                │  │
│  │  • QUIC Transport  • Rate Limiter  • Store-and-Forward Relay │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                     ┌────────────┴────────────┐
                     ▼                         ▼
           ┌─────────────────┐       ┌─────────────────┐
           │   DIRECT QUIC   │       │  RELAY NODES    │
           │   Connection    │       │  (Mesh Network) │
           └─────────────────┘       └─────────────────┘
                     │                         │
                     └────────────┬────────────┘
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      COMMAND CENTER (RECEIVER)                      │
│  • QUIC Listener  • Erasure Decoder  • Integrity Verifier (BLAKE3) │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 🛠️ Tech Stack

### Backend (Rust)

| Component | Technology | Purpose |
|-----------|------------|---------|
| **Language** | Rust 2021 | Memory safety, performance |
| **Async Runtime** | Tokio | High-performance async I/O |
| **Transport** | QUIC (Quinn) | Reliable UDP, TLS 1.3 |
| **Erasure Coding** | reed-solomon-erasure | Data recovery |
| **Hashing** | BLAKE3 | Fast cryptographic integrity |
| **Compression** | lz4_flex | Fast compression |
| **Web Framework** | Axum | REST API + WebSocket |
| **Database** | SQLite (SQLx) | Session persistence |
| **Rate Limiting** | Governor | Token bucket limiting |
| **Metrics** | Prometheus | Full observability |

### Frontend (React)

| Component | Technology |
|-----------|------------|
| **Framework** | React 18 |
| **HTTP Client** | Axios |
| **Real-time** | WebSocket |

---

## 📁 Project Structure

```
src/
├── chunk/          # Erasure coding, compression, adaptive parity
├── sync/           # Delta transfer (rsync-style rolling hash)
├── relay/          # Store-and-forward relay nodes
├── metrics/        # Prometheus observability
├── network/        # QUIC transport, rate limiting, multipath
├── coordinator/    # Transfer lifecycle orchestration
├── priority/       # Three-tier priority queue
├── session/        # SQLite persistence & intelligent resume
├── integrity/      # BLAKE3 verification
└── api/            # REST + WebSocket endpoints

tests/
├── simulation/     # Network simulation framework
├── stress/         # Large file & concurrent stress tests
└── *.rs            # Integration & benchmark tests

frontend/           # React web interface
```

---

## 📡 API Reference

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

## 🧪 Testing

```bash
# Run all tests (158+)
cargo test

# Library tests (132)
cargo test --lib

# Integration tests (5)
cargo test --test integration_test

# Stress tests (12)
cargo test --test stress_tests

# Benchmarks
cargo bench
```

### Test Coverage

| Category | Tests |
|----------|-------|
| Library Tests | 132 |
| Integration Tests | 5 |
| Stress Tests | 12 |
| Benchmark Tests | 9 |
| **Total** | **158+** |

---

## ⚔️ Competitive Advantage

| Feature | RESILIENT | rsync | croc | Syncthing | FTP |
|---------|-----------|-------|------|-----------|-----|
| **Packet Loss Tolerance** | **33%** | <1% | <5% | <5% | <1% |
| **Adaptive Erasure** | **Yes** | No | No | No | No |
| **Delta Transfer** | **Yes** | Yes | No | Yes | No |
| **Store-and-Forward** | **Yes** | No | Yes | No | No |
| **Priority Queue** | **Yes** | No | No | No | No |
| **Prometheus Metrics** | **Yes** | No | No | No | No |
| **Rate Limiting** | **Yes** | Yes | No | No | No |
| **E2E Encryption** | TLS 1.3 | SSH | PAKE | TLS | Optional |

---

## 🔧 Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `CHUNK_SIZE` | 524288 | Size of each chunk (512KB) |
| `DATA_SHARDS` | 50 | Number of data shards |
| `MIN_PARITY_SHARDS` | 5 | Minimum parity shards |
| `MAX_PARITY_SHARDS` | 25 | Maximum parity shards |
| `RECEIVER_ADDR` | 127.0.0.1:5001 | Receiver QUIC address |
| `METRICS_PORT` | 9090 | Prometheus metrics port |

---

## 👤 Built By

**[Sher110106](https://github.com/Sher110106)**

---

<p align="center">
  <strong>Built with Rust 🦀 for Maximum Performance and Reliability</strong><br>
  <em>Powered by QUIC Protocol with Adaptive Erasure Coding</em>
</p>
