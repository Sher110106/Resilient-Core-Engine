//! Concurrent transfer stress tests
//!
//! Tests system behavior with multiple simultaneous transfers

#[path = "../simulation/mod.rs"]
mod simulation;

use chunkstream_pro::chunk::{ChunkManager, Priority};
use chunkstream_pro::integrity::IntegrityVerifier;
use chunkstream_pro::priority::PriorityQueue;
use simulation::{LossyChannel, LossyChannelConfig};
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;
use tokio::fs;
use tokio::sync::Semaphore;

/// Run a single transfer and return success/duration
async fn run_single_transfer(
    file_size: usize,
    loss_rate: f32,
    transfer_id: usize,
) -> (bool, u64, usize) {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join(format!("test_{}.bin", transfer_id));

    // Generate unique test data for this transfer
    let test_data: Vec<u8> = (0..file_size)
        .map(|i| ((i + transfer_id * 1000) % 256) as u8)
        .collect();
    fs::write(&test_file, &test_data).await.unwrap();

    let original_checksum = IntegrityVerifier::calculate_checksum(&test_data);

    let chunk_manager = ChunkManager::new(256 * 1024, 50, 10).unwrap();

    let start = Instant::now();

    // Split file
    let (manifest, chunks) = match chunk_manager
        .split_file(
            &test_file,
            format!("concurrent_test_{}", transfer_id),
            Priority::Normal,
        )
        .await
    {
        Ok(result) => result,
        Err(_) => return (false, 0, 0),
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
    let output_file = temp_dir
        .path()
        .join(format!("reconstructed_{}.bin", transfer_id));
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

    (success, duration_ms, chunks.len())
}

/// Stress test: Concurrent transfers scaling
#[tokio::test]
async fn stress_concurrent_transfers() {
    println!("\n==========================================");
    println!("STRESS TEST: Concurrent Transfer Scaling");
    println!("==========================================\n");

    let file_size = 512 * 1024; // 512 KB per transfer
    let loss_rate = 0.10;
    let concurrency_levels = vec![1, 2, 4, 8, 16];

    println!("File size: 512 KB each, Loss rate: 10%\n");

    println!(
        "{:<15} | {:<10} | {:<15} | {:<12} | {:<10}",
        "Concurrency", "Completed", "Total Time", "Avg/Transfer", "Success %"
    );
    println!("{}", "-".repeat(68));

    for &concurrency in &concurrency_levels {
        let start = Instant::now();
        let semaphore = Arc::new(Semaphore::new(concurrency));

        let mut handles = Vec::new();

        for i in 0..concurrency {
            let sem = semaphore.clone();
            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                run_single_transfer(file_size, loss_rate, i).await
            });
            handles.push(handle);
        }

        let mut successes = 0;
        let mut total_transfer_time = 0u64;

        for handle in handles {
            if let Ok((success, duration, _)) = handle.await {
                if success {
                    successes += 1;
                }
                total_transfer_time += duration;
            }
        }

        let total_time = start.elapsed().as_millis() as u64;
        let avg_time = total_transfer_time / concurrency as u64;
        let success_rate = successes as f64 / concurrency as f64 * 100.0;

        println!(
            "{:>15} | {:>4}/{:<4} | {:>12}ms | {:>10}ms | {:>9.1}%",
            concurrency, successes, concurrency, total_time, avg_time, success_rate
        );
    }
}

/// Stress test: Priority queue under concurrent load
#[tokio::test]
async fn stress_priority_queue_concurrent() {
    println!("\n================================================");
    println!("STRESS TEST: Priority Queue Under Concurrent Load");
    println!("================================================\n");

    let queue = PriorityQueue::new(1000);

    // Enqueue items with different priorities
    let num_items = 100;

    println!(
        "Enqueueing {} items with mixed priorities...\n",
        num_items * 3
    );

    let start = Instant::now();

    // Add Normal priority items
    for i in 0..num_items {
        let chunk = create_dummy_chunk(i, Priority::Normal);
        queue.enqueue(chunk).unwrap();
    }

    // Add High priority items
    for i in 0..num_items {
        let chunk = create_dummy_chunk(i + num_items, Priority::High);
        queue.enqueue(chunk).unwrap();
    }

    // Add Critical priority items
    for i in 0..num_items {
        let chunk = create_dummy_chunk(i + num_items * 2, Priority::Critical);
        queue.enqueue(chunk).unwrap();
    }

    let enqueue_time = start.elapsed().as_millis();

    println!("Enqueue time: {}ms", enqueue_time);
    println!("Queue size: {}", queue.total_pending());

    // Dequeue and verify priority order
    let dequeue_start = Instant::now();

    let mut critical_count = 0;
    let mut high_count = 0;
    let mut normal_count = 0;
    let mut last_priority = Priority::Critical;
    let mut order_violations = 0;

    while let Ok(chunk) = queue.dequeue() {
        match chunk.metadata.priority {
            Priority::Critical => {
                critical_count += 1;
                if last_priority != Priority::Critical && critical_count < num_items {
                    order_violations += 1;
                }
            }
            Priority::High => {
                high_count += 1;
                if last_priority == Priority::Normal {
                    order_violations += 1;
                }
                last_priority = Priority::High;
            }
            Priority::Normal => {
                normal_count += 1;
                last_priority = Priority::Normal;
            }
        }
    }

    let dequeue_time = dequeue_start.elapsed().as_millis();

    println!("Dequeue time: {}ms", dequeue_time);
    println!("\nDequeued by priority:");
    println!("  Critical: {}", critical_count);
    println!("  High: {}", high_count);
    println!("  Normal: {}", normal_count);
    println!("\nPriority order violations: {}", order_violations);

    assert_eq!(critical_count, num_items);
    assert_eq!(high_count, num_items);
    assert_eq!(normal_count, num_items);
    assert_eq!(order_violations, 0, "Priority ordering was violated");
}

/// Stress test: Rapid enqueue/dequeue cycles
#[tokio::test]
async fn stress_rapid_queue_cycles() {
    println!("\n==========================================");
    println!("STRESS TEST: Rapid Queue Cycles");
    println!("==========================================\n");

    let queue = PriorityQueue::new(10000);
    let cycles = 1000;

    let start = Instant::now();

    for i in 0..cycles {
        // Enqueue batch
        for j in 0..10 {
            let priority = match j % 3 {
                0 => Priority::Critical,
                1 => Priority::High,
                _ => Priority::Normal,
            };
            let chunk = create_dummy_chunk(i * 10 + j, priority);
            queue.enqueue(chunk).unwrap();
        }

        // Dequeue some
        for _ in 0..5 {
            queue.dequeue();
        }
    }

    // Drain remaining
    while queue.dequeue().is_ok() {}

    let duration = start.elapsed().as_millis();

    println!("Completed {} cycles in {}ms", cycles, duration);
    println!(
        "Operations per second: {:.0}",
        (cycles * 15) as f64 / (duration as f64 / 1000.0)
    );
}

/// Helper to create dummy chunks for queue testing
fn create_dummy_chunk(id: usize, priority: Priority) -> chunkstream_pro::chunk::Chunk {
    use bytes::Bytes;
    use chunkstream_pro::chunk::ChunkMetadata;

    chunkstream_pro::chunk::Chunk {
        metadata: ChunkMetadata {
            chunk_id: id as u64,
            file_id: format!("test_file_{}", id),
            sequence_number: id as u32,
            total_chunks: 100,
            data_size: 1024,
            checksum: [0u8; 32],
            is_parity: false,
            priority,
            created_at: chrono::Utc::now().timestamp(),
            file_size: 102400,
            file_checksum: [0u8; 32],
            data_chunks: 50,
        },
        data: Bytes::from(vec![0u8; 1024]),
    }
}

/// Stress test: Mixed priority under high loss
#[tokio::test]
async fn stress_priority_mixed_loss() {
    println!("\n================================================");
    println!("STRESS TEST: Mixed Priority Under High Loss");
    println!("================================================\n");

    let file_size = 256 * 1024; // 256 KB
    let loss_rate = 0.15; // 15% loss
    let transfers_per_priority = 3;

    println!(
        "Running {} transfers per priority level at 15% loss\n",
        transfers_per_priority
    );

    let mut results: Vec<(Priority, bool, u64)> = Vec::new();

    // Run transfers for each priority
    for priority in [Priority::Critical, Priority::High, Priority::Normal] {
        for i in 0..transfers_per_priority {
            let (success, duration, _) = run_single_transfer(
                file_size,
                loss_rate,
                i + match priority {
                    Priority::Critical => 0,
                    Priority::High => 100,
                    Priority::Normal => 200,
                },
            )
            .await;
            results.push((priority, success, duration));
        }
    }

    println!(
        "{:<12} | {:<10} | {:<12} | {:<10}",
        "Priority", "Succeeded", "Avg Time", "Success %"
    );
    println!("{}", "-".repeat(48));

    for priority in [Priority::Critical, Priority::High, Priority::Normal] {
        let priority_results: Vec<_> = results.iter().filter(|(p, _, _)| *p == priority).collect();

        let successes = priority_results.iter().filter(|(_, s, _)| *s).count();
        let total = priority_results.len();
        let avg_time: u64 = priority_results.iter().map(|(_, _, d)| d).sum::<u64>() / total as u64;
        let success_rate = successes as f64 / total as f64 * 100.0;

        println!(
            "{:<12} | {:>4}/{:<4} | {:>10}ms | {:>9.1}%",
            format!("{:?}", priority),
            successes,
            total,
            avg_time,
            success_rate
        );
    }
}
