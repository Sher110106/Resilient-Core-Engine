//! Benchmark tests for validating RESILIENT's claims
//!
//! This module tests the core claims:
//! - 20% packet loss tolerance
//! - Erasure coding recovery
//! - Priority queue ordering
//! - Transfer resume functionality

#[path = "simulation/mod.rs"]
mod simulation;

use chunkstream_pro::chunk::{ChunkManager, Priority};
use chunkstream_pro::integrity::IntegrityVerifier;
use simulation::{
    BenchmarkResult, LossyChannel, LossyChannelConfig, MetricsCollector, TestMatrixParams,
};
use tempfile::TempDir;
use tokio::fs;

/// Test the erasure coding system under various packet loss conditions
async fn test_erasure_recovery_at_loss_rate(
    loss_rate: f32,
    file_size: usize,
    data_shards: usize,
    parity_shards: usize,
) -> BenchmarkResult {
    let test_name = format!(
        "erasure_recovery_{}pct_{}KB",
        (loss_rate * 100.0) as u32,
        file_size / 1024
    );

    let temp_dir = match TempDir::new() {
        Ok(dir) => dir,
        Err(e) => return BenchmarkResult::failure(&test_name, e.to_string()),
    };

    // Create test file
    let test_file = temp_dir.path().join("test.bin");
    let test_data: Vec<u8> = (0..file_size).map(|i| (i % 256) as u8).collect();

    if let Err(e) = fs::write(&test_file, &test_data).await {
        return BenchmarkResult::failure(&test_name, e.to_string());
    }

    let original_checksum = IntegrityVerifier::calculate_checksum(&test_data);

    // Create chunk manager
    let chunk_size = 512 * 1024; // 512 KB chunks
    let chunk_manager = match ChunkManager::new(chunk_size, data_shards, parity_shards) {
        Ok(cm) => cm,
        Err(e) => return BenchmarkResult::failure(&test_name, e.to_string()),
    };

    // Start metrics collection
    let mut collector = MetricsCollector::new().with_params(file_size, loss_rate, 0, 0);
    collector.start();

    // Split file into chunks
    let (manifest, chunks) = match chunk_manager
        .split_file(&test_file, "test".to_string(), Priority::Normal)
        .await
    {
        Ok(result) => result,
        Err(e) => return BenchmarkResult::failure(&test_name, e.to_string()),
    };

    // Simulate lossy channel
    let channel = LossyChannel::new(LossyChannelConfig::with_loss(loss_rate));

    // Simulate sending chunks through lossy channel
    let mut received_chunks = Vec::new();
    for chunk in &chunks {
        collector.chunk_sent();
        let chunk_data = chunk.data.to_vec();
        match channel.send(&chunk_data).await {
            Ok(data) => {
                collector.first_byte_received();
                // Re-create chunk with received data
                let mut received_chunk = chunk.clone();
                received_chunk.data = data.into();
                received_chunks.push(received_chunk);
            }
            Err(_) => {
                collector.chunk_lost();
            }
        }
    }

    // Calculate how many chunks were lost
    let chunks_received = received_chunks.len();
    let chunks_lost = chunks.len() - chunks_received;

    // Try to reconstruct
    let output_file = temp_dir.path().join("reconstructed.bin");
    let reconstruction_result = chunk_manager
        .reconstruct_file(&manifest, received_chunks, &output_file)
        .await;

    let (success, checksum_valid) = match reconstruction_result {
        Ok(_) => {
            // Verify reconstructed file
            match fs::read(&output_file).await {
                Ok(reconstructed_data) => {
                    let reconstructed_checksum =
                        IntegrityVerifier::calculate_checksum(&reconstructed_data);
                    (true, original_checksum == reconstructed_checksum)
                }
                Err(_) => (false, false),
            }
        }
        Err(_) => (false, false),
    };

    // Record recovered chunks
    if success && chunks_lost > 0 {
        for _ in 0..chunks_lost {
            collector.chunk_recovered();
        }
    }

    let metrics = collector.finish(success, checksum_valid);
    BenchmarkResult::success(&test_name, metrics)
}

/// Run the 20% packet loss claim validation test
#[tokio::test]
async fn validate_20_percent_loss_claim() {
    println!("\n========================================");
    println!("CLAIM VALIDATION: 20% Packet Loss Tolerance");
    println!("========================================\n");

    // Test with default 50 data + 10 parity (17% theoretical tolerance)
    let results_50_10 = run_loss_rate_tests(50, 10).await;

    // Test with 50 data + 15 parity (23% theoretical tolerance)
    let results_50_15 = run_loss_rate_tests(50, 15).await;

    // Print summary
    println!("\n--- Results with 50 data + 10 parity shards ---");
    print_loss_test_summary(&results_50_10);

    println!("\n--- Results with 50 data + 15 parity shards ---");
    print_loss_test_summary(&results_50_15);

    // Validate claim
    let success_at_20_percent = results_50_10
        .iter()
        .filter(|r| (r.metrics.packet_loss_rate - 0.20).abs() < 0.01)
        .filter(|r| r.is_success())
        .count();

    let total_at_20_percent = results_50_10
        .iter()
        .filter(|r| (r.metrics.packet_loss_rate - 0.20).abs() < 0.01)
        .count();

    println!("\n========================================");
    if success_at_20_percent as f64 / total_at_20_percent as f64 >= 0.95 {
        println!("CLAIM VALIDATED: 20% packet loss tolerance achieved");
    } else {
        println!("CLAIM NOT VALIDATED: Insufficient success rate at 20% loss");
        println!(
            "Success rate: {}/{} ({:.1}%)",
            success_at_20_percent,
            total_at_20_percent,
            success_at_20_percent as f64 / total_at_20_percent as f64 * 100.0
        );
    }
    println!("========================================\n");
}

async fn run_loss_rate_tests(data_shards: usize, parity_shards: usize) -> Vec<BenchmarkResult> {
    let loss_rates = vec![0.0, 0.05, 0.10, 0.15, 0.17, 0.20, 0.25, 0.30];
    let file_sizes = vec![100 * 1024, 1024 * 1024, 5 * 1024 * 1024];
    let iterations = 3; // Run each test 3 times

    let mut results = Vec::new();

    for &loss_rate in &loss_rates {
        for &file_size in &file_sizes {
            for _ in 0..iterations {
                let result = test_erasure_recovery_at_loss_rate(
                    loss_rate,
                    file_size,
                    data_shards,
                    parity_shards,
                )
                .await;
                results.push(result);
            }
        }
    }

    results
}

fn print_loss_test_summary(results: &[BenchmarkResult]) {
    // Group by loss rate
    let mut by_loss_rate: std::collections::HashMap<u32, Vec<&BenchmarkResult>> =
        std::collections::HashMap::new();

    for result in results {
        let key = (result.metrics.packet_loss_rate * 100.0) as u32;
        by_loss_rate.entry(key).or_default().push(result);
    }

    let mut keys: Vec<_> = by_loss_rate.keys().cloned().collect();
    keys.sort();

    println!(
        "{:<12} | {:<8} | {:<12} | {:<10}",
        "Loss Rate", "Tests", "Passed", "Success %"
    );
    println!("{}", "-".repeat(50));

    for key in keys {
        let tests = &by_loss_rate[&key];
        let passed = tests.iter().filter(|t| t.is_success()).count();
        let total = tests.len();
        let rate = passed as f64 / total as f64 * 100.0;

        println!(
            "{:>10}% | {:>8} | {:>12} | {:>9.1}%",
            key, total, passed, rate
        );
    }
}

/// Test throughput at various packet loss rates
#[tokio::test]
async fn benchmark_throughput_vs_loss() {
    println!("\n========================================");
    println!("BENCHMARK: Throughput vs Packet Loss");
    println!("========================================\n");

    let params = TestMatrixParams::minimal();
    let file_size = 1024 * 1024; // 1 MB

    println!(
        "{:<12} | {:<15} | {:<12} | {:<10}",
        "Loss Rate", "Throughput MB/s", "Duration ms", "Success"
    );
    println!("{}", "-".repeat(55));

    for &loss_rate in &params.loss_rates {
        let result = test_erasure_recovery_at_loss_rate(loss_rate, file_size, 50, 10).await;

        println!(
            "{:>10}% | {:>14.2} | {:>11} | {:>10}",
            (loss_rate * 100.0) as u32,
            result.metrics.throughput_mbps(),
            result.metrics.transfer_duration_ms,
            if result.is_success() { "YES" } else { "NO" }
        );
    }
}

/// Test with various file sizes
#[tokio::test]
async fn benchmark_file_sizes() {
    println!("\n========================================");
    println!("BENCHMARK: Various File Sizes");
    println!("========================================\n");

    let file_sizes = vec![
        ("1 KB", 1024),
        ("10 KB", 10 * 1024),
        ("100 KB", 100 * 1024),
        ("1 MB", 1024 * 1024),
        ("10 MB", 10 * 1024 * 1024),
    ];

    println!(
        "{:<12} | {:<12} | {:<15} | {:<10}",
        "File Size", "Chunks", "Duration ms", "Success"
    );
    println!("{}", "-".repeat(55));

    for (name, size) in file_sizes {
        let result = test_erasure_recovery_at_loss_rate(0.10, size, 50, 10).await;

        println!(
            "{:<12} | {:>12} | {:>14} | {:>10}",
            name,
            result.metrics.chunks_sent,
            result.metrics.transfer_duration_ms,
            if result.is_success() { "YES" } else { "NO" }
        );
    }
}

/// Find the maximum tolerable packet loss
#[tokio::test]
async fn find_max_tolerable_loss() {
    println!("\n========================================");
    println!("FINDING: Maximum Tolerable Packet Loss");
    println!("========================================\n");

    let file_size = 1024 * 1024; // 1 MB
    let iterations = 5;

    // Test with more parity for higher loss tolerance
    for (data_shards, parity_shards) in vec![(50, 10), (50, 15), (50, 20), (40, 20)] {
        let theoretical_tolerance =
            parity_shards as f64 / (data_shards + parity_shards) as f64 * 100.0;

        println!(
            "\nConfig: {} data + {} parity (theoretical: {:.1}% tolerance)",
            data_shards, parity_shards, theoretical_tolerance
        );
        println!("{}", "-".repeat(50));

        let mut _last_success_rate = 100.0;
        let mut max_tolerable = 0.0;

        for loss_rate in (0..=35).map(|x| x as f32 / 100.0).step_by(5) {
            let mut successes = 0;

            for _ in 0..iterations {
                let result = test_erasure_recovery_at_loss_rate(
                    loss_rate,
                    file_size,
                    data_shards,
                    parity_shards,
                )
                .await;
                if result.is_success() {
                    successes += 1;
                }
            }

            let success_rate = successes as f64 / iterations as f64 * 100.0;
            println!(
                "  {:>5.1}% loss: {:>3}/{} passed ({:.0}%)",
                loss_rate * 100.0,
                successes,
                iterations,
                success_rate
            );

            if success_rate >= 90.0 {
                max_tolerable = loss_rate * 100.0;
            }

            _last_success_rate = success_rate;
        }

        println!(
            "\n  → Maximum tolerable loss (≥90% success): {:.1}%",
            max_tolerable
        );
    }
}
