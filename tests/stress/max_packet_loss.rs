//! Maximum packet loss threshold tests
//!
//! Finds the actual failure threshold for erasure coding recovery

#[path = "../simulation/mod.rs"]
mod simulation;

use chunkstream_pro::chunk::{ChunkManager, Priority};
use chunkstream_pro::integrity::IntegrityVerifier;
use simulation::{BenchmarkResult, LossyChannel, LossyChannelConfig, MetricsCollector};
use tempfile::TempDir;
use tokio::fs;

/// Test erasure recovery at a specific loss rate with multiple iterations
async fn test_recovery_at_loss_rate(
    loss_rate: f32,
    file_size: usize,
    data_shards: usize,
    parity_shards: usize,
    iterations: usize,
) -> (usize, usize, f64) {
    let mut successes = 0;
    let mut total_duration_ms = 0u64;

    for _ in 0..iterations {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.bin");
        let test_data: Vec<u8> = (0..file_size).map(|i| (i % 256) as u8).collect();
        fs::write(&test_file, &test_data).await.unwrap();

        let original_checksum = IntegrityVerifier::calculate_checksum(&test_data);

        let chunk_manager = match ChunkManager::new(512 * 1024, data_shards, parity_shards) {
            Ok(cm) => cm,
            Err(_) => continue,
        };

        let start = std::time::Instant::now();

        // Split file
        let (manifest, chunks) = match chunk_manager
            .split_file(&test_file, "stress_test".to_string(), Priority::Normal)
            .await
        {
            Ok(result) => result,
            Err(_) => continue,
        };

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

        total_duration_ms += start.elapsed().as_millis() as u64;

        if success {
            successes += 1;
        }
    }

    let avg_duration = total_duration_ms as f64 / iterations as f64;
    (successes, iterations, avg_duration)
}

/// Find the maximum tolerable packet loss rate
/// Returns the highest loss rate where success rate >= threshold
async fn find_max_tolerable_loss(
    file_size: usize,
    data_shards: usize,
    parity_shards: usize,
    success_threshold: f64,
    iterations_per_rate: usize,
) -> f32 {
    let mut max_tolerable = 0.0f32;

    // Test from 0% to 40% in 2% increments
    for loss_pct in (0..=40).step_by(2) {
        let loss_rate = loss_pct as f32 / 100.0;
        let (successes, total, _) = test_recovery_at_loss_rate(
            loss_rate,
            file_size,
            data_shards,
            parity_shards,
            iterations_per_rate,
        )
        .await;

        let success_rate = successes as f64 / total as f64;

        if success_rate >= success_threshold {
            max_tolerable = loss_rate;
        } else {
            // Once we drop below threshold, stop searching
            break;
        }
    }

    max_tolerable
}

/// Stress test: Find actual maximum packet loss for default config (50+10)
#[tokio::test]
async fn stress_find_max_loss_default_config() {
    println!("\n========================================");
    println!("STRESS TEST: Maximum Packet Loss (50+10)");
    println!("========================================\n");

    let file_size = 1024 * 1024; // 1 MB
    let iterations = 5;

    println!("Configuration: 50 data + 10 parity shards");
    println!("Theoretical maximum: 16.7% loss\n");

    println!(
        "{:<12} | {:<10} | {:<12} | {:<10}",
        "Loss Rate", "Passed", "Success %", "Avg Time"
    );
    println!("{}", "-".repeat(50));

    for loss_pct in (0..=25).step_by(5) {
        let loss_rate = loss_pct as f32 / 100.0;
        let (successes, total, avg_time) =
            test_recovery_at_loss_rate(loss_rate, file_size, 50, 10, iterations).await;

        let success_rate = successes as f64 / total as f64 * 100.0;

        println!(
            "{:>10}% | {:>4}/{:<4} | {:>10.1}% | {:>8.0}ms",
            loss_pct, successes, total, success_rate, avg_time
        );
    }

    let max_loss = find_max_tolerable_loss(file_size, 50, 10, 0.90, iterations).await;
    println!(
        "\n=> Maximum tolerable loss (>=90% success): {:.1}%",
        max_loss * 100.0
    );
}

/// Stress test: Find maximum loss with increased parity (50+15)
#[tokio::test]
async fn stress_find_max_loss_high_parity() {
    println!("\n========================================");
    println!("STRESS TEST: Maximum Packet Loss (50+15)");
    println!("========================================\n");

    let file_size = 1024 * 1024; // 1 MB
    let iterations = 5;

    println!("Configuration: 50 data + 15 parity shards");
    println!("Theoretical maximum: 23.1% loss\n");

    println!(
        "{:<12} | {:<10} | {:<12} | {:<10}",
        "Loss Rate", "Passed", "Success %", "Avg Time"
    );
    println!("{}", "-".repeat(50));

    for loss_pct in (0..=30).step_by(5) {
        let loss_rate = loss_pct as f32 / 100.0;
        let (successes, total, avg_time) =
            test_recovery_at_loss_rate(loss_rate, file_size, 50, 15, iterations).await;

        let success_rate = successes as f64 / total as f64 * 100.0;

        println!(
            "{:>10}% | {:>4}/{:<4} | {:>10.1}% | {:>8.0}ms",
            loss_pct, successes, total, success_rate, avg_time
        );
    }

    let max_loss = find_max_tolerable_loss(file_size, 50, 15, 0.90, iterations).await;
    println!(
        "\n=> Maximum tolerable loss (>=90% success): {:.1}%",
        max_loss * 100.0
    );
}

/// Stress test: Find maximum loss with very high parity (50+25)
#[tokio::test]
async fn stress_find_max_loss_very_high_parity() {
    println!("\n==========================================");
    println!("STRESS TEST: Maximum Packet Loss (50+25)");
    println!("==========================================\n");

    let file_size = 1024 * 1024; // 1 MB
    let iterations = 5;

    println!("Configuration: 50 data + 25 parity shards");
    println!("Theoretical maximum: 33.3% loss\n");

    println!(
        "{:<12} | {:<10} | {:<12} | {:<10}",
        "Loss Rate", "Passed", "Success %", "Avg Time"
    );
    println!("{}", "-".repeat(50));

    for loss_pct in (0..=40).step_by(5) {
        let loss_rate = loss_pct as f32 / 100.0;
        let (successes, total, avg_time) =
            test_recovery_at_loss_rate(loss_rate, file_size, 50, 25, iterations).await;

        let success_rate = successes as f64 / total as f64 * 100.0;

        println!(
            "{:>10}% | {:>4}/{:<4} | {:>10.1}% | {:>8.0}ms",
            loss_pct, successes, total, success_rate, avg_time
        );
    }

    let max_loss = find_max_tolerable_loss(file_size, 50, 25, 0.90, iterations).await;
    println!(
        "\n=> Maximum tolerable loss (>=90% success): {:.1}%",
        max_loss * 100.0
    );
}

/// Comprehensive loss threshold discovery across multiple configurations
#[tokio::test]
async fn stress_comprehensive_loss_analysis() {
    println!("\n================================================");
    println!("STRESS TEST: Comprehensive Loss Threshold Analysis");
    println!("================================================\n");

    let file_size = 1024 * 1024; // 1 MB
    let iterations = 3;

    let configs = vec![
        (50, 5, "50+5 (9% overhead)"),
        (50, 10, "50+10 (17% overhead) - DEFAULT"),
        (50, 15, "50+15 (23% overhead)"),
        (50, 20, "50+20 (29% overhead)"),
        (50, 25, "50+25 (33% overhead)"),
        (40, 20, "40+20 (33% overhead)"),
    ];

    println!(
        "{:<30} | {:<15} | {:<15}",
        "Configuration", "Theoretical Max", "Actual Max (90%)"
    );
    println!("{}", "-".repeat(65));

    for (data, parity, name) in configs {
        let theoretical = parity as f64 / (data + parity) as f64 * 100.0;
        let actual = find_max_tolerable_loss(file_size, data, parity, 0.90, iterations).await;

        println!(
            "{:<30} | {:>13.1}% | {:>14.1}%",
            name,
            theoretical,
            actual * 100.0
        );
    }
}
