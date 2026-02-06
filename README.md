# RESILIENT: Disaster Data Link

> A resilient file transfer system for disaster response — uses adaptive erasure coding over QUIC to deliver data even with 20-33% packet loss.

![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)
![React](https://img.shields.io/badge/React-20232A?style=flat&logo=react&logoColor=61DAFB)
![QUIC](https://img.shields.io/badge/QUIC-Protocol-blue)
![Tests](https://img.shields.io/badge/Tests-158%2B%20Passing-green)

## Quick Start

```bash
# Clone & Build
git clone https://github.com/Sher110106/Resilient-Core-Engine.git
cd Resilient-Core-Engine
cargo build --release

# Start Receiver (Command Center)
./target/release/chunkstream-receiver 0.0.0.0:5001 ./received

# Start Sender (Field Agent) — in another terminal
./target/release/chunkstream-server

# Start Frontend — in another terminal
cd frontend && npm install && npm start
```

Open **http://localhost:3001** → drag files → transmit securely.

## Key Features

| Feature | Description |
|---------|-------------|
| **Adaptive Erasure Coding** | 5-25 parity shards, auto-adjusts to network conditions |
| **Delta Transfer** | rsync-style block-level sync, only send changes |
| **Store-and-Forward** | Mesh relay network for disconnected scenarios |
| **Priority Queue** | Critical / High / Normal transmission priority |
| **LZ4 Compression** | Fast compression reduces bandwidth |
| **Prometheus Metrics** | Full observability on port 9090 |
| **Rate Limiting** | Governor-based bandwidth control |

## Packet Loss Tolerance

| Conditions | Loss Rate | Recovery |
|------------|-----------|----------|
| Good | 0-10% | 100% |
| Degraded | 10-20% | 99%+ |
| Severe | 20-33% | 95%+ |

## Tech Stack

| Layer | Technology |
|-------|------------|
| Backend | Rust, Axum, Quinn (QUIC), Tokio |
| Encoding | Reed-Solomon erasure, BLAKE3 hashing, LZ4 |
| Frontend | React 18, Axios, WebSocket |
| Protocol | QUIC with TLS 1.3 |
| Metrics | Prometheus via `metrics` crate |

## Project Structure

```
src/
├── chunk/          # Erasure coding, compression, adaptive parity
├── sync/           # Delta transfer (rsync-style)
├── relay/          # Store-and-forward relay nodes
├── metrics/        # Prometheus observability
├── network/        # QUIC transport, rate limiting
├── coordinator/    # Transfer orchestration
├── priority/       # Three-tier priority queue
├── session/        # SQLite persistence
└── api/            # REST + WebSocket

tests/
├── simulation/     # Network simulation framework
├── stress/         # Stress tests
└── *.rs            # Integration & benchmark tests

frontend/           # React web interface
```

## Documentation

- [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md) - Detailed feature overview
- [PHASE_COMPARISON.md](PHASE_COMPARISON.md) - Implementation status
- [TECHNICAL_DEEP_DIVE.md](TECHNICAL_DEEP_DIVE.md) - In-depth technical documentation

## Running Tests

```bash
cargo test                           # All tests (158+)
cargo test --lib                     # Library tests (132)
cargo test --test integration_test   # Integration tests (5)
cargo test --test stress_tests       # Stress tests (12)
```

## Metrics Endpoint

```bash
# Start with metrics enabled
./target/release/chunkstream-server

# Scrape metrics
curl http://localhost:9090/metrics
```

## Built By

**[Sher110106](https://github.com/Sher110106)**

---

*Powered by QUIC Protocol with Adaptive Erasure Coding*
