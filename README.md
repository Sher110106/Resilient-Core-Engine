# RESILIENT: Disaster Data Link

> A resilient file transfer system for disaster response — uses erasure coding over QUIC to deliver data even with 20% packet loss.

![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)
![React](https://img.shields.io/badge/React-20232A?style=flat&logo=react&logoColor=61DAFB)
![QUIC](https://img.shields.io/badge/QUIC-Protocol-blue)

## 🚀 Quick Start

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

## 🛠️ Tech Stack

| Layer     | Technology                                    |
|-----------|-----------------------------------------------|
| Backend   | Rust, Axum, Quinn (QUIC), Tokio               |
| Encoding  | Reed-Solomon erasure coding, BLAKE3 hashing   |
| Frontend  | React 18, Axios, WebSocket                    |
| Protocol  | QUIC with TLS 1.3                             |

## ✨ Key Features

- **Erasure Coding** — recover files from 20% chunk loss
- **Priority Queue** — Critical / High / Normal transmission priority
- **Real-time Dashboard** — WebSocket-based live progress tracking
- **Dual Mode UI** — Field Agent (sender) & Command Center (receiver)

## 📁 Project Structure

```
├── src/           # Rust backend (QUIC server, chunk processing)
├── frontend/      # React web interface
├── examples/      # Demo scripts for each module
├── received/      # Received files directory
└── uploads/       # Test files for transmission
```

## 👤 Built By

**[Sher110106](https://github.com/Sher110106)**

---

*Powered by QUIC Protocol with Erasure Coding*
