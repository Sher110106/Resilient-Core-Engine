# RESILIENT: Repository Structure & File Descriptions

This document provides a full breakdown of the project's architecture, directory structure, and the purpose of each individual code file.

## 📂 Project Overview

```text
.
├── Cargo.toml                # Rust project configuration and dependencies
├── Files.md                  # (This file) Repository structure documentation
├── README.md                 # Main project overview and setup guide
├── examples/                 # Functional demonstrations of core modules
├── frontend/                 # React-based web interface (Tailwind/CSS)
│   ├── public/               # Static assets
│   └── src/                  # React source code
│       ├── components/       # Reusable UI components
│       └── services/         # API abstraction layers
├── received/                 # Default storage for files received via QUIC
├── src/                      # Rust backend source code (Core Engine)
│   ├── api/                  # REST and WebSocket handlers (Axum)
│   ├── bin/                  # Executable entry points (Server/Receiver)
│   ├── chunk/                # Intelligent chunking & Erasure coding
│   ├── coordinator/          # Transfer lifecycle & State machine
│   ├── integrity/            # BLAKE3 hashing & Verification
│   ├── network/              # QUIC transport layer (Quinn)
│   ├── priority/             # Priority-based queueing logic
│   └── session/              # Transfer session management
├── tests/                    # Integration and unit tests
└── uploads/                  # Temporary storage for files being sent
```

---

## 🦀 Backend: `src/` (Rust)

The backend is built with Rust for high performance and safety, using QUIC for resilient transport.

### 🔌 `src/api/` (API Layer)
*Handles communication between the Frontend and the Backend.*
- **`mod.rs`**: Module entry point; exports the router and shared state.
- **`rest.rs`**: Implements RESTful endpoints for file management and status queries.
- **`websocket.rs`**: Manages real-time progress updates and status broadcasts to the UI.
- **`types.rs`**: Shared data structures for API requests and responses.
- **`error.rs`**: API-specific error handling and conversion to HTTP responses.

### 🚀 `src/bin/` (Executables)
*The entry points for running the system components.*
- **`server.rs`**: The **Field Agent** server. Manages local file uploads and coordinates sending data.
- **`receiver.rs`**: The **Command Center** receiver. Listens for QUIC connections and reconstructs files.

### 📦 `src/chunk/` (Data Processing)
*Manages how data is split and protected against loss.*
- **`manager.rs`**: High-level orchestrator for file chunking and reassembly.
- **`erasure.rs`**: Implements Reed-Solomon erasure coding for error correction.
- **`types.rs`**: Definitions for `Chunk` and `EncodedChunk` structures.
- **`error.rs`**: Errors related to encoding/decoding and data corruption.

### 🎯 `src/coordinator/` (Lifecycle)
*Manages the state and flow of data transfers.*
- **`coordinator.rs`**: The main controller that glues together networking, chunking, and session store.
- **`state_machine.rs`**: Implements a robust state machine for transfer lifecycles (Pending -> Active -> Completed/Failed).
- **`types.rs`**: Coordinator-specific types for task management.

### 🛡️ `src/integrity/` (Verification)
*Ensures data remains unmodified and correct.*
- **`verifier.rs`**: Implements BLAKE3 hashing for ultra-fast data integrity checks.
- **`types.rs`**: Data types for hash values and verification reports.

### 📡 `src/network/` (Transport)
*The low-level communication bridge.*
- **`quic_transport.rs`**: Core implementation of the QUIC protocol using the `quinn` crate.
- **`multipath.rs`**: (Advanced) Preliminary support for multi-path data delivery.
- **`types.rs`**: Network-specific abstractions (Address, Connection, Stream).

### ⚖️ `src/priority/` (Traffic Control)
*Ensures critical data is sent first.*
- **`queue.rs`**: Implements a multi-level priority queue (Critical, High, Normal).
- **`types.rs`**: Definitions for priority levels and weighted scheduling.

### 💾 `src/session/` (Persistence)
*Manages the history and current state of transfers.*
- **`store.rs`**: Handles persistent storage of transfer sessions (SQLite).
- **`types.rs`**: Persistence models for database records.

---

## ⚛️ Frontend: `frontend/src/` (React)

The frontend provides the user interface for both Field Agents and Command Centers.

### 🧩 `src/components/` (Interface)
- **`App.js`**: Core layout and routing logic.
- **`ModeSelector.js`**: Toggle between Field Agent (Sender) and Command Center (Receiver) modes.
- **`FileUpload.js`**: Drag-and-drop interface for initiating new transmissions.
- **`TransferList.js`**: Real-time dashboard showing active transfers and their progress.
- **`ReceiverDashboard.js`**: Specialized view for the Command Center to monitor incoming files.
- **`ReceivedFilesList.js`**: Viewer for completed and verified downloads.

### 🛠️ `src/services/` (Logic)
- **`api.js`**: Abstraction layer for communicating with the Field Agent API.
- **`receiverApi.js`**: Abstraction layer for communicating with the Command Center API.

---

## 🧪 Miscellaneous

- **`examples/`**: contains standalone scripts like `chunk_demo.rs` and `network_demo.rs` to test modules in isolation.
- **`test_unified_ui.sh`**: A shell script to spin up the entire ecosystem for integration testing.
- **`Cargo.toml`**: Defines project dependencies including `quinn` (QUIC), `reed-solomon-erasure` (Error Correction), and `axum` (API).
