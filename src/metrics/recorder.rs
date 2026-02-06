//! Metrics recorder for RESILIENT transfer operations
//!
//! Records various metrics about transfer performance and health.

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

static METRICS_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize metric descriptions (call once at startup)
pub fn init_metrics() {
    if METRICS_INITIALIZED.swap(true, Ordering::SeqCst) {
        return; // Already initialized
    }

    // Transfer counters
    describe_counter!("resilient_chunks_sent_total", "Total number of chunks sent");
    describe_counter!(
        "resilient_chunks_received_total",
        "Total number of chunks received"
    );
    describe_counter!(
        "resilient_chunks_lost_total",
        "Total number of chunks lost during transfer"
    );
    describe_counter!(
        "resilient_chunks_recovered_total",
        "Total number of chunks recovered via erasure coding"
    );

    // Byte counters
    describe_counter!("resilient_bytes_sent_total", "Total bytes sent");
    describe_counter!("resilient_bytes_received_total", "Total bytes received");

    // Transfer counters
    describe_counter!(
        "resilient_transfers_started_total",
        "Total number of transfers started"
    );
    describe_counter!(
        "resilient_transfers_completed_total",
        "Total number of transfers completed successfully"
    );
    describe_counter!(
        "resilient_transfers_failed_total",
        "Total number of transfers that failed"
    );

    // Gauges
    describe_gauge!(
        "resilient_active_transfers",
        "Number of currently active transfers"
    );
    describe_gauge!(
        "resilient_queue_depth",
        "Current number of items in priority queue"
    );
    describe_gauge!(
        "resilient_storage_used_bytes",
        "Current storage usage in bytes"
    );

    // Histograms
    describe_histogram!(
        "resilient_chunk_transfer_duration_seconds",
        "Time to transfer a single chunk"
    );
    describe_histogram!(
        "resilient_transfer_duration_seconds",
        "Total transfer duration"
    );
    describe_histogram!(
        "resilient_throughput_bytes_per_second",
        "Transfer throughput in bytes per second"
    );
    describe_histogram!(
        "resilient_erasure_overhead_ratio",
        "Ratio of parity shards to data shards"
    );
}

// ============== Chunk Operations ==============

/// Record a chunk being sent
pub fn record_chunk_sent(transfer_id: &str, chunk_size: usize, priority: &str) {
    counter!("resilient_chunks_sent_total", "transfer_id" => transfer_id.to_string(), "priority" => priority.to_string()).increment(1);
    counter!("resilient_bytes_sent_total", "transfer_id" => transfer_id.to_string())
        .increment(chunk_size as u64);
}

/// Record a chunk being received
pub fn record_chunk_received(transfer_id: &str, chunk_size: usize) {
    counter!("resilient_chunks_received_total", "transfer_id" => transfer_id.to_string())
        .increment(1);
    counter!("resilient_bytes_received_total", "transfer_id" => transfer_id.to_string())
        .increment(chunk_size as u64);
}

/// Record a chunk being lost
pub fn record_chunk_lost(transfer_id: &str) {
    counter!("resilient_chunks_lost_total", "transfer_id" => transfer_id.to_string()).increment(1);
}

/// Record a chunk being recovered via erasure coding
pub fn record_chunk_recovered(transfer_id: &str) {
    counter!("resilient_chunks_recovered_total", "transfer_id" => transfer_id.to_string())
        .increment(1);
}

/// Record chunk transfer duration
pub fn record_chunk_duration(duration: Duration) {
    histogram!("resilient_chunk_transfer_duration_seconds").record(duration.as_secs_f64());
}

// ============== Transfer Operations ==============

/// Record a transfer starting
pub fn record_transfer_started(transfer_id: &str, file_size: u64) {
    counter!("resilient_transfers_started_total", "transfer_id" => transfer_id.to_string())
        .increment(1);
    gauge!("resilient_active_transfers").increment(1.0);

    // Also record in histogram for distribution tracking
    histogram!("resilient_transfer_size_bytes").record(file_size as f64);
}

/// Record a transfer completing successfully
pub fn record_transfer_complete(transfer_id: &str, duration: Duration, bytes_transferred: u64) {
    counter!("resilient_transfers_completed_total", "transfer_id" => transfer_id.to_string())
        .increment(1);
    gauge!("resilient_active_transfers").decrement(1.0);

    histogram!("resilient_transfer_duration_seconds").record(duration.as_secs_f64());

    let throughput = if duration.as_secs_f64() > 0.0 {
        bytes_transferred as f64 / duration.as_secs_f64()
    } else {
        0.0
    };
    histogram!("resilient_throughput_bytes_per_second").record(throughput);
}

/// Record a transfer failing
pub fn record_transfer_failed(transfer_id: &str, reason: &str) {
    counter!("resilient_transfers_failed_total", "transfer_id" => transfer_id.to_string(), "reason" => reason.to_string()).increment(1);
    gauge!("resilient_active_transfers").decrement(1.0);
}

// ============== Queue Metrics ==============

/// Update queue depth gauge
pub fn set_queue_depth(priority: &str, depth: usize) {
    gauge!("resilient_queue_depth", "priority" => priority.to_string()).set(depth as f64);
}

// ============== Erasure Coding Metrics ==============

/// Record erasure coding configuration
pub fn record_erasure_config(data_shards: usize, parity_shards: usize) {
    let overhead = parity_shards as f64 / (data_shards + parity_shards) as f64;
    histogram!("resilient_erasure_overhead_ratio").record(overhead);
}

// ============== Storage Metrics ==============

/// Update storage usage gauge
pub fn set_storage_used(bytes: u64) {
    gauge!("resilient_storage_used_bytes").set(bytes as f64);
}

// ============== Network Metrics ==============

/// Record network latency observation
pub fn record_network_latency(peer: &str, latency_ms: u64) {
    histogram!("resilient_network_latency_ms", "peer" => peer.to_string())
        .record(latency_ms as f64);
}

/// Record packet loss rate
pub fn record_packet_loss_rate(rate: f64) {
    histogram!("resilient_packet_loss_rate").record(rate);
}

/// Helper struct to time operations and record duration
pub struct TransferMetrics {
    transfer_id: String,
    start_time: Instant,
    bytes_transferred: u64,
}

impl TransferMetrics {
    /// Start tracking a new transfer
    pub fn start(transfer_id: impl Into<String>, file_size: u64) -> Self {
        let id = transfer_id.into();
        record_transfer_started(&id, file_size);

        Self {
            transfer_id: id,
            start_time: Instant::now(),
            bytes_transferred: 0,
        }
    }

    /// Record bytes transferred
    pub fn add_bytes(&mut self, bytes: u64) {
        self.bytes_transferred += bytes;
    }

    /// Mark transfer as complete
    pub fn complete(self) {
        let duration = self.start_time.elapsed();
        record_transfer_complete(&self.transfer_id, duration, self.bytes_transferred);
    }

    /// Mark transfer as failed
    pub fn fail(self, reason: &str) {
        record_transfer_failed(&self.transfer_id, reason);
    }

    /// Get current duration
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get current throughput in bytes/second
    pub fn throughput(&self) -> f64 {
        let secs = self.start_time.elapsed().as_secs_f64();
        if secs > 0.0 {
            self.bytes_transferred as f64 / secs
        } else {
            0.0
        }
    }
}

/// Helper struct to time individual chunk operations
pub struct ChunkTimer {
    start_time: Instant,
}

impl ChunkTimer {
    /// Start timing a chunk operation
    pub fn start() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    /// Stop timing and record the duration
    pub fn stop(self) {
        record_chunk_duration(self.start_time.elapsed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_metrics() {
        // Should not panic when called multiple times
        init_metrics();
        init_metrics();
    }

    #[test]
    fn test_transfer_metrics() {
        let mut metrics = TransferMetrics::start("test-transfer", 1000);
        metrics.add_bytes(500);
        metrics.add_bytes(500);

        assert_eq!(metrics.bytes_transferred, 1000);
        assert!(metrics.elapsed() >= Duration::ZERO);
    }

    #[test]
    fn test_chunk_timer() {
        let timer = ChunkTimer::start();
        std::thread::sleep(Duration::from_millis(10));
        timer.stop(); // Should not panic
    }

    #[test]
    fn test_throughput_calculation() {
        let mut metrics = TransferMetrics::start("throughput-test", 1000);
        std::thread::sleep(Duration::from_millis(100));
        metrics.add_bytes(1000);

        let throughput = metrics.throughput();
        // Should be roughly 10KB/s (1000 bytes in 0.1 seconds)
        assert!(throughput > 5000.0 && throughput < 50000.0);
    }
}
