//! Large file stress tests
//!
//! Tests system behavior with large files (10MB - 100MB+)

#[path = "../simulation/mod.rs"]
mod simulation;

use chunkstream_pro::chunk::{ChunkManager, Priority};
use chunkstream_pro::integrity::IntegrityVerifier;
use simulation::{LossyChannel, LossyChannelConfig};
use std::time::Instant;
use tempfile::TempDir;
use tokio::fs;

/// Test large file transfer with specified parameters
async fn test_large_file(
    file_size: usize,
    loss_rate: f32,
    chunk_size: usize,
) -> (bool, u64, usize, usize) {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("large_test.bin");

    // Generate test data
    let test_data: Vec<u8> = (0..file_size).map(|i| (i % 256) as u8).collect();
    fs::write(&test_file, &test_data).await.unwrap();

    let original_checksum = IntegrityVerifier::calculate_checksum(&test_data);

    let chunk_manager = ChunkManager::new(chunk_size, 50, 10).unwrap();

    let start = Instant::now();

    // Split file
    let (manifest, chunks) = chunk_manager
        .split_file(&test_file, "large_file_test".to_string(), Priority::Normal)
        .await
        .unwrap();

    let total_chunks = chunks.len();

    // Simulate lossy channel
    let channel = LossyChannel::new(LossyChannelConfig::with_loss(loss_rate));
    let mut received_chunks = Vec::new();

    for chunk in &chunks {
        let chunk_data = chunk.data.to_vec();
        if let Ok(data) = channel.send(&chunk_data).await {
            let mut received_chunk = chunk.clone();
            received_chunk.data = data.into();
            received_chunks.push(received_chunk);
        }
    }

    let received_count = received_chunks.len();

    // Try to reconstruct
    let output_file = temp_dir.path().join("reconstructed.bin");
    let success = match chunk_manager
        .reconstruct_file(&manifest, received_chunks, &output_file)
        .await
    {
        Ok(_) => {
            if let Ok(reconstructed_data) = fs::read(&output_file).await {
                let reconstructed_checksum =
                    IntegrityVerifier::calculate_checksum(&reconstructed_data);
                original_checksum == reconstructed_checksum
            } else {
                false
            }
        }
        Err(_) => false,
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    (success, duration_ms, total_chunks, received_count)
}

fn format_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{} MB", bytes / (1024 * 1024))
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{} B", bytes)
    }
}

/// Stress test: Large file transfers at various sizes
#[tokio::test]
async fn stress_large_file_sizes() {
    println!("\n========================================");
    println!("STRESS TEST: Large File Transfers");
    println!("========================================\n");

    let file_sizes = vec![
        1 * 1024 * 1024,  // 1 MB
        5 * 1024 * 1024,  // 5 MB
        10 * 1024 * 1024, // 10 MB
        25 * 1024 * 1024, // 25 MB
        50 * 1024 * 1024, // 50 MB
    ];

    let loss_rate = 0.10; // 10% loss

    println!("Configuration: 50+10 shards, 512KB chunks, 10% packet loss\n");

    println!(
        "{:<12} | {:<10} | {:<12} | {:<15} | {:<10}",
        "File Size", "Chunks", "Received", "Duration", "Success"
    );
    println!("{}", "-".repeat(65));

    for file_size in file_sizes {
        let (success, duration_ms, total, received) =
            test_large_file(file_size, loss_rate, 512 * 1024).await;

        let throughput_mbps = if duration_ms > 0 {
            (file_size as f64 / (1024.0 * 1024.0)) / (duration_ms as f64 / 1000.0)
        } else {
            0.0
        };

        println!(
            "{:<12} | {:>10} | {:>6}/{:<5} | {:>10}ms | {:>10}",
            format_size(file_size),
            total,
            received,
            total,
            duration_ms,
            if success { "PASS" } else { "FAIL" }
        );
    }
}

/// Stress test: Large files with varying chunk sizes
#[tokio::test]
async fn stress_chunk_size_impact() {
    println!("\n============================================");
    println!("STRESS TEST: Chunk Size Impact on Large Files");
    println!("============================================\n");

    let file_size = 10 * 1024 * 1024; // 10 MB
    let loss_rate = 0.10;

    let chunk_sizes = vec![
        64 * 1024,   // 64 KB
        128 * 1024,  // 128 KB
        256 * 1024,  // 256 KB
        512 * 1024,  // 512 KB
        1024 * 1024, // 1 MB
    ];

    println!("File size: 10 MB, Loss rate: 10%\n");

    println!(
        "{:<12} | {:<10} | {:<12} | {:<12} | {:<10}",
        "Chunk Size", "Chunks", "Duration", "Throughput", "Success"
    );
    println!("{}", "-".repeat(62));

    for chunk_size in chunk_sizes {
        let (success, duration_ms, total, _received) =
            test_large_file(file_size, loss_rate, chunk_size).await;

        let throughput_mbps = if duration_ms > 0 {
            (file_size as f64 / (1024.0 * 1024.0)) / (duration_ms as f64 / 1000.0)
        } else {
            0.0
        };

        println!(
            "{:<12} | {:>10} | {:>10}ms | {:>9.2} MB/s | {:>10}",
            format_size(chunk_size),
            total,
            duration_ms,
            throughput_mbps,
            if success { "PASS" } else { "FAIL" }
        );
    }
}

/// Stress test: Large files under heavy packet loss
#[tokio::test]
async fn stress_large_file_heavy_loss() {
    println!("\n============================================");
    println!("STRESS TEST: Large Files Under Heavy Loss");
    println!("============================================\n");

    let file_size = 10 * 1024 * 1024; // 10 MB
    let chunk_size = 512 * 1024;

    let loss_rates = vec![0.0, 0.05, 0.10, 0.15, 0.17, 0.20];

    println!("File size: 10 MB, Chunk size: 512 KB\n");

    println!(
        "{:<12} | {:<12} | {:<12} | {:<10}",
        "Loss Rate", "Duration", "Throughput", "Success"
    );
    println!("{}", "-".repeat(50));

    for &loss_rate in &loss_rates {
        let (success, duration_ms, _total, _received) =
            test_large_file(file_size, loss_rate, chunk_size).await;

        let throughput_mbps = if duration_ms > 0 {
            (file_size as f64 / (1024.0 * 1024.0)) / (duration_ms as f64 / 1000.0)
        } else {
            0.0
        };

        println!(
            "{:>10}% | {:>10}ms | {:>9.2} MB/s | {:>10}",
            (loss_rate * 100.0) as u32,
            duration_ms,
            throughput_mbps,
            if success { "PASS" } else { "FAIL" }
        );
    }
}

/// Memory usage estimation during large file processing
#[tokio::test]
async fn stress_memory_estimation() {
    println!("\n==========================================");
    println!("STRESS TEST: Memory Usage Estimation");
    println!("==========================================\n");

    let file_sizes = vec![
        1 * 1024 * 1024,  // 1 MB
        10 * 1024 * 1024, // 10 MB
        50 * 1024 * 1024, // 50 MB
    ];

    println!(
        "{:<12} | {:<15} | {:<15} | {:<12}",
        "File Size", "Est. Peak RAM", "Chunks in RAM", "Overhead"
    );
    println!("{}", "-".repeat(58));

    for file_size in file_sizes {
        let chunk_size = 512 * 1024;
        let data_shards = 50;
        let parity_shards = 10;

        // Estimate: file data + chunks (data + parity) + overhead
        let num_chunks = (file_size + chunk_size - 1) / chunk_size;
        let total_shards = num_chunks * (data_shards + parity_shards);
        let estimated_peak = file_size + (total_shards * chunk_size / data_shards);
        let overhead = (estimated_peak as f64 / file_size as f64 - 1.0) * 100.0;

        println!(
            "{:<12} | {:>15} | {:>15} | {:>10.1}%",
            format_size(file_size),
            format_size(estimated_peak),
            total_shards,
            overhead
        );
    }

    println!("\nNote: Actual memory usage depends on streaming implementation.");
    println!("Current implementation loads entire file into memory.");
}
