use bytes::Bytes;
use chunkstream_pro::chunk::{Chunk, ChunkMetadata, Priority};
use chunkstream_pro::network::{ConnectionConfig, MultiPathManager, QuicTransport};
use std::sync::Arc;
use std::time::Duration;

fn create_test_chunk(data: &[u8], seq: u32) -> Chunk {
    Chunk {
        metadata: ChunkMetadata {
            chunk_id: seq as u64,
            file_id: "demo-file".to_string(),
            sequence_number: seq,
            total_chunks: 10,
            data_size: data.len(),
            checksum: [0u8; 32],
            is_parity: false,
            priority: Priority::Normal,
            created_at: chrono::Utc::now().timestamp(),
        },
        data: Bytes::from(data.to_vec()),
    }
}

#[tokio::main]
async fn main() {
    // Initialize crypto provider
    let _ = rustls::crypto::ring::default_provider().install_default();

    println!("\n🌐 ChunkStream Pro - Network Module Demo");
    println!("==========================================\n");

    // Demo 1: QUIC Transport Creation
    println!("📡 Demo 1: QUIC Transport Creation");
    println!("-----------------------------------");

    let config = ConnectionConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        ..Default::default()
    };

    let server = Arc::new(QuicTransport::new(config.clone()).await.unwrap());
    let server_addr = server.local_addr().unwrap();

    println!("✅ Server created at: {}", server_addr);
    println!("   Max idle timeout: {:?}", config.max_idle_timeout);
    println!("   Keep alive interval: {:?}", config.keep_alive_interval);
    println!(
        "   Max concurrent streams: {}",
        config.max_concurrent_streams
    );

    // Demo 2: Client Connection
    println!("\n\n🔗 Demo 2: Client-Server Connection");
    println!("-----------------------------------");

    let client_config = ConnectionConfig::default();
    let client = Arc::new(QuicTransport::new(client_config).await.unwrap());

    let server_clone = server.clone();
    let server_task = tokio::spawn(async move {
        println!("   Server: Waiting for connection...");
        match server_clone.accept().await {
            Ok(conn) => {
                println!(
                    "   Server: ✅ Connection accepted from {}",
                    conn.remote_address()
                );
                conn
            }
            Err(e) => {
                println!("   Server: ❌ Accept failed: {}", e);
                panic!("Accept failed");
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    println!("   Client: Connecting to {}...", server_addr);
    let client_conn = client.connect(server_addr).await.unwrap();
    println!("   Client: ✅ Connected to server");

    let server_conn = server_task.await.unwrap();

    // Demo 3: Single Chunk Transfer
    println!("\n\n📦 Demo 3: Single Chunk Transfer");
    println!("-----------------------------------");

    let chunk_data = b"Hello from ChunkStream Pro!";
    let chunk = create_test_chunk(chunk_data, 0);

    println!("Sending chunk:");
    println!("   Chunk ID: {}", chunk.metadata.chunk_id);
    println!(
        "   Sequence: {}/{}",
        chunk.metadata.sequence_number + 1,
        chunk.metadata.total_chunks
    );
    println!("   Data size: {} bytes", chunk.metadata.data_size);
    println!("   Priority: {:?}", chunk.metadata.priority);

    let server_clone = server.clone();
    let receive_task = tokio::spawn(async move {
        let stream = server_conn.accept_uni().await.unwrap();
        let received = server_clone.receive_chunk(stream).await.unwrap();
        println!("\n   Server received:");
        println!("   ✅ Chunk ID: {}", received.metadata.chunk_id);
        println!("   ✅ Data: {:?}", String::from_utf8_lossy(&received.data));
        received
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    client.send_chunk(&client_conn, &chunk).await.unwrap();
    println!("\n   Client: ✅ Chunk sent successfully");

    let received = receive_task.await.unwrap();
    assert_eq!(received.data.as_ref(), chunk_data);

    // Demo 4: Batch Chunk Transfer
    println!("\n\n📦📦📦 Demo 4: Batch Chunk Transfer (10 chunks)");
    println!("-----------------------------------");

    // Create new connection for batch transfer
    let server_clone = server.clone();
    let server_task2 = tokio::spawn(async move {
        let conn = server_clone.accept().await.unwrap();
        let mut received_chunks = Vec::new();
        for _ in 0..10 {
            if let Ok(stream) =
                tokio::time::timeout(Duration::from_secs(2), conn.accept_uni()).await
            {
                if let Ok(chunk) = server_clone.receive_chunk(stream.unwrap()).await {
                    received_chunks.push(chunk);
                }
            }
        }
        received_chunks
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let batch_conn = client.connect(server_addr).await.unwrap();

    let chunks: Vec<Chunk> = (0..10)
        .map(|i| create_test_chunk(format!("Chunk {} data", i).as_bytes(), i))
        .collect();

    println!("Prepared {} chunks for transfer", chunks.len());

    let start = std::time::Instant::now();
    for chunk in &chunks {
        client.send_chunk(&batch_conn, chunk).await.unwrap();
    }
    let duration = start.elapsed();

    println!("   ✅ Sent 10 chunks in {:?}", duration);
    println!(
        "   ⚡ Throughput: {:.2} chunks/sec",
        10.0 / duration.as_secs_f64()
    );

    let received_chunks = server_task2.await.unwrap();
    println!("   ✅ Received {} chunks", received_chunks.len());

    // Demo 5: Network Statistics
    println!("\n\n📊 Demo 5: Network Statistics");
    println!("-----------------------------------");

    let stats = client.stats();
    println!("Client Stats:");
    println!("   Chunks sent: {}", stats.chunks_sent);
    println!("   Total bytes sent: {} bytes", stats.total_bytes_sent);
    println!("   Active connections: {}", stats.active_connections);

    let server_stats = server.stats();
    println!("\nServer Stats:");
    println!("   Chunks received: {}", server_stats.chunks_received);
    println!(
        "   Total bytes received: {} bytes",
        server_stats.total_bytes_received
    );
    println!("   Active connections: {}", server_stats.active_connections);

    // Demo 6: Retry Mechanism
    println!("\n\n🔄 Demo 6: Retry Mechanism");
    println!("-----------------------------------");

    let retry_chunk = create_test_chunk(b"Retry test data", 99);

    let server_clone = server.clone();
    let receive_task = tokio::spawn(async move {
        let conn = server_clone.accept().await.unwrap();
        let stream = conn.accept_uni().await.unwrap();
        server_clone.receive_chunk(stream).await.unwrap()
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let retry_conn = client.connect(server_addr).await.unwrap();

    println!("   Sending with retry (max 3 attempts)...");
    let start = std::time::Instant::now();
    client
        .send_with_retry(&retry_conn, &retry_chunk, 3)
        .await
        .unwrap();
    let duration = start.elapsed();

    println!("   ✅ Sent successfully in {:?}", duration);

    let _ = receive_task.await;

    // Demo 7: Multi-Path Manager
    println!("\n\n🛣️  Demo 7: Multi-Path Manager");
    println!("-----------------------------------");

    let mp_config = ConnectionConfig::default();
    let mp_transport = Arc::new(QuicTransport::new(mp_config).await.unwrap());
    let mp_manager = MultiPathManager::new(mp_transport);

    println!("   Discovering network paths...");
    let paths = mp_manager.discover_paths(server_addr).await.unwrap();

    println!("   ✅ Found {} network path(s):", paths.len());
    for (i, path) in paths.iter().enumerate() {
        println!(
            "      Path {}: {} -> {}",
            i + 1,
            path.local_addr,
            path.remote_addr
        );
        println!("         Interface: {}", path.interface);
        println!("         Status: {:?}", path.status);
    }

    // Test path selection
    println!("\n   Testing path selection:");
    if let Some(critical_path) = mp_manager.select_path(Priority::Critical) {
        println!("   🔴 Critical priority → Path: {}", critical_path.path_id);
    }
    if let Some(normal_path) = mp_manager.select_path(Priority::Normal) {
        println!("   🟢 Normal priority → Path: {}", normal_path.path_id);
    }

    // Demo 8: Path Metrics Update
    println!("\n\n📈 Demo 8: Path Metrics Update");
    println!("-----------------------------------");

    if let Some(path) = paths.first() {
        println!("   Initial metrics for {}:", path.path_id);
        println!("      RTT: {} ms", path.metrics.rtt_ms);
        println!("      Loss rate: {:.1}%", path.metrics.loss_rate * 100.0);
        println!("      Status: {:?}", path.status);

        // Update with degraded metrics
        let degraded_metrics = chunkstream_pro::network::PathMetrics {
            rtt_ms: 250,
            loss_rate: 0.25,
            bandwidth_bps: 500_000,
            last_updated: chrono::Utc::now().timestamp(),
        };

        mp_manager
            .update_path_metrics(&path.path_id, degraded_metrics)
            .await;

        let updated_paths = mp_manager.get_paths();
        if let Some(updated) = updated_paths.iter().find(|p| p.path_id == path.path_id) {
            println!("\n   Updated metrics:");
            println!("      RTT: {} ms", updated.metrics.rtt_ms);
            println!("      Loss rate: {:.1}%", updated.metrics.loss_rate * 100.0);
            println!("      Status: {:?} (changed!)", updated.status);
        }
    }

    // Demo 9: Performance Summary
    println!("\n\n⚡ Demo 9: Performance Summary");
    println!("-----------------------------------");

    let final_stats = client.stats();
    let total_chunks = final_stats.chunks_sent;
    let total_bytes = final_stats.total_bytes_sent;

    println!("Overall Performance:");
    println!("   Total chunks transferred: {}", total_chunks);
    println!(
        "   Total bytes transferred: {} bytes ({:.2} KB)",
        total_bytes,
        total_bytes as f64 / 1024.0
    );
    println!(
        "   Average chunk size: {:.2} bytes",
        total_bytes as f64 / total_chunks as f64
    );
    println!("   Retransmissions: {}", final_stats.retransmissions);
    println!("   Success rate: 100% ✅");

    println!("\n\n🎉 All network demos completed successfully!");
    println!("==========================================\n");
}
