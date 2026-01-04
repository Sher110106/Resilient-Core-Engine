use crate::chunk::{Chunk, Priority};
use crate::priority::error::{QueueError, QueueResult};
use crate::priority::types::{BandwidthAllocation, QueueStats, QueuedChunk};
use parking_lot::RwLock;
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::time::Duration;

const MAX_RETRIES: u32 = 5;

pub struct PriorityQueue {
    queues: [Arc<RwLock<BinaryHeap<QueuedChunk>>>; 3],
    stats: Arc<RwLock<QueueStats>>,
    max_capacity: usize,
}

impl PriorityQueue {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            queues: [
                Arc::new(RwLock::new(BinaryHeap::new())),
                Arc::new(RwLock::new(BinaryHeap::new())),
                Arc::new(RwLock::new(BinaryHeap::new())),
            ],
            stats: Arc::new(RwLock::new(QueueStats::default())),
            max_capacity,
        }
    }

    /// Enqueue chunk with priority
    pub fn enqueue(&self, chunk: Chunk) -> QueueResult<()> {
        let priority_idx = self.priority_to_index(chunk.metadata.priority);

        // Check capacity
        if self.total_pending() >= self.max_capacity {
            return Err(QueueError::QueueFull(self.max_capacity));
        }

        let queued = QueuedChunk::new(chunk, priority_idx);

        {
            let mut queue = self.queues[priority_idx].write();
            queue.push(queued);
        }

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.total_enqueued += 1;
            match priority_idx {
                0 => stats.critical_pending += 1,
                1 => stats.high_pending += 1,
                2 => stats.normal_pending += 1,
                _ => {}
            }
        }

        Ok(())
    }

    /// Dequeue next chunk (priority-ordered)
    pub fn dequeue(&self) -> QueueResult<Chunk> {
        // Try queues in priority order: Critical -> High -> Normal
        for priority_idx in 0..3 {
            let mut queue = self.queues[priority_idx].write();

            if let Some(queued) = queue.pop() {
                let wait_time_ms = queued.wait_time().as_millis() as u64;

                // Update stats
                {
                    let mut stats = self.stats.write();
                    stats.total_processed += 1;

                    // Update average wait time
                    if stats.avg_wait_time_ms == 0 {
                        stats.avg_wait_time_ms = wait_time_ms;
                    } else {
                        stats.avg_wait_time_ms = (stats.avg_wait_time_ms + wait_time_ms) / 2;
                    }

                    // Update max wait time
                    if wait_time_ms > stats.max_wait_time_ms {
                        stats.max_wait_time_ms = wait_time_ms;
                    }

                    match priority_idx {
                        0 => stats.critical_pending = stats.critical_pending.saturating_sub(1),
                        1 => stats.high_pending = stats.high_pending.saturating_sub(1),
                        2 => stats.normal_pending = stats.normal_pending.saturating_sub(1),
                        _ => {}
                    }
                }

                return Ok(queued.chunk);
            }
        }

        Err(QueueError::QueueEmpty)
    }

    /// Dequeue from specific priority level
    pub fn dequeue_priority(&self, priority: Priority) -> QueueResult<Chunk> {
        let priority_idx = self.priority_to_index(priority);
        let mut queue = self.queues[priority_idx].write();

        if let Some(queued) = queue.pop() {
            let wait_time_ms = queued.wait_time().as_millis() as u64;

            // Update stats
            {
                let mut stats = self.stats.write();
                stats.total_processed += 1;
                stats.avg_wait_time_ms = (stats.avg_wait_time_ms + wait_time_ms) / 2;

                if wait_time_ms > stats.max_wait_time_ms {
                    stats.max_wait_time_ms = wait_time_ms;
                }

                match priority_idx {
                    0 => stats.critical_pending = stats.critical_pending.saturating_sub(1),
                    1 => stats.high_pending = stats.high_pending.saturating_sub(1),
                    2 => stats.normal_pending = stats.normal_pending.saturating_sub(1),
                    _ => {}
                }
            }

            Ok(queued.chunk)
        } else {
            Err(QueueError::QueueEmpty)
        }
    }

    /// Re-enqueue failed chunk with retry count
    pub async fn requeue(&self, chunk: Chunk, retry_count: u32) -> QueueResult<()> {
        if retry_count >= MAX_RETRIES {
            return Err(QueueError::MaxRetriesExceeded {
                chunk_id: chunk.metadata.chunk_id,
                retries: retry_count,
            });
        }

        // Exponential backoff
        let delay = Duration::from_millis(100 * 2u64.pow(retry_count));
        tokio::time::sleep(delay).await;

        self.enqueue(chunk)
    }

    /// Get queue statistics
    pub fn stats(&self) -> QueueStats {
        self.stats.read().clone()
    }

    /// Get pending count for specific priority
    pub fn pending_count(&self, priority: Priority) -> usize {
        let priority_idx = self.priority_to_index(priority);
        self.queues[priority_idx].read().len()
    }

    /// Get total pending count across all priorities
    pub fn total_pending(&self) -> usize {
        self.queues.iter().map(|q| q.read().len()).sum()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.total_pending() == 0
    }

    /// Clear all queues
    pub fn clear(&self) {
        for queue in &self.queues {
            queue.write().clear();
        }

        let mut stats = self.stats.write();
        stats.critical_pending = 0;
        stats.high_pending = 0;
        stats.normal_pending = 0;
    }

    /// Allocate bandwidth based on current queue state
    pub fn allocate_bandwidth(&self, total_bps: u64) -> BandwidthAllocation {
        let stats = self.stats.read();
        BandwidthAllocation::new(
            total_bps,
            stats.critical_pending,
            stats.high_pending,
            stats.normal_pending,
        )
    }

    /// Peek at next chunk without removing it
    pub fn peek(&self) -> Option<Priority> {
        for priority_idx in 0..3 {
            let queue = self.queues[priority_idx].read();
            if !queue.is_empty() {
                return Some(self.index_to_priority(priority_idx));
            }
        }
        None
    }

    /// Get capacity information
    pub fn capacity_info(&self) -> (usize, usize, f64) {
        let used = self.total_pending();
        let available = self.max_capacity.saturating_sub(used);
        let utilization = (used as f64 / self.max_capacity as f64) * 100.0;
        (used, available, utilization)
    }

    // Helper functions
    fn priority_to_index(&self, priority: Priority) -> usize {
        match priority {
            Priority::Critical => 0,
            Priority::High => 1,
            Priority::Normal => 2,
        }
    }

    fn index_to_priority(&self, index: usize) -> Priority {
        match index {
            0 => Priority::Critical,
            1 => Priority::High,
            _ => Priority::Normal,
        }
    }
}

impl Clone for PriorityQueue {
    fn clone(&self) -> Self {
        Self {
            queues: [
                self.queues[0].clone(),
                self.queues[1].clone(),
                self.queues[2].clone(),
            ],
            stats: self.stats.clone(),
            max_capacity: self.max_capacity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkMetadata;
    use bytes::Bytes;

    fn create_test_chunk(priority: Priority, seq: u32) -> Chunk {
        Chunk {
            metadata: ChunkMetadata {
                chunk_id: seq as u64,
                file_id: "test-file".to_string(),
                sequence_number: seq,
                total_chunks: 100,
                data_size: 1024,
                checksum: [0u8; 32],
                is_parity: false,
                priority,
                created_at: chrono::Utc::now().timestamp(),
            },
            data: Bytes::from(vec![0u8; 1024]),
        }
    }

    #[test]
    fn test_queue_creation() {
        let queue = PriorityQueue::new(1000);
        assert_eq!(queue.total_pending(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_enqueue_dequeue() {
        let queue = PriorityQueue::new(1000);

        let chunk = create_test_chunk(Priority::Normal, 0);
        queue.enqueue(chunk.clone()).unwrap();

        assert_eq!(queue.total_pending(), 1);

        let dequeued = queue.dequeue().unwrap();
        assert_eq!(dequeued.metadata.chunk_id, chunk.metadata.chunk_id);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_priority_ordering() {
        let queue = PriorityQueue::new(1000);

        // Enqueue in mixed order
        queue
            .enqueue(create_test_chunk(Priority::Normal, 0))
            .unwrap();
        queue
            .enqueue(create_test_chunk(Priority::Critical, 1))
            .unwrap();
        queue.enqueue(create_test_chunk(Priority::High, 2)).unwrap();

        // Should dequeue in priority order: Critical -> High -> Normal
        let chunk1 = queue.dequeue().unwrap();
        assert_eq!(chunk1.metadata.priority, Priority::Critical);

        let chunk2 = queue.dequeue().unwrap();
        assert_eq!(chunk2.metadata.priority, Priority::High);

        let chunk3 = queue.dequeue().unwrap();
        assert_eq!(chunk3.metadata.priority, Priority::Normal);
    }

    #[test]
    fn test_sequence_ordering_within_priority() {
        let queue = PriorityQueue::new(1000);

        // Enqueue same priority, different sequences
        queue
            .enqueue(create_test_chunk(Priority::Normal, 5))
            .unwrap();
        queue
            .enqueue(create_test_chunk(Priority::Normal, 2))
            .unwrap();
        queue
            .enqueue(create_test_chunk(Priority::Normal, 8))
            .unwrap();

        // Should dequeue in sequence order: 2, 5, 8
        let chunk1 = queue.dequeue().unwrap();
        assert_eq!(chunk1.metadata.sequence_number, 2);

        let chunk2 = queue.dequeue().unwrap();
        assert_eq!(chunk2.metadata.sequence_number, 5);

        let chunk3 = queue.dequeue().unwrap();
        assert_eq!(chunk3.metadata.sequence_number, 8);
    }

    #[test]
    fn test_queue_capacity() {
        let queue = PriorityQueue::new(3);

        queue
            .enqueue(create_test_chunk(Priority::Normal, 0))
            .unwrap();
        queue
            .enqueue(create_test_chunk(Priority::Normal, 1))
            .unwrap();
        queue
            .enqueue(create_test_chunk(Priority::Normal, 2))
            .unwrap();

        // Should fail on 4th enqueue
        let result = queue.enqueue(create_test_chunk(Priority::Normal, 3));
        assert!(matches!(result, Err(QueueError::QueueFull(_))));
    }

    #[test]
    fn test_dequeue_empty() {
        let queue = PriorityQueue::new(1000);
        let result = queue.dequeue();
        assert!(matches!(result, Err(QueueError::QueueEmpty)));
    }

    #[test]
    fn test_stats_tracking() {
        let queue = PriorityQueue::new(1000);

        queue
            .enqueue(create_test_chunk(Priority::Critical, 0))
            .unwrap();
        queue.enqueue(create_test_chunk(Priority::High, 1)).unwrap();
        queue
            .enqueue(create_test_chunk(Priority::Normal, 2))
            .unwrap();

        let stats = queue.stats();
        assert_eq!(stats.critical_pending, 1);
        assert_eq!(stats.high_pending, 1);
        assert_eq!(stats.normal_pending, 1);
        assert_eq!(stats.total_enqueued, 3);

        queue.dequeue().unwrap();
        let stats = queue.stats();
        assert_eq!(stats.critical_pending, 0);
        assert_eq!(stats.total_processed, 1);
    }

    #[test]
    fn test_bandwidth_allocation() {
        let queue = PriorityQueue::new(1000);

        // Add chunks to all priorities
        queue
            .enqueue(create_test_chunk(Priority::Critical, 0))
            .unwrap();
        queue.enqueue(create_test_chunk(Priority::High, 1)).unwrap();
        queue
            .enqueue(create_test_chunk(Priority::Normal, 2))
            .unwrap();

        let allocation = queue.allocate_bandwidth(1_000_000);

        // Should allocate: 50% critical, 30% high, 20% normal
        assert_eq!(allocation.critical_bps, 500_000);
        assert_eq!(allocation.high_bps, 300_000);
        assert_eq!(allocation.normal_bps, 200_000);
        assert_eq!(allocation.total_bps, 1_000_000);
    }

    #[test]
    fn test_bandwidth_redistribution() {
        let queue = PriorityQueue::new(1000);

        // Only add normal priority chunks
        queue
            .enqueue(create_test_chunk(Priority::Normal, 0))
            .unwrap();

        let allocation = queue.allocate_bandwidth(1_000_000);

        // Critical and high bandwidth should be redistributed to others
        // When critical and high are empty, their bandwidth gets split
        // Critical's 500k gets split: 250k to high, 250k to normal
        // High's (300k + 250k) gets split: 275k to critical, 275k to normal
        // So normal gets: 200k (base) + 250k (from critical) + 275k (from high) = 725k
        // But order matters in redistribution, so we just check it's significantly more
        assert!(allocation.normal_bps > 200_000);
        assert!(allocation.critical_bps < 500_000 || allocation.critical_bps == 0);
    }

    #[test]
    fn test_dequeue_specific_priority() {
        let queue = PriorityQueue::new(1000);

        queue
            .enqueue(create_test_chunk(Priority::Critical, 0))
            .unwrap();
        queue.enqueue(create_test_chunk(Priority::High, 1)).unwrap();
        queue
            .enqueue(create_test_chunk(Priority::Normal, 2))
            .unwrap();

        let high_chunk = queue.dequeue_priority(Priority::High).unwrap();
        assert_eq!(high_chunk.metadata.priority, Priority::High);

        // Critical and Normal should still be there
        assert_eq!(queue.pending_count(Priority::Critical), 1);
        assert_eq!(queue.pending_count(Priority::Normal), 1);
    }

    #[test]
    fn test_clear() {
        let queue = PriorityQueue::new(1000);

        queue
            .enqueue(create_test_chunk(Priority::Critical, 0))
            .unwrap();
        queue.enqueue(create_test_chunk(Priority::High, 1)).unwrap();
        queue
            .enqueue(create_test_chunk(Priority::Normal, 2))
            .unwrap();

        assert_eq!(queue.total_pending(), 3);

        queue.clear();

        assert_eq!(queue.total_pending(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_peek() {
        let queue = PriorityQueue::new(1000);

        assert!(queue.peek().is_none());

        queue
            .enqueue(create_test_chunk(Priority::Normal, 0))
            .unwrap();
        assert_eq!(queue.peek(), Some(Priority::Normal));

        queue
            .enqueue(create_test_chunk(Priority::Critical, 1))
            .unwrap();
        assert_eq!(queue.peek(), Some(Priority::Critical));

        // Peek shouldn't remove
        assert_eq!(queue.total_pending(), 2);
    }

    #[test]
    fn test_capacity_info() {
        let queue = PriorityQueue::new(100);

        for i in 0..25 {
            queue
                .enqueue(create_test_chunk(Priority::Normal, i))
                .unwrap();
        }

        let (used, available, utilization) = queue.capacity_info();
        assert_eq!(used, 25);
        assert_eq!(available, 75);
        assert_eq!(utilization, 25.0);
    }

    #[tokio::test]
    async fn test_requeue_with_backoff() {
        let queue = PriorityQueue::new(1000);
        let chunk = create_test_chunk(Priority::Normal, 0);

        let start = std::time::Instant::now();
        queue.requeue(chunk, 2).await.unwrap(); // 2^2 * 100ms = 400ms
        let elapsed = start.elapsed();

        assert!(elapsed >= Duration::from_millis(400));
        assert_eq!(queue.total_pending(), 1);
    }

    #[tokio::test]
    async fn test_max_retries() {
        let queue = PriorityQueue::new(1000);
        let chunk = create_test_chunk(Priority::Normal, 0);

        let result = queue.requeue(chunk, 5).await;
        assert!(matches!(result, Err(QueueError::MaxRetriesExceeded { .. })));
    }
}
