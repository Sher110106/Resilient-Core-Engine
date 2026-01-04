use chunkstream_pro::chunk::{Chunk, ChunkManager, Priority};
use chunkstream_pro::coordinator::TransferCoordinator;
use chunkstream_pro::integrity::IntegrityVerifier;
use chunkstream_pro::network::{ConnectionConfig, QuicTransport};
use chunkstream_pro::priority::PriorityQueue;
use chunkstream_pro::session::{SessionState, SessionStore};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs;
use tokio::time::{sleep, Duration};

/// Test full sender-to-receiver workflow with actual file transfer
#[tokio::test]
async fn test_full_transfer_workflow() {
    // Initialize crypto provider
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    println!("\n=== Testing Full Transfer Workflow ===\n");

    // Create temporary directories
    let temp_dir = TempDir::new().unwrap();
    let sender_dir = temp_dir.path().join("sender");
    let receiver_dir = temp_dir.path().join("receiver");
    let db_path = temp_dir.path().join("test.db");

    fs::create_dir_all(&sender_dir).await.unwrap();
    fs::create_dir_all(&receiver_dir).await.unwrap();

    // Create test file (1MB)
    let test_file = sender_dir.join("test_1mb.bin");
    let test_data: Vec<u8> = (0..1024 * 1024).map(|i| (i % 256) as u8).collect();
    fs::write(&test_file, &test_data).await.unwrap();
    println!("✓ Created test file: {} bytes", test_data.len());

    // Calculate original checksum
    let original_checksum = IntegrityVerifier::calculate_checksum(&test_data);
    println!("✓ Original checksum: {:?}", hex::encode(original_checksum));

    // Setup receiver
    let receiver_config = ConnectionConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(), // Random port
        ..Default::default()
    };
    let receiver_transport = Arc::new(QuicTransport::new(receiver_config).await.unwrap());
    let receiver_addr = receiver_transport.local_addr().unwrap();
    println!("✓ Receiver listening on: {}", receiver_addr);

    let receiver_chunk_manager = Arc::new(ChunkManager::new(256 * 1024, 10, 3).unwrap());
    let receiver_output = receiver_dir.join("received_test_1mb.bin");

    // Spawn receiver task
    let receiver_transport_clone = receiver_transport.clone();
    let receiver_chunk_manager_clone = receiver_chunk_manager.clone();
    let receiver_output_clone = receiver_output.clone();

    let receiver_handle = tokio::spawn(async move {
        println!("🔵 Receiver: Waiting for connection...");
        let conn = receiver_transport_clone.accept().await.unwrap();
        println!(
            "🔵 Receiver: Connection accepted from {}",
            conn.remote_address()
        );

        let mut chunks = Vec::new();
        let mut manifest = None;

        loop {
            match conn.accept_uni().await {
                Ok(recv_stream) => {
                    match receiver_transport_clone.receive_chunk(recv_stream).await {
                        Ok(chunk) => {
                            println!(
                                "🔵 Receiver: Chunk {}/{} received",
                                chunk.metadata.sequence_number + 1,
                                chunk.metadata.total_chunks
                            );

                            if manifest.is_none() {
                                manifest = Some(chunkstream_pro::chunk::FileManifest {
                                    file_id: chunk.metadata.file_id.clone(),
                                    filename: "test_1mb.bin".to_string(),
                                    total_size: chunk.metadata.file_size,
                                    chunk_size: chunk.data.len(),
                                    total_chunks: chunk.metadata.total_chunks,
                                    data_chunks: chunk.metadata.data_chunks,
                                    parity_chunks: chunk.metadata.total_chunks
                                        - chunk.metadata.data_chunks,
                                    checksum: chunk.metadata.file_checksum,
                                    priority: chunk.metadata.priority,
                                });
                            }

                            chunks.push(chunk);

                            if let Some(ref m) = manifest {
                                if chunks.len() >= m.data_chunks as usize {
                                    println!(
                                        "🔵 Receiver: Attempting reconstruction with {} chunks",
                                        chunks.len()
                                    );
                                    match receiver_chunk_manager_clone
                                        .reconstruct_file(m, chunks.clone(), &receiver_output_clone)
                                        .await
                                    {
                                        Ok(_) => {
                                            println!(
                                                "🔵 Receiver: ✅ File reconstructed successfully!"
                                            );
                                            break;
                                        }
                                        Err(e) => {
                                            println!(
                                                "🔵 Receiver: ⏳ Waiting for more chunks... ({})",
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("🔵 Receiver: ❌ Failed to receive chunk: {}", e);
                            break;
                        }
                    }
                }
                Err(_) => {
                    println!("🔵 Receiver: Connection closed");
                    break;
                }
            }
        }
    });

    // Give receiver time to start
    sleep(Duration::from_millis(500)).await;

    // Setup sender
    let sender_config = ConnectionConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        ..Default::default()
    };

    let sender_transport = QuicTransport::new(sender_config).await.unwrap();
    let sender_chunk_manager = ChunkManager::new(256 * 1024, 10, 3).unwrap();
    let session_store = SessionStore::new(db_path.to_str().unwrap()).await.unwrap();
    let priority_queue = PriorityQueue::new(1000);
    let verifier = IntegrityVerifier;

    let coordinator = TransferCoordinator::new(
        sender_chunk_manager,
        verifier,
        sender_transport,
        priority_queue,
        session_store,
    );

    println!("🟢 Sender: Starting transfer to {}", receiver_addr);

    // Start transfer
    let session_id = coordinator
        .send_file(test_file.clone(), Priority::High, Some(receiver_addr))
        .await
        .unwrap();

    println!(
        "🟢 Sender: Transfer started with session ID: {}",
        session_id
    );

    // Wait for receiver to finish
    tokio::time::timeout(Duration::from_secs(30), receiver_handle)
        .await
        .expect("Receiver timed out")
        .unwrap();

    // Verify received file
    println!("\n=== Verifying Transfer ===\n");

    assert!(receiver_output.exists(), "Received file should exist");

    let received_data = fs::read(&receiver_output).await.unwrap();
    println!("✓ Received file size: {} bytes", received_data.len());

    let received_checksum = IntegrityVerifier::calculate_checksum(&received_data);
    println!("✓ Received checksum: {:?}", hex::encode(received_checksum));

    assert_eq!(
        test_data.len(),
        received_data.len(),
        "File sizes should match"
    );
    assert_eq!(
        original_checksum, received_checksum,
        "Checksums should match"
    );
    assert_eq!(
        test_data, received_data,
        "File contents should match exactly"
    );

    println!("\n✅ Transfer workflow test PASSED!\n");
}

/// Test with different file sizes
#[tokio::test]
async fn test_multiple_file_sizes() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    println!("\n=== Testing Multiple File Sizes ===\n");

    let temp_dir = TempDir::new().unwrap();
    let chunk_manager = Arc::new(ChunkManager::new(512 * 1024, 50, 10).unwrap());

    let test_cases = vec![
        ("100KB", 100 * 1024),
        ("500KB", 500 * 1024),
        ("1MB", 1024 * 1024),
        ("2MB", 2 * 1024 * 1024),
        ("5MB", 5 * 1024 * 1024),
    ];

    for (name, size) in test_cases {
        println!("Testing {} file...", name);

        let test_file = temp_dir.path().join(format!("test_{}.bin", name));
        let test_data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        fs::write(&test_file, &test_data).await.unwrap();

        let original_checksum = IntegrityVerifier::calculate_checksum(&test_data);
        let file_id = format!("test_{}", name);

        // Split file
        let (manifest, chunks) = chunk_manager
            .split_file(&test_file, file_id, Priority::Normal)
            .await
            .unwrap();
        println!(
            "  ✓ Split into {} chunks ({} data + {} parity)",
            manifest.total_chunks, manifest.data_chunks, manifest.parity_chunks
        );

        // Reconstruct
        let output_file = temp_dir.path().join(format!("reconstructed_{}.bin", name));
        chunk_manager
            .reconstruct_file(&manifest, chunks, &output_file)
            .await
            .unwrap();

        // Verify
        let reconstructed_data = fs::read(&output_file).await.unwrap();
        let reconstructed_checksum = IntegrityVerifier::calculate_checksum(&reconstructed_data);

        assert_eq!(
            original_checksum, reconstructed_checksum,
            "{} checksum mismatch",
            name
        );
        assert_eq!(
            test_data.len(),
            reconstructed_data.len(),
            "{} size mismatch",
            name
        );
        println!("  ✅ {} test passed\n", name);
    }

    println!("✅ All file size tests PASSED!\n");
}

/// Test erasure coding with missing chunks
#[tokio::test]
async fn test_erasure_coding_with_packet_loss() {
    println!("\n=== Testing Erasure Coding (Packet Loss) ===\n");

    let temp_dir = TempDir::new().unwrap();
    let chunk_manager = Arc::new(ChunkManager::new(256 * 1024, 10, 3).unwrap());

    // Create 1MB test file
    let test_file = temp_dir.path().join("test.bin");
    let test_data: Vec<u8> = (0..1024 * 1024).map(|i| (i % 256) as u8).collect();
    fs::write(&test_file, &test_data).await.unwrap();

    let original_checksum = IntegrityVerifier::calculate_checksum(&test_data);

    // Split file
    let file_id = "test_erasure".to_string();
    let (manifest, chunks) = chunk_manager
        .split_file(&test_file, file_id, Priority::Normal)
        .await
        .unwrap();
    println!(
        "✓ Split into {} chunks ({} data + {} parity)",
        manifest.total_chunks, manifest.data_chunks, manifest.parity_chunks
    );

    // Simulate losing the last 3 chunks (should still work with 10 data chunks)
    let chunks_with_loss: Vec<_> = chunks
        .iter()
        .take(manifest.data_chunks as usize)
        .cloned()
        .collect();

    println!(
        "✓ Simulating packet loss: using only {} of {} chunks",
        chunks_with_loss.len(),
        chunks.len()
    );

    // Reconstruct with missing chunks
    let output_file = temp_dir.path().join("reconstructed.bin");
    chunk_manager
        .reconstruct_file(&manifest, chunks_with_loss, &output_file)
        .await
        .unwrap();

    // Verify
    let reconstructed_data = fs::read(&output_file).await.unwrap();
    let reconstructed_checksum = IntegrityVerifier::calculate_checksum(&reconstructed_data);

    assert_eq!(
        original_checksum, reconstructed_checksum,
        "Checksum should match even with packet loss"
    );
    assert_eq!(
        test_data.len(),
        reconstructed_data.len(),
        "Size should match"
    );

    println!("✅ Erasure coding test PASSED! (Recovered from packet loss)\n");
}

/// Test priority queue ordering
#[tokio::test]
async fn test_priority_queue() {
    println!("\n=== Testing Priority Queue ===\n");

    let queue = PriorityQueue::new(100);

    // Create mock chunks with different priorities
    let chunk1 = Chunk {
        metadata: chunkstream_pro::chunk::ChunkMetadata {
            file_id: "session1".to_string(),
            sequence_number: 0,
            total_chunks: 1,
            data_chunks: 1,
            checksum: [0u8; 32],
            file_size: 1024,
            file_checksum: [0u8; 32],
            priority: Priority::Normal,
        },
        data: vec![0u8; 256],
    };

    let chunk2 = Chunk {
        metadata: chunkstream_pro::chunk::ChunkMetadata {
            file_id: "session2".to_string(),
            sequence_number: 0,
            total_chunks: 1,
            data_chunks: 1,
            checksum: [0u8; 32],
            file_size: 1024,
            file_checksum: [0u8; 32],
            priority: Priority::Critical,
        },
        data: vec![0u8; 256],
    };

    let chunk3 = Chunk {
        metadata: chunkstream_pro::chunk::ChunkMetadata {
            file_id: "session3".to_string(),
            sequence_number: 0,
            total_chunks: 1,
            data_chunks: 1,
            checksum: [0u8; 32],
            file_size: 1024,
            file_checksum: [0u8; 32],
            priority: Priority::High,
        },
        data: vec![0u8; 256],
    };

    // Enqueue chunks
    queue.enqueue(chunk1).unwrap();
    queue.enqueue(chunk2).unwrap();
    queue.enqueue(chunk3).unwrap();

    // Dequeue and verify order (Critical > High > Normal)
    let dequeued1 = queue.dequeue().unwrap();
    assert_eq!(dequeued1.metadata.priority, Priority::Critical);
    println!("✓ 1st dequeue: Critical priority");

    let dequeued2 = queue.dequeue().unwrap();
    assert_eq!(dequeued2.metadata.priority, Priority::High);
    println!("✓ 2nd dequeue: High priority");

    let dequeued3 = queue.dequeue().unwrap();
    assert_eq!(dequeued3.metadata.priority, Priority::Normal);
    println!("✓ 3rd dequeue: Normal priority");

    println!("✅ Priority queue test PASSED!\n");
}

/// Test session store persistence
#[tokio::test]
async fn test_session_persistence() {
    println!("\n=== Testing Session Persistence ===\n");

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_session.db");

    // Create session store and save a session
    let store = SessionStore::new(db_path.to_str().unwrap()).await.unwrap();

    let manifest = chunkstream_pro::chunk::FileManifest {
        file_id: "test_file_id".to_string(),
        filename: "test.bin".to_string(),
        total_size: 1024 * 1024,
        chunk_size: 256 * 1024,
        total_chunks: 13,
        data_chunks: 10,
        parity_chunks: 3,
        checksum: [0u8; 32],
        priority: Priority::High,
    };

    let session = SessionState::new(
        "test-session-123".to_string(),
        "test_file_id".to_string(),
        manifest,
    );

    store.save(&session).await.unwrap();
    println!("✓ Session saved to database");

    // Load session
    let loaded = store.get("test-session-123").await.unwrap().unwrap();
    assert_eq!(loaded.session_id, session.session_id);
    assert_eq!(loaded.file_id, session.file_id);
    println!("✓ Session loaded from database");

    // List active sessions
    let active = store.list_active().await.unwrap();
    assert!(active.iter().any(|s| s.session_id == "test-session-123"));
    println!("✓ Active sessions listed");

    println!("✅ Session persistence test PASSED!\n");
}
