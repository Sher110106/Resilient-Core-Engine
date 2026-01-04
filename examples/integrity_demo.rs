use bytes::Bytes;
use chunkstream_pro::chunk::{Chunk, ChunkMetadata, FileManifest, Priority};
use chunkstream_pro::integrity::{IntegrityCheck, IntegrityVerifier};

fn create_test_chunk(data: &[u8], sequence_number: u32, total_chunks: u32) -> Chunk {
    let checksum = IntegrityVerifier::calculate_checksum(data);
    Chunk {
        metadata: ChunkMetadata {
            chunk_id: sequence_number as u64,
            file_id: "demo-file".to_string(),
            sequence_number,
            total_chunks,
            data_size: data.len(),
            checksum,
            is_parity: false,
            priority: Priority::Normal,
            created_at: chrono::Utc::now().timestamp(),
        },
        data: Bytes::from(data.to_vec()),
    }
}

#[tokio::main]
async fn main() {
    println!("\n🔐 ChunkStream Pro - Integrity Module Demo");
    println!("==========================================\n");

    // Demo 1: Basic checksum calculation
    println!("📝 Demo 1: Basic Checksum Calculation");
    println!("--------------------------------------");
    let data1 = b"Hello, ChunkStream Pro!";
    let checksum1 = IntegrityVerifier::calculate_checksum(data1);
    println!("Data: {:?}", String::from_utf8_lossy(data1));
    println!("BLAKE3 Checksum: {}", hex::encode(checksum1));

    let data2 = b"Hello, ChunkStream Pro!"; // Same data
    let checksum2 = IntegrityVerifier::calculate_checksum(data2);
    println!(
        "\nSame data produces same checksum: {}",
        checksum1 == checksum2
    );

    let data3 = b"Different data";
    let checksum3 = IntegrityVerifier::calculate_checksum(data3);
    println!(
        "Different data produces different checksum: {}",
        checksum1 != checksum3
    );

    // Demo 2: Chunk verification
    println!("\n\n✅ Demo 2: Single Chunk Verification");
    println!("--------------------------------------");
    let chunk_data = b"This is chunk data that needs verification";
    let chunk = create_test_chunk(chunk_data, 0, 10);

    println!("Chunk ID: {}", chunk.metadata.chunk_id);
    println!(
        "Sequence: {}/{}",
        chunk.metadata.sequence_number + 1,
        chunk.metadata.total_chunks
    );
    println!("Data size: {} bytes", chunk.metadata.data_size);
    println!("Checksum: {}", hex::encode(chunk.metadata.checksum));

    match IntegrityVerifier::verify_chunk(&chunk) {
        Ok(_) => println!("✅ Chunk verification: PASSED"),
        Err(e) => println!("❌ Chunk verification: FAILED - {}", e),
    }

    // Demo 3: Corrupted chunk detection
    println!("\n\n🚨 Demo 3: Corrupted Chunk Detection");
    println!("--------------------------------------");
    let mut corrupted_chunk = create_test_chunk(b"Original data", 1, 10);
    corrupted_chunk.metadata.checksum = [0u8; 32]; // Corrupt checksum

    println!(
        "Original checksum: {}",
        hex::encode(IntegrityVerifier::calculate_checksum(&corrupted_chunk.data))
    );
    println!(
        "Stored checksum: {}",
        hex::encode(corrupted_chunk.metadata.checksum)
    );

    match IntegrityVerifier::verify_chunk(&corrupted_chunk) {
        Ok(_) => println!("✅ Verification passed (unexpected!)"),
        Err(e) => println!("❌ Verification failed (expected!): {}", e),
    }

    // Demo 4: Batch parallel verification
    println!("\n\n⚡ Demo 4: Batch Parallel Verification");
    println!("--------------------------------------");

    let mut chunks: Vec<Chunk> = (0..20)
        .map(|i| create_test_chunk(format!("Chunk {} data", i).as_bytes(), i, 20))
        .collect();

    println!("Created {} chunks for verification", chunks.len());

    // Corrupt some chunks
    chunks[5].metadata.checksum = [0u8; 32];
    chunks[12].metadata.checksum = [0u8; 32];
    chunks[18].metadata.checksum = [0u8; 32];
    println!("Corrupted chunks: #5, #12, #18");

    let summary = IntegrityVerifier::verify_batch_summary(&chunks)
        .await
        .unwrap();

    println!("\n📊 Verification Summary:");
    println!("  Total chunks: {}", summary.total);
    println!("  ✅ Passed: {}", summary.passed);
    println!("  ❌ Failed: {}", summary.failed);
    println!("  Success rate: {:.1}%", summary.success_rate);

    if summary.has_failures() {
        println!("\n  Failed chunks:");
        for failed in &summary.failed_chunks {
            println!(
                "    - Chunk #{} (seq: {}) - {}",
                failed.chunk_id, failed.sequence_number, failed.error
            );
        }
    }

    // Demo 5: Detailed verification results
    println!("\n\n📋 Demo 5: Detailed Verification Results");
    println!("--------------------------------------");

    let good_chunk = create_test_chunk(b"Good chunk", 0, 2);
    let mut bad_chunk = create_test_chunk(b"Bad chunk", 1, 2);
    bad_chunk.metadata.checksum = [0xff; 32];

    let result1 = IntegrityVerifier::verify_chunk_detailed(&good_chunk);
    let result2 = IntegrityVerifier::verify_chunk_detailed(&bad_chunk);

    println!("Good Chunk:");
    println!("  Success: {}", result1.success);
    println!("  Expected: {}", hex::encode(&result1.expected[..8]));
    println!(
        "  Actual: {}",
        hex::encode(&result1.actual.as_ref().unwrap()[..8])
    );

    println!("\nBad Chunk:");
    println!("  Success: {}", result2.success);
    println!("  Expected: {}", hex::encode(&result2.expected[..8]));
    println!(
        "  Actual: {}",
        hex::encode(&result2.actual.as_ref().unwrap()[..8])
    );

    // Demo 6: Metadata verification
    println!("\n\n🔍 Demo 6: Metadata Verification");
    println!("--------------------------------------");

    let valid_metadata = ChunkMetadata {
        chunk_id: 1,
        file_id: "test-file".to_string(),
        sequence_number: 5,
        total_chunks: 10,
        data_size: 256 * 1024,
        checksum: [0u8; 32],
        is_parity: false,
        priority: Priority::High,
        created_at: chrono::Utc::now().timestamp(),
    };

    match IntegrityVerifier::verify_metadata(&valid_metadata) {
        Ok(_) => println!("✅ Valid metadata: PASSED"),
        Err(e) => println!("❌ Valid metadata: FAILED - {}", e),
    }

    let invalid_metadata = ChunkMetadata {
        chunk_id: 2,
        file_id: "test-file".to_string(),
        sequence_number: 10, // Invalid: >= total_chunks
        total_chunks: 10,
        data_size: 256 * 1024,
        checksum: [0u8; 32],
        is_parity: false,
        priority: Priority::Normal,
        created_at: chrono::Utc::now().timestamp(),
    };

    match IntegrityVerifier::verify_metadata(&invalid_metadata) {
        Ok(_) => println!("✅ Invalid metadata: PASSED (unexpected!)"),
        Err(e) => println!("❌ Invalid metadata: FAILED (expected!) - {}", e),
    }

    // Demo 7: Manifest verification
    println!("\n\n📦 Demo 7: File Manifest Verification");
    println!("--------------------------------------");

    let valid_manifest = FileManifest {
        file_id: "demo-file".to_string(),
        filename: "demo.bin".to_string(),
        total_size: 1024 * 1024,
        chunk_size: 256 * 1024,
        total_chunks: 13,
        data_chunks: 10,
        parity_chunks: 3,
        priority: Priority::Normal,
        checksum: [0u8; 32],
    };

    println!("Manifest:");
    println!("  File: {}", valid_manifest.filename);
    println!("  Size: {} bytes", valid_manifest.total_size);
    println!(
        "  Chunks: {} (data: {}, parity: {})",
        valid_manifest.total_chunks, valid_manifest.data_chunks, valid_manifest.parity_chunks
    );

    match IntegrityVerifier::verify_manifest(&valid_manifest) {
        Ok(_) => println!("  ✅ Manifest verification: PASSED"),
        Err(e) => println!("  ❌ Manifest verification: FAILED - {}", e),
    }

    // Demo 8: Integrity check records
    println!("\n\n💾 Demo 8: Integrity Check Records");
    println!("--------------------------------------");

    let data = b"Data that needs integrity tracking";
    let mut check = IntegrityVerifier::create_check(data);

    println!("Created integrity check:");
    println!("  Type: {:?}", check.checksum_type);
    println!("  Checksum: {}", hex::encode(&check.value[..8]));
    println!("  Verified: {:?}", check.verified_at);

    match IntegrityVerifier::verify_check(data, &check) {
        Ok(_) => {
            check.mark_verified();
            println!("  ✅ Verification: PASSED");
            println!("  Verified at: {}", check.verified_at.unwrap());
        }
        Err(e) => println!("  ❌ Verification: FAILED - {}", e),
    }

    // Demo 9: Performance test
    println!("\n\n⚡ Demo 9: Performance Test");
    println!("--------------------------------------");

    let chunk_count = 100;
    let large_chunks: Vec<Chunk> = (0..chunk_count)
        .map(|i| {
            let data = format!("Large chunk {} with lots of data: {}", i, "x".repeat(10000));
            create_test_chunk(data.as_bytes(), i, chunk_count)
        })
        .collect();

    println!("Verifying {} chunks in parallel...", chunk_count);

    let start = std::time::Instant::now();
    let results = IntegrityVerifier::verify_chunks_parallel(&large_chunks).await;
    let duration = start.elapsed();

    let passed = results.iter().filter(|r| r.is_ok()).count();
    println!("  ✅ Verified {} chunks in {:?}", passed, duration);
    println!(
        "  ⚡ Throughput: {:.2} chunks/sec",
        chunk_count as f64 / duration.as_secs_f64()
    );

    println!("\n\n🎉 All demos completed successfully!");
    println!("==========================================\n");
}
