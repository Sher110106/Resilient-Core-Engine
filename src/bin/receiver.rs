use chunkstream_pro::chunk::{Chunk, ChunkManager, FileManifest};
use chunkstream_pro::integrity::IntegrityVerifier;
use chunkstream_pro::network::{ConnectionConfig, QuicTransport};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    println!("\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║        ChunkStream Pro - File Receiver Agent                    ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let bind_addr: SocketAddr = if args.len() > 1 {
        args[1].parse().expect("Invalid bind address")
    } else {
        "0.0.0.0:5001".parse().unwrap()
    };

    let save_dir = if args.len() > 2 {
        PathBuf::from(&args[2])
    } else {
        PathBuf::from("./received")
    };

    // Create save directory
    tokio::fs::create_dir_all(&save_dir)
        .await
        .expect("Failed to create save directory");

    println!("🚀 Starting receiver...\n");
    println!("📍 Bind Address:    {}", bind_addr);
    println!("💾 Save Directory:  {}", save_dir.display());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Initialize components
    let chunk_manager = Arc::new(ChunkManager::new(256 * 1024, 10, 3).expect("Failed to create chunk manager"));
    let verifier = Arc::new(IntegrityVerifier);
    
    let config = ConnectionConfig {
        bind_addr,
        ..Default::default()
    };
    let transport = Arc::new(QuicTransport::new(config).await.expect("Failed to create transport"));

    println!("✅ Receiver ready! Waiting for incoming transfers...\n");

    // Active transfers: session_id -> (manifest, chunks)
    let active_transfers: Arc<Mutex<HashMap<String, (FileManifest, Vec<Chunk>)>>> = 
        Arc::new(Mutex::new(HashMap::new()));

    // Accept connections loop
    loop {
        match transport.accept().await {
            Ok(conn) => {
                let remote_addr = conn.remote_address();
                println!("📡 New connection from: {}", remote_addr);

                let transport_clone = transport.clone();
                let chunk_manager_clone = chunk_manager.clone();
                let verifier_clone = verifier.clone();
                let save_dir_clone = save_dir.clone();
                let active_transfers_clone = active_transfers.clone();

                tokio::spawn(async move {
                    if let Err(e) = handle_transfer(
                        conn,
                        transport_clone,
                        chunk_manager_clone,
                        verifier_clone,
                        save_dir_clone,
                        active_transfers_clone,
                    ).await {
                        eprintln!("❌ Transfer failed from {}: {}", remote_addr, e);
                    }
                });
            }
            Err(e) => {
                eprintln!("❌ Failed to accept connection: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}

async fn handle_transfer(
    conn: quinn::Connection,
    transport: Arc<QuicTransport>,
    chunk_manager: Arc<ChunkManager>,
    verifier: Arc<IntegrityVerifier>,
    save_dir: PathBuf,
    active_transfers: Arc<Mutex<HashMap<String, (FileManifest, Vec<Chunk>)>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let remote_addr = conn.remote_address();
    println!("   📦 Receiving chunks from {}...", remote_addr);

    let mut session_id: Option<String> = None;
    let mut chunk_count = 0;

    // Receive all chunks from this connection
    loop {
        match conn.accept_uni().await {
            Ok(recv_stream) => {
                // Receive chunk
                match transport.receive_chunk(recv_stream).await {
                    Ok(chunk) => {
                        chunk_count += 1;
                        
                        // Verify chunk integrity
                        let calculated_checksum = IntegrityVerifier::calculate_checksum(&chunk.data);
                        if calculated_checksum != chunk.metadata.checksum {
                            eprintln!("   ⚠️  Chunk {} failed verification!", chunk.metadata.sequence_number);
                            continue;
                        }

                        let chunk_session_id = chunk.metadata.file_id.clone();
                        
                        // First chunk - extract session info
                        if session_id.is_none() {
                            session_id = Some(chunk_session_id.clone());
                            println!("   📋 Transfer ID: {}", chunk_session_id);
                        }

                        println!("   ✓ Chunk {}/{} received ({})", 
                            chunk.metadata.sequence_number + 1,
                            chunk.metadata.total_chunks,
                            format_bytes(chunk.data.len())
                        );

                        // Store chunk
                        let mut transfers = active_transfers.lock().await;
                        let entry = transfers.entry(chunk_session_id.clone()).or_insert_with(|| {
                            // Create manifest from first chunk
                            // Note: We don't have full manifest info, so use placeholders
                            let manifest = FileManifest {
                                file_id: chunk.metadata.file_id.clone(),
                                filename: format!("file_{}", chunk.metadata.file_id),
                                total_size: (chunk.data.len() * chunk.metadata.total_chunks as usize) as u64,  // Estimate
                                chunk_size: chunk.data.len(),  // From actual chunk data
                                total_chunks: chunk.metadata.total_chunks,
                                data_chunks: 10,  // Assume default 10+3 erasure coding
                                parity_chunks: 3,
                                checksum: [0u8; 32],  // Don't have file checksum yet
                                priority: chunk.metadata.priority,
                            };
                            (manifest, Vec::new())
                        });

                        entry.1.push(chunk.clone());

                        // Check if we have enough chunks to reconstruct
                        let (manifest, chunks) = entry;
                        if chunks.len() >= manifest.data_chunks as usize {  // Need at least data_chunks
                            println!("\n   🎯 Received {} chunks - attempting reconstruction...", chunks.len());
                            
                            // Try to reconstruct
                            let output_filename = format!("received_{}", manifest.file_id);
                            let output_path = save_dir.join(output_filename);
                            
                            match chunk_manager.reconstruct_file(manifest, chunks.clone(), &output_path).await {
                                Ok(_) => {
                                    println!("   ✅ File reconstructed successfully!");
                                    println!("   💾 Saved to: {}", output_path.display());
                                    println!("   📊 Total chunks received: {}", chunks.len());
                                    
                                    // Verify reconstructed file
                                    if let Ok(file_data) = tokio::fs::read(&output_path).await {
                                        let reconstructed_checksum = IntegrityVerifier::calculate_checksum(&file_data);
                                        if reconstructed_checksum == manifest.checksum {
                                            println!("   🔒 File integrity verified! ✓");
                                        } else {
                                            println!("   ⚠️  Warning: File checksum mismatch!");
                                        }
                                    }
                                    
                                    // Clean up
                                    transfers.remove(&chunk_session_id);
                                    break;
                                }
                                Err(e) => {
                                    println!("   ⏳ Waiting for more chunks... (error: {})", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("   ❌ Failed to receive chunk: {}", e);
                        break;
                    }
                }
            }
            Err(e) => {
                // Connection closed or error
                if chunk_count > 0 {
                    println!("   📊 Connection closed. Received {} chunks total.", chunk_count);
                }
                break;
            }
        }
    }

    Ok(())
}

fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
