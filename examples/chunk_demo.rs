use chunkstream_pro::chunk::{ChunkManager, Priority};
use std::path::Path;
use tempfile::TempDir;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("═══════════════════════════════════════════════════════════");
    println!("  ChunkStream Pro - Module 1 Demo: Chunk Manager");
    println!("═══════════════════════════════════════════════════════════\n");

    // Create a temporary directory for our demo
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("demo_file.bin");

    // Create a test file (1MB)
    println!("📄 Creating test file (1 MB)...");
    let mut file = File::create(&test_file).await?;
    let data: Vec<u8> = (0..1024 * 1024).map(|i| (i % 256) as u8).collect();
    file.write_all(&data).await?;
    println!("   ✓ Test file created at: {:?}\n", test_file);

    // Initialize ChunkManager with 256KB chunks, 10 data + 3 parity
    println!("⚙️  Initializing ChunkManager...");
    let manager = ChunkManager::new(256 * 1024, 10, 3)?;
    println!("   ✓ Chunk size: 256 KB");
    println!("   ✓ Erasure coding: 10 data shards + 3 parity shards");
    println!("   ✓ Can survive loss of up to 3 chunks\n");

    // Split the file
    println!("🔪 Splitting file into chunks...");
    let (manifest, chunks) = manager
        .split_file(&test_file, "demo-file-id".to_string(), Priority::Normal)
        .await?;

    println!("   ✓ File split complete!");
    println!("   → Original file size: {} bytes", manifest.total_size);
    println!("   → Data chunks: {}", manifest.data_chunks);
    println!("   → Parity chunks: {}", manifest.parity_chunks);
    println!("   → Total chunks: {}", manifest.total_chunks);
    println!(
        "   → Checksum: {}",
        hex::encode(&manifest.checksum[..8])
    );

    // Show chunk details
    println!("\n📦 Chunk details:");
    for (i, chunk) in chunks.iter().take(5).enumerate() {
        let chunk_type = if chunk.metadata.is_parity {
            "PARITY"
        } else {
            "DATA"
        };
        println!(
            "   Chunk #{}: {} | {} bytes | checksum: {}",
            chunk.metadata.sequence_number,
            chunk_type,
            chunk.metadata.data_size,
            hex::encode(&chunk.metadata.checksum[..4])
        );
    }
    if chunks.len() > 5 {
        println!("   ... and {} more chunks", chunks.len() - 5);
    }

    // Reconstruct with all chunks
    println!("\n🔧 Reconstructing file (with all chunks)...");
    let output_file1 = temp_dir.path().join("reconstructed_full.bin");
    manager
        .reconstruct_file(&manifest, chunks.clone(), &output_file1)
        .await?;
    println!("   ✓ File reconstructed successfully!");
    println!("   → Output: {:?}\n", output_file1);

    // Simulate chunk loss and reconstruct
    println!("⚠️  Simulating chunk loss scenario...");
    let mut partial_chunks = chunks.clone();

    // Remove 3 chunks (2 data + 1 parity)
    partial_chunks.remove(1);
    partial_chunks.remove(3);
    partial_chunks.remove(5);

    println!("   ✗ Removed 3 chunks (simulating network loss)");
    println!("   → Remaining chunks: {}/{}", partial_chunks.len(), chunks.len());

    println!("\n🔧 Reconstructing file (with {} chunks missing)...", 3);
    let output_file2 = temp_dir.path().join("reconstructed_partial.bin");
    manager
        .reconstruct_file(&manifest, partial_chunks, &output_file2)
        .await?;
    println!("   ✓ File reconstructed successfully even with missing chunks!");
    println!("   → Output: {:?}", output_file2);

    // Verify files are identical
    println!("\n✅ Verifying reconstruction integrity...");
    let original = tokio::fs::read(&test_file).await?;
    let reconstructed_full = tokio::fs::read(&output_file1).await?;
    let reconstructed_partial = tokio::fs::read(&output_file2).await?;

    assert_eq!(original, reconstructed_full, "Full reconstruction mismatch!");
    assert_eq!(
        original, reconstructed_partial,
        "Partial reconstruction mismatch!"
    );

    println!("   ✓ All reconstructed files match the original!");
    println!("   ✓ Erasure coding works perfectly!\n");

    // Demonstrate adaptive chunk sizing
    println!("🌐 Adaptive Chunk Sizing Examples:");
    println!("   ┌─────────────────────────────────────────────────────┐");
    println!(
        "   │ Network Quality    │ RTT  │ Loss │ Chunk Size         │"
    );
    println!("   ├─────────────────────────────────────────────────────┤");
    println!(
        "   │ Excellent          │ 20ms │  0%  │ {:>6} KB          │",
        manager.calculate_optimal_chunk_size(20, 0.0) / 1024
    );
    println!(
        "   │ Good               │ 50ms │  1%  │ {:>6} KB          │",
        manager.calculate_optimal_chunk_size(50, 0.01) / 1024
    );
    println!(
        "   │ Fair               │ 100ms│  5%  │ {:>6} KB          │",
        manager.calculate_optimal_chunk_size(100, 0.05) / 1024
    );
    println!(
        "   │ Poor               │ 200ms│ 12%  │ {:>6} KB          │",
        manager.calculate_optimal_chunk_size(200, 0.12) / 1024
    );
    println!(
        "   │ Very Poor          │ 400ms│ 20%  │ {:>6} KB          │",
        manager.calculate_optimal_chunk_size(400, 0.20) / 1024
    );
    println!("   └─────────────────────────────────────────────────────┘\n");

    println!("═══════════════════════════════════════════════════════════");
    println!("  ✅ Module 1 Demo Complete!");
    println!("═══════════════════════════════════════════════════════════");

    Ok(())
}
