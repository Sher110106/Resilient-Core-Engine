use chunkstream_pro::api::create_api_server;
use chunkstream_pro::chunk::ChunkManager;
use chunkstream_pro::coordinator::TransferCoordinator;
use chunkstream_pro::integrity::IntegrityVerifier;
use chunkstream_pro::network::{ConnectionConfig, QuicTransport};
use chunkstream_pro::priority::PriorityQueue;
use chunkstream_pro::session::SessionStore;

#[tokio::main]
async fn main() {
    println!("\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║          ChunkStream Pro - File Transfer Server                 ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    println!("🚀 Initializing system components...\n");

    // Initialize Chunk Manager
    println!("📦 Chunk Manager: 256KB chunks, 10 data + 3 parity shards");
    let chunk_manager = ChunkManager::new(256 * 1024, 10, 3)
        .expect("Failed to create chunk manager");

    // Initialize Integrity Verifier
    println!("🔒 Integrity Verifier: BLAKE3 hashing");
    let verifier = IntegrityVerifier;

    // Initialize QUIC Transport
    println!("🌐 Network Engine: QUIC transport with TLS 1.3");
    let config = ConnectionConfig::default();
    let transport = QuicTransport::new(config)
        .await
        .expect("Failed to create QUIC transport");

    // Initialize Priority Queue
    println!("⚡ Priority Queue: 1M capacity, 3-level system");
    let queue = PriorityQueue::new(1_000_000);

    // Initialize Session Store
    println!("💾 Session Store: In-memory SQLite database");
    let session_store = SessionStore::new_in_memory()
        .await
        .expect("Failed to create session store");

    // Create Transfer Coordinator
    println!("🎯 Transfer Coordinator: Orchestrating all modules");
    let coordinator = TransferCoordinator::new(
        chunk_manager,
        verifier,
        transport,
        queue,
        session_store,
    );

    // Create API server
    println!("🌐 API Layer: REST + WebSocket endpoints");
    let app = create_api_server(coordinator);

    // Bind server
    println!("\n📡 Starting server...");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind to port 3000");

    println!("\n✅ ChunkStream Pro Server is running!\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📍 Server Address:  http://localhost:3000");
    println!("🏥 Health Check:    http://localhost:3000/health");
    println!("📡 REST API:        http://localhost:3000/api/v1/transfers");
    println!("🔌 WebSocket:       ws://localhost:3000/ws");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\n📚 API Endpoints:");
    println!("   POST   /api/v1/transfers              - Start new transfer");
    println!("   GET    /api/v1/transfers              - List all transfers");
    println!("   GET    /api/v1/transfers/:id          - Get transfer state");
    println!("   GET    /api/v1/transfers/:id/progress - Get transfer progress");
    println!("   POST   /api/v1/transfers/:id/pause    - Pause transfer");
    println!("   POST   /api/v1/transfers/:id/resume   - Resume transfer");
    println!("   POST   /api/v1/transfers/:id/cancel   - Cancel transfer");
    println!("\n💡 Frontend: Open http://localhost:3001 in your browser");
    println!("   (Make sure to start the React app: cd frontend && npm start)");
    println!("\n🛑 Press Ctrl+C to stop the server\n");

    // Start serving
    axum::serve(listener, app)
        .await
        .expect("Server error");
}
