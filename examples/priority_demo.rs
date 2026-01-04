use bytes::Bytes;
use chunkstream_pro::chunk::{Chunk, ChunkMetadata, Priority};
use chunkstream_pro::priority::PriorityQueue;
use std::time::Duration;

fn create_test_chunk(priority: Priority, seq: u32, data: &str) -> Chunk {
    Chunk {
        metadata: ChunkMetadata {
            chunk_id: seq as u64,
            file_id: format!("file-{}", seq),
            sequence_number: seq,
            total_chunks: 100,
            data_size: data.len(),
            checksum: [0u8; 32],
            is_parity: false,
            priority,
            created_at: chrono::Utc::now().timestamp(),
        },
        data: Bytes::from(data.to_owned()),
    }
}

#[tokio::main]
async fn main() {
    println!("\n📋 ChunkStream Pro - Priority Queue Module Demo");
    println!("=================================================\n");

    // Demo 1: Queue Creation
    println!("📦 Demo 1: Priority Queue Creation");
    println!("-----------------------------------");

    let queue = PriorityQueue::new(1000);
    println!("✅ Created priority queue with capacity: 1000");
    println!("   Initial state: {} chunks pending", queue.total_pending());
    println!("   Empty: {}", queue.is_empty());

    // Demo 2: Enqueue with Different Priorities
    println!("\n\n🎯 Demo 2: Enqueue with Different Priorities");
    println!("-----------------------------------");

    queue
        .enqueue(create_test_chunk(
            Priority::Normal,
            1,
            "Normal priority data",
        ))
        .unwrap();
    queue
        .enqueue(create_test_chunk(
            Priority::Critical,
            2,
            "Critical priority data",
        ))
        .unwrap();
    queue
        .enqueue(create_test_chunk(Priority::High, 3, "High priority data"))
        .unwrap();
    queue
        .enqueue(create_test_chunk(Priority::Normal, 4, "Another normal"))
        .unwrap();
    queue
        .enqueue(create_test_chunk(Priority::Critical, 5, "Another critical"))
        .unwrap();

    println!("Enqueued 5 chunks:");
    println!(
        "   Normal: {} chunks",
        queue.pending_count(Priority::Normal)
    );
    println!("   High: {} chunks", queue.pending_count(Priority::High));
    println!(
        "   Critical: {} chunks",
        queue.pending_count(Priority::Critical)
    );
    println!("   Total: {} chunks", queue.total_pending());

    // Demo 3: Priority Ordering
    println!("\n\n⚡ Demo 3: Priority-Based Dequeue");
    println!("-----------------------------------");
    println!("Dequeuing in priority order (Critical -> High -> Normal):\n");

    for i in 1..=5 {
        let chunk = queue.dequeue().unwrap();
        println!(
            "   {}. Priority: {:?}, Chunk ID: {}, File: {}",
            i, chunk.metadata.priority, chunk.metadata.chunk_id, chunk.metadata.file_id
        );
    }

    // Demo 4: Sequence Ordering Within Priority
    println!("\n\n🔢 Demo 4: Sequence Ordering Within Same Priority");
    println!("-----------------------------------");

    // Enqueue same priority, different sequences
    queue
        .enqueue(create_test_chunk(Priority::Normal, 10, "Seq 10"))
        .unwrap();
    queue
        .enqueue(create_test_chunk(Priority::Normal, 5, "Seq 5"))
        .unwrap();
    queue
        .enqueue(create_test_chunk(Priority::Normal, 15, "Seq 15"))
        .unwrap();
    queue
        .enqueue(create_test_chunk(Priority::Normal, 3, "Seq 3"))
        .unwrap();

    println!("Enqueued 4 Normal priority chunks with sequences: 10, 5, 15, 3");
    println!("Dequeuing in sequence order:\n");

    for i in 1..=4 {
        let chunk = queue.dequeue().unwrap();
        println!("   {}. Sequence: {}", i, chunk.metadata.sequence_number);
    }

    // Demo 5: Queue Statistics
    println!("\n\n📊 Demo 5: Queue Statistics");
    println!("-----------------------------------");

    queue.clear();

    // Add various chunks
    for i in 0..10 {
        let priority = match i % 3 {
            0 => Priority::Critical,
            1 => Priority::High,
            _ => Priority::Normal,
        };
        queue
            .enqueue(create_test_chunk(priority, i, &format!("Data {}", i)))
            .unwrap();
    }

    tokio::time::sleep(Duration::from_millis(10)).await;

    // Process some
    for _ in 0..5 {
        queue.dequeue().unwrap();
    }

    let stats = queue.stats();
    println!("Queue Statistics:");
    println!("   Total enqueued: {}", stats.total_enqueued);
    println!("   Total processed: {}", stats.total_processed);
    println!("   Processing rate: {:.1}%", stats.processing_rate());
    println!("   Critical pending: {}", stats.critical_pending);
    println!("   High pending: {}", stats.high_pending);
    println!("   Normal pending: {}", stats.normal_pending);
    println!("   Avg wait time: {}ms", stats.avg_wait_time_ms);

    // Demo 6: Bandwidth Allocation
    println!("\n\n💾 Demo 6: Bandwidth Allocation");
    println!("-----------------------------------");

    let allocation = queue.allocate_bandwidth(10_000_000); // 10 Mbps

    println!("Total bandwidth: {} bps (10 Mbps)", allocation.total_bps);
    println!("\nAllocations:");
    println!(
        "   🔴 Critical: {} bps ({:.1} Mbps) - {:.0}%",
        allocation.critical_bps,
        allocation.critical_bps as f64 / 1_000_000.0,
        (allocation.critical_bps as f64 / allocation.total_bps as f64) * 100.0
    );
    println!(
        "   🟡 High: {} bps ({:.1} Mbps) - {:.0}%",
        allocation.high_bps,
        allocation.high_bps as f64 / 1_000_000.0,
        (allocation.high_bps as f64 / allocation.total_bps as f64) * 100.0
    );
    println!(
        "   🟢 Normal: {} bps ({:.1} Mbps) - {:.0}%",
        allocation.normal_bps,
        allocation.normal_bps as f64 / 1_000_000.0,
        (allocation.normal_bps as f64 / allocation.total_bps as f64) * 100.0
    );

    // Demo 7: Dynamic Bandwidth Redistribution
    println!("\n\n🔄 Demo 7: Dynamic Bandwidth Redistribution");
    println!("-----------------------------------");

    queue.clear();

    // Only normal priority chunks
    queue
        .enqueue(create_test_chunk(Priority::Normal, 0, "Normal only"))
        .unwrap();

    let allocation1 = queue.allocate_bandwidth(10_000_000);
    println!("Scenario: Only Normal priority chunks present");
    println!(
        "   Critical: {} bps ({:.1} Mbps)",
        allocation1.critical_bps,
        allocation1.critical_bps as f64 / 1_000_000.0
    );
    println!(
        "   High: {} bps ({:.1} Mbps)",
        allocation1.high_bps,
        allocation1.high_bps as f64 / 1_000_000.0
    );
    println!(
        "   Normal: {} bps ({:.1} Mbps) - BOOSTED!",
        allocation1.normal_bps,
        allocation1.normal_bps as f64 / 1_000_000.0
    );

    // Add critical chunks
    queue
        .enqueue(create_test_chunk(Priority::Critical, 1, "Critical added"))
        .unwrap();

    let allocation2 = queue.allocate_bandwidth(10_000_000);
    println!("\nScenario: Critical chunks added");
    println!(
        "   Critical: {} bps ({:.1} Mbps) - ACTIVE!",
        allocation2.critical_bps,
        allocation2.critical_bps as f64 / 1_000_000.0
    );
    println!(
        "   High: {} bps ({:.1} Mbps)",
        allocation2.high_bps,
        allocation2.high_bps as f64 / 1_000_000.0
    );
    println!(
        "   Normal: {} bps ({:.1} Mbps)",
        allocation2.normal_bps,
        allocation2.normal_bps as f64 / 1_000_000.0
    );

    // Demo 8: Queue Capacity Management
    println!("\n\n📏 Demo 8: Queue Capacity Management");
    println!("-----------------------------------");

    queue.clear();

    for i in 0..25 {
        queue
            .enqueue(create_test_chunk(
                Priority::Normal,
                i,
                &format!("Chunk {}", i),
            ))
            .unwrap();
    }

    let (used, available, utilization) = queue.capacity_info();
    println!("Capacity Info:");
    println!("   Used: {} chunks", used);
    println!("   Available: {} chunks", available);
    println!("   Utilization: {:.1}%", utilization);

    // Try to exceed capacity
    let small_queue = PriorityQueue::new(5);
    for i in 0..5 {
        small_queue
            .enqueue(create_test_chunk(Priority::Normal, i, "data"))
            .unwrap();
    }

    println!("\nTesting capacity limit (max: 5):");
    println!("   Enqueued: 5 chunks ✅");

    let result = small_queue.enqueue(create_test_chunk(Priority::Normal, 6, "overflow"));
    match result {
        Err(e) => println!("   Attempted 6th chunk: ❌ {}", e),
        Ok(_) => println!("   Attempted 6th chunk: ✅ (unexpected!)"),
    }

    // Demo 9: Retry with Backoff
    println!("\n\n🔄 Demo 9: Retry Mechanism with Exponential Backoff");
    println!("-----------------------------------");

    queue.clear();

    let chunk = create_test_chunk(Priority::Normal, 99, "Retry test");

    println!("Testing retry delays:");
    for retry in 0..4 {
        let expected_ms = 100 * 2u64.pow(retry);
        println!("   Retry {}: Waiting {}ms...", retry, expected_ms);

        let start = std::time::Instant::now();
        queue.requeue(chunk.clone(), retry).await.unwrap();
        let actual = start.elapsed();

        println!("      Actual delay: {:.0}ms ✅", actual.as_millis());
        queue.clear();
    }

    // Demo 10: Peek Without Removing
    println!("\n\n👀 Demo 10: Peek Without Removing");
    println!("-----------------------------------");

    queue.clear();
    queue
        .enqueue(create_test_chunk(Priority::High, 1, "High prio"))
        .unwrap();
    queue
        .enqueue(create_test_chunk(Priority::Normal, 2, "Normal prio"))
        .unwrap();

    println!("Queue has 2 chunks");

    if let Some(priority) = queue.peek() {
        println!("   Peeked priority: {:?}", priority);
    }

    println!("   Chunks still in queue: {}", queue.total_pending());

    let chunk = queue.dequeue().unwrap();
    println!("   Dequeued priority: {:?}", chunk.metadata.priority);
    println!("   Remaining chunks: {}", queue.total_pending());

    // Demo 11: Performance Summary
    println!("\n\n⚡ Demo 11: Performance Summary");
    println!("-----------------------------------");

    queue.clear();

    let chunk_count = 1000;
    let start = std::time::Instant::now();

    for i in 0..chunk_count {
        let priority = match i % 3 {
            0 => Priority::Critical,
            1 => Priority::High,
            _ => Priority::Normal,
        };
        queue
            .enqueue(create_test_chunk(priority, i, "data"))
            .unwrap();
    }

    let enqueue_time = start.elapsed();

    let start = std::time::Instant::now();
    let mut dequeued = 0;

    while !queue.is_empty() {
        queue.dequeue().unwrap();
        dequeued += 1;
    }

    let dequeue_time = start.elapsed();

    println!("Performance Metrics (1000 chunks):");
    println!("   Enqueue time: {:?}", enqueue_time);
    println!(
        "   Enqueue rate: {:.0} chunks/sec",
        chunk_count as f64 / enqueue_time.as_secs_f64()
    );
    println!("   Dequeue time: {:?}", dequeue_time);
    println!(
        "   Dequeue rate: {:.0} chunks/sec",
        dequeued as f64 / dequeue_time.as_secs_f64()
    );
    println!("   Total chunks processed: {}", dequeued);

    println!("\n\n🎉 All priority queue demos completed successfully!");
    println!("=================================================\n");
}
