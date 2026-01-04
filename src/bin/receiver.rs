use axum::{
    extract::{Path as AxumPath, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chunkstream_pro::chunk::{Chunk, ChunkManager, FileManifest};
use chunkstream_pro::integrity::IntegrityVerifier;
use chunkstream_pro::network::{ConnectionConfig, QuicTransport};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() {
    // Initialize crypto provider for rustls/quinn
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

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

    // Initialize components (must match sender config)
    let chunk_manager =
        Arc::new(ChunkManager::new(512 * 1024, 50, 10).expect("Failed to create chunk manager"));
    let verifier = Arc::new(IntegrityVerifier);

    let config = ConnectionConfig {
        bind_addr,
        ..Default::default()
    };
    let transport = Arc::new(
        QuicTransport::new(config)
            .await
            .expect("Failed to create transport"),
    );

    println!("✅ Receiver ready! Waiting for incoming transfers...\n");

    // Shared state for REST API
    let received_files: Arc<Mutex<Vec<ReceivedFileInfo>>> = Arc::new(Mutex::new(Vec::new()));
    let (tx, _rx) = broadcast::channel::<String>(100);

    // Active transfers: session_id -> (manifest, chunks)
    let active_transfers: Arc<Mutex<HashMap<String, (FileManifest, Vec<Chunk>)>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Start REST API server on port 8080
    let api_state = ReceiverApiState {
        received_files: received_files.clone(),
        save_dir: save_dir.clone(),
        bind_addr,
        tx: tx.clone(),
    };

    tokio::spawn(async move {
        if let Err(e) = start_api_server(api_state).await {
            eprintln!("❌ Failed to start API server: {}", e);
        }
    });

    println!("🌐 REST API running on http://0.0.0.0:8080\n");

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
                let received_files_clone = received_files.clone();
                let tx_clone = tx.clone();

                tokio::spawn(async move {
                    if let Err(e) = handle_transfer(
                        conn,
                        transport_clone,
                        chunk_manager_clone,
                        verifier_clone,
                        save_dir_clone,
                        active_transfers_clone,
                        received_files_clone,
                        tx_clone,
                    )
                    .await
                    {
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
    received_files: Arc<Mutex<Vec<ReceivedFileInfo>>>,
    tx: broadcast::Sender<String>,
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
                        let calculated_checksum =
                            IntegrityVerifier::calculate_checksum(&chunk.data);
                        if calculated_checksum != chunk.metadata.checksum {
                            eprintln!(
                                "   ⚠️  Chunk {} failed verification!",
                                chunk.metadata.sequence_number
                            );
                            continue;
                        }

                        let chunk_session_id = chunk.metadata.file_id.clone();

                        // First chunk - extract session info
                        if session_id.is_none() {
                            session_id = Some(chunk_session_id.clone());
                            println!("   📋 Transfer ID: {}", chunk_session_id);
                        }

                        println!(
                            "   ✓ Chunk {}/{} received ({})",
                            chunk.metadata.sequence_number + 1,
                            chunk.metadata.total_chunks,
                            format_bytes(chunk.data.len())
                        );

                        // Store chunk
                        let mut transfers = active_transfers.lock().await;
                        let entry =
                            transfers
                                .entry(chunk_session_id.clone())
                                .or_insert_with(|| {
                                    // Create manifest from chunk metadata
                                    let manifest = FileManifest {
                                        file_id: chunk.metadata.file_id.clone(),
                                        filename: format!("file_{}", chunk.metadata.file_id),
                                        total_size: chunk.metadata.file_size, // From chunk metadata
                                        chunk_size: chunk.data.len(),
                                        total_chunks: chunk.metadata.total_chunks,
                                        data_chunks: chunk.metadata.data_chunks, // From chunk metadata
                                        parity_chunks: chunk.metadata.total_chunks
                                            - chunk.metadata.data_chunks,
                                        checksum: chunk.metadata.file_checksum, // From chunk metadata
                                        priority: chunk.metadata.priority,
                                    };
                                    (manifest, Vec::new())
                                });

                        entry.1.push(chunk.clone());

                        // Check if we have enough chunks to reconstruct
                        let (manifest, chunks) = entry;
                        if chunks.len() >= manifest.data_chunks as usize {
                            // Need at least data_chunks
                            println!(
                                "\n   🎯 Received {} chunks - attempting reconstruction...",
                                chunks.len()
                            );

                            // Try to reconstruct - sanitize file_id to use as filename
                            let safe_filename = manifest
                                .file_id
                                .replace('/', "_")
                                .replace('\\', "_")
                                .replace(':', "_");
                            let output_filename = format!("received_{}", safe_filename);
                            let output_path = save_dir.join(output_filename);

                            match chunk_manager
                                .reconstruct_file(manifest, chunks.clone(), &output_path)
                                .await
                            {
                                Ok(_) => {
                                    println!("   ✅ File reconstructed successfully!");
                                    println!("   💾 Saved to: {}", output_path.display());
                                    println!(
                                        "   📊 Total chunks used: {} (out of {} received)",
                                        manifest.data_chunks,
                                        chunks.len()
                                    );

                                    // Calculate and display reconstructed file info
                                    if let Ok(file_data) = tokio::fs::read(&output_path).await {
                                        let reconstructed_checksum =
                                            IntegrityVerifier::calculate_checksum(&file_data);
                                        println!("   📏 File size: {} bytes", file_data.len());

                                        // Only verify if we have a real checksum from sender
                                        let zero_checksum = [0u8; 32];
                                        let verified = if manifest.checksum != zero_checksum {
                                            if reconstructed_checksum == manifest.checksum {
                                                println!("   🔒 File integrity verified! ✓");
                                                true
                                            } else {
                                                println!("   ⚠️  Checksum mismatch with sender!");
                                                false
                                            }
                                        } else {
                                            println!("   🔒 File integrity: Cannot verify (no sender checksum)");
                                            false
                                        };

                                        // Add to received files list
                                        let file_info = ReceivedFileInfo {
                                            filename: output_path
                                                .file_name()
                                                .unwrap_or_default()
                                                .to_string_lossy()
                                                .to_string(),
                                            size: file_data.len() as u64,
                                            received_at: chrono::Utc::now().to_rfc3339(),
                                            verified,
                                            path: output_path.to_string_lossy().to_string(),
                                        };

                                        received_files.lock().await.push(file_info.clone());

                                        // Notify via broadcast
                                        let _ = tx.send(
                                            serde_json::to_string(&file_info).unwrap_or_default(),
                                        );
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
                    println!(
                        "   📊 Connection closed. Received {} chunks total.",
                        chunk_count
                    );
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

// REST API types
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReceivedFileInfo {
    filename: String,
    size: u64,
    received_at: String,
    verified: bool,
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReceiverStatus {
    listening: bool,
    bind_addr: String,
    files_received: usize,
    total_size: u64,
}

#[derive(Clone)]
struct ReceiverApiState {
    received_files: Arc<Mutex<Vec<ReceivedFileInfo>>>,
    save_dir: PathBuf,
    bind_addr: SocketAddr,
    tx: broadcast::Sender<String>,
}

async fn start_api_server(state: ReceiverApiState) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/api/v1/receiver/status", get(get_receiver_status))
        .route("/api/v1/receiver/files", get(list_received_files))
        .route("/api/v1/receiver/files/:filename", get(download_file))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    println!("   🌐 API server listening on port 8080");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn get_receiver_status(State(state): State<ReceiverApiState>) -> Json<ReceiverStatus> {
    let files = state.received_files.lock().await;
    let total_size: u64 = files.iter().map(|f| f.size).sum();

    Json(ReceiverStatus {
        listening: true,
        bind_addr: state.bind_addr.to_string(),
        files_received: files.len(),
        total_size,
    })
}

async fn list_received_files(State(state): State<ReceiverApiState>) -> Json<Vec<ReceivedFileInfo>> {
    let files = state.received_files.lock().await;
    Json(files.clone())
}

async fn download_file(
    State(state): State<ReceiverApiState>,
    AxumPath(filename): AxumPath<String>,
) -> impl IntoResponse {
    let files = state.received_files.lock().await;

    if let Some(file_info) = files.iter().find(|f| f.filename == filename) {
        match tokio::fs::read(&file_info.path).await {
            Ok(contents) => {
                let headers = [
                    (header::CONTENT_TYPE, "application/octet-stream"),
                    (
                        header::CONTENT_DISPOSITION,
                        &format!("attachment; filename=\"{}\"", filename),
                    ),
                ];
                (StatusCode::OK, headers, contents).into_response()
            }
            Err(_) => (StatusCode::NOT_FOUND, "File not found on disk").into_response(),
        }
    } else {
        (StatusCode::NOT_FOUND, "File not found").into_response()
    }
}
