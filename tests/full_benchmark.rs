//! Full benchmark test matrix runner
//!
//! This test runs a comprehensive set of benchmarks and generates a report.
//! Run with: cargo test --test full_benchmark -- --nocapture --ignored

#[path = "simulation/mod.rs"]
mod simulation;

use chunkstream_pro::chunk::{ChunkManager, Priority};
use chunkstream_pro::integrity::IntegrityVerifier;
use simulation::{
    BenchmarkReport, BenchmarkResult, LossyChannel, LossyChannelConfig, MetricsCollector,
};
use std::path::Path;
use tempfile::TempDir;
use tokio::fs;

/// Test erasure recovery and collect metrics
async fn run_benchmark(
    file_size: usize,
    loss_rate: f32,
    data_shards: usize,
    parity_shards: usize,
) -> BenchmarkResult {
    let test_name = format!(
        "erasure_{}_{}pct_{}d_{}p",
        format_size_short(file_size),
        (loss_rate * 100.0) as u32,
        data_shards,
        parity_shards
    );

    let temp_dir = match TempDir::new() {
        Ok(dir) => dir,
        Err(e) => return BenchmarkResult::failure(&test_name, e.to_string()),
    };

    let test_file = temp_dir.path().join("test.bin");
    let test_data: Vec<u8> = (0..file_size).map(|i| (i % 256) as u8).collect();

    if let Err(e) = fs::write(&test_file, &test_data).await {
        return BenchmarkResult::failure(&test_name, e.to_string());
    }

    let original_checksum = IntegrityVerifier::calculate_checksum(&test_data);

    let chunk_size = 256 * 1024; // 256 KB
    let chunk_manager = match ChunkManager::new(chunk_size, data_shards, parity_shards) {
        Ok(cm) => cm,
        Err(e) => return BenchmarkResult::failure(&test_name, e.to_string()),
    };

    let mut collector = MetricsCollector::new().with_params(file_size, loss_rate, 0, 0);
    collector.start();

    // Split file
    let (manifest, chunks) = match chunk_manager
        .split_file(&test_file, "benchmark".to_string(), Priority::Normal)
        .await
    {
        Ok(result) => result,
        Err(e) => return BenchmarkResult::failure(&test_name, e.to_string()),
    };

    // Simulate lossy channel
    let channel = LossyChannel::new(LossyChannelConfig::with_loss(loss_rate));
    let mut received_chunks = Vec::new();

    for chunk in &chunks {
        collector.chunk_sent();
        let chunk_data = chunk.data.to_vec();
        match channel.send(&chunk_data).await {
            Ok(data) => {
                collector.first_byte_received();
                let mut received_chunk = chunk.clone();
                received_chunk.data = data.into();
                received_chunks.push(received_chunk);
            }
            Err(_) => {
                collector.chunk_lost();
            }
        }
    }

    let chunks_lost = chunks.len() - received_chunks.len();

    // Try to reconstruct
    let output_file = temp_dir.path().join("reconstructed.bin");
    let reconstruction_result = chunk_manager
        .reconstruct_file(&manifest, received_chunks, &output_file)
        .await;

    let (success, checksum_valid) = match reconstruction_result {
        Ok(_) => match fs::read(&output_file).await {
            Ok(reconstructed_data) => {
                let reconstructed_checksum =
                    IntegrityVerifier::calculate_checksum(&reconstructed_data);
                (true, original_checksum == reconstructed_checksum)
            }
            Err(_) => (false, false),
        },
        Err(_) => (false, false),
    };

    if success && chunks_lost > 0 {
        for _ in 0..chunks_lost {
            collector.chunk_recovered();
        }
    }

    let metrics = collector.finish(success, checksum_valid);
    BenchmarkResult::success(&test_name, metrics)
}

fn format_size_short(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{}MB", bytes / (1024 * 1024))
    } else if bytes >= 1024 {
        format!("{}KB", bytes / 1024)
    } else {
        format!("{}B", bytes)
    }
}

/// Run the full benchmark suite and generate report
/// This is an ignored test - run explicitly with --ignored
#[tokio::test]
#[ignore]
async fn run_full_benchmark_suite() {
    println!("\n=============================================");
    println!("    RESILIENT Full Benchmark Suite");
    println!("=============================================\n");

    let file_sizes = vec![
        64 * 1024,       // 64 KB
        256 * 1024,      // 256 KB
        1024 * 1024,     // 1 MB
        4 * 1024 * 1024, // 4 MB
    ];

    let loss_rates = vec![0.0, 0.05, 0.10, 0.15, 0.17, 0.20, 0.25];

    let configs = vec![(50, 10, "default"), (50, 15, "high_parity")];

    let total_tests = file_sizes.len() * loss_rates.len() * configs.len();
    println!("Running {} benchmark tests...\n", total_tests);

    let mut results = Vec::new();
    let mut completed = 0;

    for &file_size in &file_sizes {
        for &loss_rate in &loss_rates {
            for &(data_shards, parity_shards, _name) in &configs {
                let result = run_benchmark(file_size, loss_rate, data_shards, parity_shards).await;

                completed += 1;
                let status = if result.is_success() { "PASS" } else { "FAIL" };
                print!(
                    "\r[{}/{}] {} - {}",
                    completed, total_tests, result.test_name, status
                );

                results.push(result);
            }
        }
    }

    println!("\n\nGenerating report...\n");

    let report = BenchmarkReport::from_results(results);

    // Print summary
    println!("=============================================");
    println!("              BENCHMARK RESULTS");
    println!("=============================================\n");

    println!("Total Tests: {}", report.summary.total_tests);
    println!(
        "Passed: {} ({:.1}%)",
        report.summary.passed, report.summary.pass_rate
    );
    println!("Failed: {}", report.summary.failed);

    println!(
        "\n--- Claim Validation: {} ---\n",
        report.claim_validation.claim
    );

    for result in &report.claim_validation.details {
        let status = if result.success_rate >= 95.0 {
            "✓"
        } else if result.success_rate >= 80.0 {
            "~"
        } else {
            "✗"
        };
        println!(
            "  {} {:.0}% loss: {:.1}% success ({}/{} tests)",
            status,
            result.loss_rate * 100.0,
            result.success_rate,
            result.tests_passed,
            result.tests_run
        );
    }

    let verdict = if report.claim_validation.validated {
        "✅ CLAIM VALIDATED"
    } else {
        "❌ CLAIM NOT VALIDATED"
    };

    println!("\n{}", verdict);
    println!(
        "Maximum tolerable loss (≥90% success): {:.1}%\n",
        report.claim_validation.max_tolerable_loss
    );

    // Save report
    let report_dir = Path::new("benchmark_reports");
    if let Err(e) = report.save(report_dir) {
        eprintln!("Failed to save report: {}", e);
    }

    println!("\n=============================================");
    println!("           Benchmark Complete!");
    println!("=============================================");
}

/// Quick validation test (not ignored)
#[tokio::test]
async fn quick_claim_validation() {
    println!("\n=== Quick 20% Loss Claim Validation ===\n");

    let file_size = 1024 * 1024; // 1 MB
    let iterations = 3;

    let mut successes = 0;
    for i in 0..iterations {
        let result = run_benchmark(file_size, 0.20, 50, 10).await;
        if result.is_success() {
            successes += 1;
        }
        println!(
            "  Run {}: {}",
            i + 1,
            if result.is_success() { "PASS" } else { "FAIL" }
        );
    }

    let success_rate = successes as f64 / iterations as f64 * 100.0;
    println!(
        "\nSuccess rate at 20% loss: {:.1}% ({}/{})",
        success_rate, successes, iterations
    );

    // We expect at least 60% success at 20% loss with 50+10 config
    assert!(
        success_rate >= 60.0,
        "Expected at least 60% success at 20% loss"
    );
}
