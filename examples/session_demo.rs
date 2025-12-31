use chunkstream_pro::chunk::{FileManifest, Priority};
use chunkstream_pro::session::{SessionState, SessionStatus, SessionStore};

fn create_test_manifest(filename: &str, total_chunks: u32) -> FileManifest {
    FileManifest {
        file_id: format!("file-{}", filename),
        filename: filename.to_string(),
        total_size: (total_chunks as u64) * 256 * 1024,
        chunk_size: 256 * 1024,
        total_chunks,
        data_chunks: (total_chunks as f32 * 0.77) as u32, // ~77% data chunks
        parity_chunks: total_chunks - (total_chunks as f32 * 0.77) as u32,
        priority: Priority::Normal,
        checksum: [0u8; 32],
    }
}

#[tokio::main]
async fn main() {
    println!("\n💾 ChunkStream Pro - Session Store Module Demo");
    println!("===============================================\n");

    // Demo 1: Session Store Creation
    println!("📦 Demo 1: Session Store Creation");
    println!("----------------------------------");
    
    let store = SessionStore::new_in_memory().await.unwrap();
    println!("✅ Created in-memory session store");
    println!("   Initial session count: {}", store.count().await.unwrap());

    // Demo 2: Creating and Saving Sessions
    println!("\n\n💼 Demo 2: Creating and Saving Sessions");
    println!("----------------------------------");
    
    let manifest1 = create_test_manifest("document.pdf", 20);
    let state1 = SessionState::new(
        "session-001".to_string(),
        "file-001".to_string(),
        manifest1,
    );
    
    store.save(&state1).await.unwrap();
    println!("✅ Session created: {}", state1.session_id);
    println!("   File: {}", state1.manifest.filename);
    println!("   Total chunks: {}", state1.manifest.total_chunks);
    println!("   Status: {:?}", state1.status);

    // Demo 3: Loading Sessions
    println!("\n\n📂 Demo 3: Loading Sessions");
    println!("----------------------------------");
    
    let loaded = store.load("session-001").await.unwrap().unwrap();
    println!("✅ Loaded session: {}", loaded.session_id);
    println!("   File ID: {}", loaded.file_id);
    println!("   Filename: {}", loaded.manifest.filename);
    println!("   Progress: {:.1}%", loaded.progress_percent());

    // Demo 4: Tracking Chunk Completion
    println!("\n\n✅ Demo 4: Tracking Chunk Completion");
    println!("----------------------------------");
    
    store.update_status("session-001", SessionStatus::Active).await.unwrap();
    println!("Session status updated to: Active");
    
    println!("\nCompleting chunks:");
    for i in 0..5 {
        store.mark_chunk_completed("session-001", i).await.unwrap();
        let state = store.load("session-001").await.unwrap().unwrap();
        println!("   Chunk {}: ✅ Progress: {:.1}%", i, state.progress_percent());
    }

    // Demo 5: Resume Information
    println!("\n\n🔄 Demo 5: Resume Information");
    println!("----------------------------------");
    
    let resume_info = store.get_resume_info("session-001").await.unwrap();
    println!("Resume Info:");
    println!("   Session ID: {}", resume_info.session_id);
    println!("   Total chunks: {}", resume_info.total_chunks);
    println!("   Completed: {}", resume_info.completed_chunks);
    println!("   Remaining: {}", resume_info.remaining_chunks);
    println!("   Progress: {:.1}%", resume_info.progress_percent);
    println!("   Can resume: {}", resume_info.can_resume);

    // Demo 6: Simulating Partial Transfer with Pause
    println!("\n\n⏸️  Demo 6: Partial Transfer with Pause");
    println!("----------------------------------");
    
    let manifest2 = create_test_manifest("video.mp4", 100);
    let mut state2 = SessionState::new(
        "session-002".to_string(),
        "file-002".to_string(),
        manifest2,
    );
    state2.status = SessionStatus::Active;
    store.save(&state2).await.unwrap();
    
    println!("Started transfer: {}", state2.manifest.filename);
    
    // Simulate partial completion
    for i in 0..30 {
        store.mark_chunk_completed("session-002", i).await.unwrap();
    }
    
    let state = store.load("session-002").await.unwrap().unwrap();
    println!("   Transferred: {}/{} chunks ({:.1}%)",
        state.completed_chunks.len(),
        state.manifest.total_chunks,
        state.progress_percent()
    );
    
    // Pause
    store.update_status("session-002", SessionStatus::Paused).await.unwrap();
    println!("   Status: Paused ⏸");

    // Demo 7: Resume After Pause
    println!("\n\n▶️  Demo 7: Resume After Pause");
    println!("----------------------------------");
    
    let resume_info = store.get_resume_info("session-002").await.unwrap();
    println!("Resuming session: {}", resume_info.session_id);
    println!("   Already completed: {} chunks", resume_info.completed_chunks);
    println!("   Remaining: {} chunks", resume_info.remaining_chunks);
    
    store.update_status("session-002", SessionStatus::Active).await.unwrap();
    println!("   Status: Active ▶");
    
    // Complete remaining chunks
    for i in 30..77 {
        store.mark_chunk_completed("session-002", i).await.unwrap();
    }
    
    let state = store.load("session-002").await.unwrap().unwrap();
    println!("   Status after completion: {:?}", state.status);
    println!("   Progress: {:.1}%", state.progress_percent());

    // Demo 8: Failed Chunks Tracking
    println!("\n\n❌ Demo 8: Failed Chunks Tracking");
    println!("----------------------------------");
    
    let manifest3 = create_test_manifest("archive.zip", 50);
    let mut state3 = SessionState::new(
        "session-003".to_string(),
        "file-003".to_string(),
        manifest3,
    );
    state3.status = SessionStatus::Active;
    store.save(&state3).await.unwrap();
    
    println!("Transfer started: {}", state3.manifest.filename);
    
    // Simulate some failures
    store.mark_chunk_failed("session-003", 5).await.unwrap();
    store.mark_chunk_failed("session-003", 12).await.unwrap();
    store.mark_chunk_failed("session-003", 23).await.unwrap();
    
    // Complete others
    for i in 0..38 {
        if i != 5 && i != 12 && i != 23 {
            store.mark_chunk_completed("session-003", i).await.unwrap();
        }
    }
    
    let state = store.load("session-003").await.unwrap().unwrap();
    println!("   Completed chunks: {}", state.completed_chunks.len());
    println!("   Failed chunks: {} - {:?}", state.failed_chunks.len(), state.failed_chunks);
    println!("   Progress: {:.1}%", state.progress_percent());

    // Demo 9: Listing All Sessions
    println!("\n\n📋 Demo 9: Listing All Sessions");
    println!("----------------------------------");
    
    let sessions = store.list_all().await.unwrap();
    println!("Total sessions: {}\n", sessions.len());
    
    for (i, session) in sessions.iter().enumerate() {
        println!("{}. {}", i + 1, session.session_id);
        println!("   File: {}", session.filename);
        println!("   Status: {:?}", session.status);
        println!("   Progress: {:.1}%", session.progress_percent);
    }

    // Demo 10: Filtering by Status
    println!("\n\n🔍 Demo 10: Filtering by Status");
    println!("----------------------------------");
    
    let active = store.list_by_status(SessionStatus::Active).await.unwrap();
    println!("Active sessions: {}", active.len());
    for session in &active {
        println!("   - {} ({:.1}%)", session.session_id, session.progress_percent);
    }
    
    let completed = store.list_by_status(SessionStatus::Completed).await.unwrap();
    println!("\nCompleted sessions: {}", completed.len());
    for session in &completed {
        println!("   - {} (✅ 100%)", session.session_id);
    }
    
    let paused = store.list_by_status(SessionStatus::Paused).await.unwrap();
    println!("\nPaused sessions: {}", paused.len());
    for session in &paused {
        println!("   - {} ({:.1}%)", session.session_id, session.progress_percent);
    }

    // Demo 11: Session Existence Check
    println!("\n\n🔎 Demo 11: Session Existence Check");
    println!("----------------------------------");
    
    let exists1 = store.exists("session-001").await.unwrap();
    println!("Session 'session-001' exists: {}", exists1);
    
    let exists2 = store.exists("non-existent").await.unwrap();
    println!("Session 'non-existent' exists: {}", exists2);

    // Demo 12: Deleting Sessions
    println!("\n\n🗑️  Demo 12: Deleting Sessions");
    println!("----------------------------------");
    
    let count_before = store.count().await.unwrap();
    println!("Sessions before deletion: {}", count_before);
    
    let deleted = store.delete("session-003").await.unwrap();
    println!("Deleted 'session-003': {}", deleted);
    
    let count_after = store.count().await.unwrap();
    println!("Sessions after deletion: {}", count_after);

    // Demo 13: Database Persistence
    println!("\n\n💾 Demo 13: Database Persistence");
    println!("----------------------------------");
    
    // Create file-based store (using current directory)
    let temp_db = "chunkstream_demo.db";
    println!("Creating persistent database: {}", temp_db);
    
    match SessionStore::new(&format!("sqlite:{}", temp_db)).await {
        Ok(file_store) => {
            let manifest4 = create_test_manifest("data.bin", 30);
            let state4 = SessionState::new(
                "persistent-session".to_string(),
                "file-004".to_string(),
                manifest4,
            );
            
            file_store.save(&state4).await.unwrap();
            println!("✅ Saved session to disk: {}", temp_db);
            
            // Close and reopen
            file_store.close().await;
            println!("   Closed database connection");
            
            let file_store2 = SessionStore::new(&format!("sqlite:{}", temp_db)).await.unwrap();
            let loaded = file_store2.load("persistent-session").await.unwrap();
            
            if loaded.is_some() {
                println!("✅ Successfully loaded session after reopen");
                println!("   Session persists across restarts!");
            }
            
            file_store2.close().await;
            std::fs::remove_file(temp_db).ok();
        }
        Err(e) => {
            println!("⚠️  Skipping file persistence demo: {}", e);
            println!("   (In-memory database works perfectly!)");
        }
    }

    // Demo 14: Performance Summary
    println!("\n\n⚡ Demo 14: Performance Summary");
    println!("----------------------------------");
    
    let total_sessions = store.count().await.unwrap();
    let all_sessions = store.list_all().await.unwrap();
    
    let mut total_chunks = 0usize;
    for session in &all_sessions {
        if let Some(state) = store.load(&session.session_id).await.ok().flatten() {
            total_chunks += state.completed_chunks.len();
        }
    }
    
    println!("Database Statistics:");
    println!("   Total sessions: {}", total_sessions);
    println!("   Total chunks tracked: {}", total_chunks);
    println!("   Database type: SQLite (in-memory)");
    println!("   All operations: Async/non-blocking");

    println!("\n\n🎉 All session store demos completed successfully!");
    println!("===============================================\n");
}
