use chunkstream_pro::api::{create_api_server, StartTransferRequest};
use chunkstream_pro::chunk::{ChunkManager, Priority};
use chunkstream_pro::coordinator::TransferCoordinator;
use chunkstream_pro::integrity::IntegrityVerifier;
use chunkstream_pro::network::{ConnectionConfig, QuicTransport};
use chunkstream_pro::priority::PriorityQueue;
use chunkstream_pro::session::SessionStore;
use std::io::Write;
use tempfile::NamedTempFile;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    println!("\n🌐 ChunkStream Pro - API Layer Demo");
    println!("====================================\n");

    // Demo 1: Creating the API Server
    println!("📦 Demo 1: Creating the API Server");
    println!("----------------------------------");

    let chunk_manager = ChunkManager::new(256 * 1024, 10, 3).unwrap();
    let verifier = IntegrityVerifier;
    let config = ConnectionConfig::default();
    let transport = QuicTransport::new(config).await.unwrap();
    let queue = PriorityQueue::new(1_000_000);
    let session_store = SessionStore::new_in_memory().await.unwrap();

    let coordinator =
        TransferCoordinator::new(chunk_manager, verifier, transport, queue, session_store);

    let _app = create_api_server(coordinator.clone());

    println!("✅ API server created successfully");
    println!("   Available endpoints:");
    println!("   - GET  /health");
    println!("   - POST /api/v1/transfers");
    println!("   - GET  /api/v1/transfers");
    println!("   - GET  /api/v1/transfers/:id");
    println!("   - POST /api/v1/transfers/:id/pause");
    println!("   - POST /api/v1/transfers/:id/resume");
    println!("   - POST /api/v1/transfers/:id/cancel");
    println!("   - GET  /api/v1/transfers/:id/progress");
    println!("   - GET  /ws (WebSocket endpoint)");

    // Demo 2: REST API - Starting a Transfer
    println!("\n\n📤 Demo 2: REST API - Starting a Transfer");
    println!("----------------------------------");

    // Create a test file
    let mut test_file = NamedTempFile::new().unwrap();
    let test_data: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();
    test_file.write_all(&test_data).unwrap();
    test_file.flush().unwrap();

    let file_path = test_file.path().to_path_buf();
    println!("Created test file: {}", file_path.display());
    println!("File size: {} bytes", test_data.len());

    let _request = StartTransferRequest {
        file_path: file_path.to_string_lossy().to_string(),
        priority: Priority::High,
        receiver_addr: None,
    };

    println!("\nSimulating REST API call:");
    println!("POST /api/v1/transfers");
    println!("Body: {{");
    println!("  \"file_path\": \"{}\",", file_path.display());
    println!("  \"priority\": \"High\"");
    println!("}}");

    // Simulate the API call by directly calling the coordinator
    let session_id = coordinator
        .send_file(file_path.clone(), Priority::High, None)
        .await
        .unwrap();

    println!("\n✅ Response:");
    println!("{{");
    println!("  \"session_id\": \"{}\",", session_id);
    println!(
        "  \"message\": \"Transfer started with session ID: {}\"",
        session_id
    );
    println!("}}");

    sleep(Duration::from_millis(100)).await;

    // Demo 3: REST API - Getting Transfer Progress
    println!("\n\n📊 Demo 3: REST API - Getting Transfer Progress");
    println!("----------------------------------");

    println!("Simulating REST API call:");
    println!("GET /api/v1/transfers/{}/progress", session_id);

    match coordinator.get_progress(&session_id).await {
        Ok(progress) => {
            println!("\n✅ Response:");
            println!("{{");
            println!("  \"session_id\": \"{}\",", progress.session_id);
            println!("  \"status\": \"{:?}\",", progress.status);
            println!("  \"progress_percent\": {:.1},", progress.progress_percent);
            println!("  \"completed_chunks\": {},", progress.completed_chunks);
            println!("  \"total_chunks\": {},", progress.total_chunks);
            println!("  \"bytes_transferred\": {},", progress.bytes_transferred);
            println!("  \"total_bytes\": {}", progress.total_bytes);
            println!("}}");
        }
        Err(e) => {
            println!("⚠️  Error: {}", e);
        }
    }

    // Demo 4: REST API - Listing All Transfers
    println!("\n\n📋 Demo 4: REST API - Listing All Transfers");
    println!("----------------------------------");

    // Start more transfers
    let mut test_file2 = NamedTempFile::new().unwrap();
    test_file2.write_all(&vec![1u8; 2048]).unwrap();
    test_file2.flush().unwrap();

    let session_id2 = coordinator
        .send_file(test_file2.path().to_path_buf(), Priority::Normal, None)
        .await
        .unwrap();

    println!("Simulating REST API call:");
    println!("GET /api/v1/transfers");

    let active_transfers = coordinator.list_active();
    println!("\n✅ Response:");
    println!("{{");
    println!("  \"active_transfers\": [");
    for (i, id) in active_transfers.iter().enumerate() {
        if i < active_transfers.len() - 1 {
            println!("    \"{}\",", id);
        } else {
            println!("    \"{}\"", id);
        }
    }
    println!("  ],");
    println!("  \"count\": {}", active_transfers.len());
    println!("}}");

    sleep(Duration::from_millis(100)).await;

    // Demo 5: REST API - Getting Transfer State
    println!("\n\n🔍 Demo 5: REST API - Getting Transfer State");
    println!("----------------------------------");

    println!("Simulating REST API call:");
    println!("GET /api/v1/transfers/{}", session_id);

    if let Some(state) = coordinator.get_state(&session_id) {
        let state_str = match state {
            chunkstream_pro::coordinator::TransferState::Idle => "Idle",
            chunkstream_pro::coordinator::TransferState::Preparing => "Preparing",
            chunkstream_pro::coordinator::TransferState::Transferring { .. } => "Transferring",
            chunkstream_pro::coordinator::TransferState::Paused { .. } => "Paused",
            chunkstream_pro::coordinator::TransferState::Completing => "Completing",
            chunkstream_pro::coordinator::TransferState::Completed => "Completed",
            chunkstream_pro::coordinator::TransferState::Failed { .. } => "Failed",
        };

        println!("\n✅ Response:");
        println!("{{");
        println!("  \"session_id\": \"{}\",", session_id);
        println!("  \"state\": \"{}\",", state_str);
        println!("  \"is_active\": {},", state.is_active());
        println!("  \"is_paused\": {},", state.is_paused());
        println!("  \"is_terminal\": {}", state.is_terminal());
        println!("}}");
    }

    // Demo 6: REST API - Pausing a Transfer
    println!("\n\n⏸️  Demo 6: REST API - Pausing a Transfer");
    println!("----------------------------------");

    if coordinator.get_state(&session_id2).is_some() {
        println!("Simulating REST API call:");
        println!("POST /api/v1/transfers/{}/pause", session_id2);

        match coordinator.pause_transfer(&session_id2).await {
            Ok(_) => {
                println!("\n✅ Response:");
                println!("{{");
                println!("  \"message\": \"Transfer {} paused\"", session_id2);
                println!("}}");
            }
            Err(e) => {
                println!("⚠️  Could not pause: {}", e);
            }
        }
    }

    sleep(Duration::from_millis(100)).await;

    // Demo 7: REST API - Resuming a Transfer
    println!("\n\n▶️  Demo 7: REST API - Resuming a Transfer");
    println!("----------------------------------");

    println!("Simulating REST API call:");
    println!("POST /api/v1/transfers/{}/resume", session_id2);

    match coordinator.resume_transfer(&session_id2).await {
        Ok(_) => {
            println!("\n✅ Response:");
            println!("{{");
            println!("  \"message\": \"Transfer {} resumed\"", session_id2);
            println!("}}");
        }
        Err(e) => {
            println!("⚠️  Could not resume: {}", e);
        }
    }

    // Demo 8: REST API - Cancelling a Transfer
    println!("\n\n❌ Demo 8: REST API - Cancelling a Transfer");
    println!("----------------------------------");

    // Create another transfer to cancel
    let mut test_file3 = NamedTempFile::new().unwrap();
    test_file3.write_all(&vec![2u8; 8192]).unwrap();
    test_file3.flush().unwrap();

    let session_id3 = coordinator
        .send_file(test_file3.path().to_path_buf(), Priority::Normal, None)
        .await
        .unwrap();

    sleep(Duration::from_millis(50)).await;

    println!("Simulating REST API call:");
    println!("POST /api/v1/transfers/{}/cancel", session_id3);

    match coordinator.cancel_transfer(&session_id3).await {
        Ok(_) => {
            println!("\n✅ Response:");
            println!("{{");
            println!("  \"message\": \"Transfer {} cancelled\"", session_id3);
            println!("}}");
        }
        Err(e) => {
            println!("⚠️  Error: {}", e);
        }
    }

    // Demo 9: WebSocket - Real-time Updates
    println!("\n\n📡 Demo 9: WebSocket - Real-time Updates");
    println!("----------------------------------");

    println!("WebSocket connection: ws://localhost:3000/ws");
    println!("\nSimulated real-time messages:");

    for i in 1..=3 {
        sleep(Duration::from_millis(300)).await;

        for session_id in coordinator.list_active() {
            if let Ok(progress) = coordinator.get_progress(&session_id).await {
                println!("\n[Message {}] TransferProgress:", i);
                println!("{{");
                println!("  \"type\": \"TransferProgress\",");
                println!("  \"data\": {{");
                println!("    \"session_id\": \"{}\",", &session_id[..8]);
                println!(
                    "    \"progress_percent\": {:.1},",
                    progress.progress_percent
                );
                println!("    \"status\": \"{:?}\"", progress.status);
                println!("  }}");
                println!("}}");
            }
        }
    }

    // Demo 10: API Server Summary
    println!("\n\n📊 Demo 10: API Server Summary");
    println!("----------------------------------");

    let active_count = coordinator.list_active().len();

    println!("API Server Statistics:");
    println!("   Active transfers: {}", active_count);
    println!("   Supported protocols:");
    println!("     - REST API (JSON)");
    println!("     - WebSocket (real-time)");
    println!("   Features:");
    println!("     ✓ Start/pause/resume/cancel transfers");
    println!("     ✓ Progress monitoring");
    println!("     ✓ Transfer state management");
    println!("     ✓ Real-time WebSocket updates");

    println!("\n\n🎉 All API demos completed successfully!");
    println!("====================================\n");
}
