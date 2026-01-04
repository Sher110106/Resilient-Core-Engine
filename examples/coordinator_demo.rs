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
    println!("\n🚀 ChunkStream Pro - Transfer Coordinator Demo");
    println!("================================================\n");

    // Demo 1: Coordinator Creation
    println!("📦 Demo 1: Creating Transfer Coordinator");
    println!("----------------------------------");

    let chunk_manager = ChunkManager::new(256 * 1024, 10, 3).unwrap();
    let verifier = IntegrityVerifier;
    let config = ConnectionConfig::default();
    let transport = QuicTransport::new(config).await.unwrap();
    let queue = PriorityQueue::new(1_000_000);
    let session_store = SessionStore::new_in_memory().await.unwrap();

    let coordinator =
        TransferCoordinator::new(chunk_manager, verifier, transport, queue, session_store);

    println!("✅ Coordinator created successfully");
    println!("   Components:");
    println!("   - Chunk Manager: 256KB chunks, 10 data + 3 parity");
    println!("   - Integrity Verifier: BLAKE3 hashing");
    println!("   - QUIC Transport: Enabled");
    println!("   - Priority Queue: 1M capacity");
    println!("   - Session Store: In-memory SQLite");
    println!("   Active transfers: {}", coordinator.list_active().len());

    // Demo 2: Starting a File Transfer
    println!("\n\n📤 Demo 2: Starting a File Transfer");
    println!("----------------------------------");

    // Create a test file
    let mut test_file1 = NamedTempFile::new().unwrap();
    let test_data: Vec<u8> = (0..2048).map(|i| (i % 256) as u8).collect();
    test_file1.write_all(&test_data).unwrap();
    test_file1.flush().unwrap();

    let file_path1 = test_file1.path().to_path_buf();
    println!(
        "Created test file: {} ({} bytes)",
        file_path1.display(),
        test_data.len()
    );

    let session_id1 = coordinator
        .send_file(file_path1.clone(), Priority::Normal, None)
        .await
        .unwrap();
    println!("✅ Transfer started!");
    println!("   Session ID: {}", session_id1);
    println!("   Priority: Normal");

    sleep(Duration::from_millis(50)).await;

    // Demo 3: Monitoring Transfer Progress
    println!("\n\n📊 Demo 3: Monitoring Transfer Progress");
    println!("----------------------------------");

    sleep(Duration::from_millis(100)).await;

    match coordinator.get_progress(&session_id1).await {
        Ok(progress) => {
            println!("Transfer Progress:");
            println!("   Session: {}", progress.session_id);
            println!("   Status: {:?}", progress.status);
            println!("   Progress: {:.1}%", progress.progress_percent);
            println!(
                "   Chunks: {}/{}",
                progress.completed_chunks, progress.total_chunks
            );
            println!(
                "   Bytes: {}/{}",
                progress.bytes_transferred, progress.total_bytes
            );
        }
        Err(e) => {
            println!("⚠️  Could not get progress: {}", e);
        }
    }

    // Demo 4: Multiple Concurrent Transfers
    println!("\n\n🔀 Demo 4: Multiple Concurrent Transfers");
    println!("----------------------------------");

    // Create additional test files
    let mut test_file2 = NamedTempFile::new().unwrap();
    test_file2.write_all(&vec![1u8; 4096]).unwrap();
    test_file2.flush().unwrap();

    let mut test_file3 = NamedTempFile::new().unwrap();
    test_file3.write_all(&vec![2u8; 8192]).unwrap();
    test_file3.flush().unwrap();

    let session_id2 = coordinator
        .send_file(test_file2.path().to_path_buf(), Priority::High, None)
        .await
        .unwrap();

    let session_id3 = coordinator
        .send_file(test_file3.path().to_path_buf(), Priority::Critical, None)
        .await
        .unwrap();

    println!("Started 3 concurrent transfers:");
    println!("   1. {} (Normal priority)", session_id1);
    println!("   2. {} (High priority)", session_id2);
    println!("   3. {} (Critical priority)", session_id3);

    let active = coordinator.list_active();
    println!("\nActive transfers: {}", active.len());

    // Demo 5: Transfer State Monitoring
    println!("\n\n🔍 Demo 5: Transfer State Monitoring");
    println!("----------------------------------");

    sleep(Duration::from_millis(50)).await;

    for session_id in [&session_id1, &session_id2, &session_id3] {
        if let Some(state) = coordinator.get_state(session_id) {
            let status = match state {
                chunkstream_pro::coordinator::TransferState::Idle => "Idle",
                chunkstream_pro::coordinator::TransferState::Preparing => "Preparing",
                chunkstream_pro::coordinator::TransferState::Transferring { .. } => "Transferring",
                chunkstream_pro::coordinator::TransferState::Paused { .. } => "Paused",
                chunkstream_pro::coordinator::TransferState::Completing => "Completing",
                chunkstream_pro::coordinator::TransferState::Completed => "Completed",
                chunkstream_pro::coordinator::TransferState::Failed { .. } => "Failed",
            };
            println!("   {} -> {}", &session_id[..8], status);
        }
    }

    // Demo 6: Pausing a Transfer
    println!("\n\n⏸️  Demo 6: Pausing a Transfer");
    println!("----------------------------------");

    if coordinator.get_state(&session_id2).is_some() {
        println!("Pausing transfer: {}", session_id2);

        match coordinator.pause_transfer(&session_id2).await {
            Ok(_) => {
                println!("✅ Transfer paused successfully");

                if let Some(state) = coordinator.get_state(&session_id2) {
                    println!("   New state: {:?}", state);
                }
            }
            Err(e) => {
                println!("⚠️  Could not pause: {}", e);
            }
        }
    } else {
        println!("⚠️  Transfer not found or already completed");
    }

    sleep(Duration::from_millis(100)).await;

    // Demo 7: Resuming a Paused Transfer
    println!("\n\n▶️  Demo 7: Resuming a Paused Transfer");
    println!("----------------------------------");

    println!("Resuming transfer: {}", session_id2);

    match coordinator.resume_transfer(&session_id2).await {
        Ok(_) => {
            println!("✅ Transfer resumed successfully");

            sleep(Duration::from_millis(50)).await;

            if let Some(state) = coordinator.get_state(&session_id2) {
                println!("   New state: {:?}", state);
            }
        }
        Err(e) => {
            println!("⚠️  Could not resume: {}", e);
        }
    }

    // Demo 8: Cancelling a Transfer
    println!("\n\n❌ Demo 8: Cancelling a Transfer");
    println!("----------------------------------");

    // Create another transfer to cancel
    let mut test_file4 = NamedTempFile::new().unwrap();
    test_file4.write_all(&vec![3u8; 16384]).unwrap();
    test_file4.flush().unwrap();

    let session_id4 = coordinator
        .send_file(test_file4.path().to_path_buf(), Priority::Normal, None)
        .await
        .unwrap();

    println!("Started transfer: {}", session_id4);

    sleep(Duration::from_millis(50)).await;

    println!("Cancelling transfer...");
    match coordinator.cancel_transfer(&session_id4).await {
        Ok(_) => {
            println!("✅ Transfer cancelled successfully");
            println!("   Transfer removed from active list");
        }
        Err(e) => {
            println!("⚠️  Could not cancel: {}", e);
        }
    }

    // Demo 9: Checking Progress of All Transfers
    println!("\n\n📈 Demo 9: Progress Summary");
    println!("----------------------------------");

    sleep(Duration::from_millis(200)).await;

    for session_id in [&session_id1, &session_id2, &session_id3] {
        match coordinator.get_progress(session_id).await {
            Ok(progress) => {
                println!("Session {}:", &session_id[..8]);
                println!("   Status: {:?}", progress.status);
                println!("   Progress: {:.1}%", progress.progress_percent);
                println!(
                    "   Chunks: {}/{}",
                    progress.completed_chunks, progress.total_chunks
                );
            }
            Err(e) => {
                println!("   Error: {}", e);
            }
        }
        println!();
    }

    // Demo 10: System Statistics
    println!("\n📊 Demo 10: System Statistics");
    println!("----------------------------------");

    let active_count = coordinator.list_active().len();

    println!("Coordinator Statistics:");
    println!("   Active transfers: {}", active_count);
    println!("   Completed demo scenarios: 10");
    println!("   System state: Operational");

    println!("\n\n🎉 All coordinator demos completed successfully!");
    println!("================================================\n");
}
