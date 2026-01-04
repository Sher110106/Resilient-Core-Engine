use crate::chunk::Chunk;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct QueuedChunk {
    pub chunk: Chunk,
    pub enqueued_at: Instant,
    pub retry_count: u32,
    pub priority_idx: usize,
}

impl QueuedChunk {
    pub fn new(chunk: Chunk, priority_idx: usize) -> Self {
        Self {
            chunk,
            enqueued_at: Instant::now(),
            retry_count: 0,
            priority_idx,
        }
    }

    pub fn wait_time(&self) -> std::time::Duration {
        self.enqueued_at.elapsed()
    }
}

impl PartialEq for QueuedChunk {
    fn eq(&self, other: &Self) -> bool {
        self.chunk.metadata.sequence_number == other.chunk.metadata.sequence_number
            && self.chunk.metadata.file_id == other.chunk.metadata.file_id
    }
}

impl Eq for QueuedChunk {}

impl PartialOrd for QueuedChunk {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedChunk {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Lower sequence numbers have higher priority (sent first)
        // Reverse ordering for max-heap behavior
        other
            .chunk
            .metadata
            .sequence_number
            .cmp(&self.chunk.metadata.sequence_number)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueueStats {
    pub critical_pending: usize,
    pub high_pending: usize,
    pub normal_pending: usize,
    pub total_processed: u64,
    pub total_enqueued: u64,
    pub avg_wait_time_ms: u64,
    pub max_wait_time_ms: u64,
}

impl QueueStats {
    pub fn total_pending(&self) -> usize {
        self.critical_pending + self.high_pending + self.normal_pending
    }

    pub fn processing_rate(&self) -> f64 {
        if self.total_enqueued == 0 {
            0.0
        } else {
            (self.total_processed as f64 / self.total_enqueued as f64) * 100.0
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthAllocation {
    pub critical_bps: u64,
    pub high_bps: u64,
    pub normal_bps: u64,
    pub total_bps: u64,
}

impl BandwidthAllocation {
    pub fn new(
        total_bps: u64,
        critical_pending: usize,
        high_pending: usize,
        normal_pending: usize,
    ) -> Self {
        // Default ratios: Critical 50%, High 30%, Normal 20%
        let mut critical_bps = total_bps / 2;
        let mut high_bps = total_bps * 3 / 10;
        let mut normal_bps = total_bps / 5;

        // Redistribute unused bandwidth
        if critical_pending == 0 {
            high_bps += critical_bps / 2;
            normal_bps += critical_bps / 2;
            critical_bps = 0;
        }

        if high_pending == 0 {
            critical_bps += high_bps / 2;
            normal_bps += high_bps / 2;
            high_bps = 0;
        }

        if normal_pending == 0 {
            critical_bps += normal_bps / 2;
            high_bps += normal_bps / 2;
            normal_bps = 0;
        }

        Self {
            critical_bps,
            high_bps,
            normal_bps,
            total_bps,
        }
    }

    pub fn get_allocation(&self, priority_idx: usize) -> u64 {
        match priority_idx {
            0 => self.critical_bps,
            1 => self.high_bps,
            2 => self.normal_bps,
            _ => 0,
        }
    }
}
