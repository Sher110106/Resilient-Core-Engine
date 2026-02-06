//! Benchmark metrics collection and reporting

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Metrics collected during a benchmark run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
    /// Test parameters
    pub file_size: usize,
    pub packet_loss_rate: f32,
    pub latency_ms: u64,
    pub bandwidth_bps: u64,
    pub concurrent_transfers: usize,

    /// Performance metrics
    pub throughput_bps: f64,
    pub goodput_bps: f64,
    pub transfer_duration_ms: u64,
    pub first_byte_latency_ms: u64,

    /// Reliability metrics
    pub success: bool,
    pub chunks_sent: u32,
    pub chunks_lost: u32,
    pub chunks_recovered: u32,
    pub retransmissions: u32,

    /// Resource metrics
    pub peak_memory_bytes: usize,
    pub cpu_time_ms: u64,

    /// Integrity
    pub checksum_valid: bool,
    pub bytes_corrupted: usize,
}

impl Default for BenchmarkMetrics {
    fn default() -> Self {
        Self {
            file_size: 0,
            packet_loss_rate: 0.0,
            latency_ms: 0,
            bandwidth_bps: 0,
            concurrent_transfers: 1,
            throughput_bps: 0.0,
            goodput_bps: 0.0,
            transfer_duration_ms: 0,
            first_byte_latency_ms: 0,
            success: false,
            chunks_sent: 0,
            chunks_lost: 0,
            chunks_recovered: 0,
            retransmissions: 0,
            peak_memory_bytes: 0,
            cpu_time_ms: 0,
            checksum_valid: false,
            bytes_corrupted: 0,
        }
    }
}

impl BenchmarkMetrics {
    /// Calculate throughput in MB/s
    pub fn throughput_mbps(&self) -> f64 {
        self.throughput_bps / 1_000_000.0 / 8.0
    }

    /// Calculate goodput in MB/s
    pub fn goodput_mbps(&self) -> f64 {
        self.goodput_bps / 1_000_000.0 / 8.0
    }

    /// Calculate recovery rate
    pub fn recovery_rate(&self) -> f64 {
        if self.chunks_lost > 0 {
            self.chunks_recovered as f64 / self.chunks_lost as f64
        } else {
            1.0
        }
    }

    /// Calculate actual packet loss rate
    pub fn actual_loss_rate(&self) -> f64 {
        if self.chunks_sent > 0 {
            self.chunks_lost as f64 / self.chunks_sent as f64
        } else {
            0.0
        }
    }
}

/// Result of a single benchmark test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub test_name: String,
    pub metrics: BenchmarkMetrics,
    pub error: Option<String>,
    pub timestamp: i64,
}

impl BenchmarkResult {
    pub fn success(test_name: &str, metrics: BenchmarkMetrics) -> Self {
        Self {
            test_name: test_name.to_string(),
            metrics,
            error: None,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    pub fn failure(test_name: &str, error: String) -> Self {
        Self {
            test_name: test_name.to_string(),
            metrics: BenchmarkMetrics::default(),
            error: Some(error),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    pub fn is_success(&self) -> bool {
        self.error.is_none() && self.metrics.success
    }
}

/// Builder for collecting metrics during a benchmark
pub struct MetricsCollector {
    start_time: Option<Instant>,
    first_byte_time: Option<Instant>,
    metrics: BenchmarkMetrics,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            start_time: None,
            first_byte_time: None,
            metrics: BenchmarkMetrics::default(),
        }
    }

    /// Set test parameters
    pub fn with_params(
        mut self,
        file_size: usize,
        loss_rate: f32,
        latency_ms: u64,
        bandwidth_bps: u64,
    ) -> Self {
        self.metrics.file_size = file_size;
        self.metrics.packet_loss_rate = loss_rate;
        self.metrics.latency_ms = latency_ms;
        self.metrics.bandwidth_bps = bandwidth_bps;
        self
    }

    /// Start timing
    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
    }

    /// Record first byte received
    pub fn first_byte_received(&mut self) {
        if self.first_byte_time.is_none() {
            self.first_byte_time = Some(Instant::now());
            if let Some(start) = self.start_time {
                self.metrics.first_byte_latency_ms = self
                    .first_byte_time
                    .unwrap()
                    .duration_since(start)
                    .as_millis() as u64;
            }
        }
    }

    /// Record chunk sent
    pub fn chunk_sent(&mut self) {
        self.metrics.chunks_sent += 1;
    }

    /// Record chunk lost
    pub fn chunk_lost(&mut self) {
        self.metrics.chunks_lost += 1;
    }

    /// Record chunk recovered via erasure coding
    pub fn chunk_recovered(&mut self) {
        self.metrics.chunks_recovered += 1;
    }

    /// Record retransmission
    pub fn retransmission(&mut self) {
        self.metrics.retransmissions += 1;
    }

    /// Record bytes corrupted
    pub fn bytes_corrupted(&mut self, count: usize) {
        self.metrics.bytes_corrupted += count;
    }

    /// Finish and calculate final metrics
    pub fn finish(mut self, success: bool, checksum_valid: bool) -> BenchmarkMetrics {
        if let Some(start) = self.start_time {
            self.metrics.transfer_duration_ms = start.elapsed().as_millis() as u64;

            // Calculate throughput
            if self.metrics.transfer_duration_ms > 0 {
                self.metrics.throughput_bps = (self.metrics.file_size as f64 * 8.0 * 1000.0)
                    / self.metrics.transfer_duration_ms as f64;

                // Goodput excludes retransmissions overhead
                let overhead_factor = if self.metrics.chunks_sent > 0 {
                    (self.metrics.chunks_sent - self.metrics.retransmissions) as f64
                        / self.metrics.chunks_sent as f64
                } else {
                    1.0
                };
                self.metrics.goodput_bps = self.metrics.throughput_bps * overhead_factor;
            }
        }

        self.metrics.success = success;
        self.metrics.checksum_valid = checksum_valid;
        self.metrics
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary statistics for a batch of benchmarks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub total_tests: usize,
    pub passed: usize,
    pub failed: usize,
    pub success_rate: f64,
    pub avg_throughput_mbps: f64,
    pub min_throughput_mbps: f64,
    pub max_throughput_mbps: f64,
    pub avg_latency_ms: f64,
    pub total_duration_secs: f64,
}

impl BenchmarkSummary {
    pub fn from_results(results: &[BenchmarkResult], total_duration: Duration) -> Self {
        let total_tests = results.len();
        let passed = results.iter().filter(|r| r.is_success()).count();
        let failed = total_tests - passed;

        let throughputs: Vec<f64> = results
            .iter()
            .filter(|r| r.is_success())
            .map(|r| r.metrics.throughput_mbps())
            .collect();

        let avg_throughput = if !throughputs.is_empty() {
            throughputs.iter().sum::<f64>() / throughputs.len() as f64
        } else {
            0.0
        };

        let min_throughput = throughputs.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_throughput = throughputs.iter().cloned().fold(0.0, f64::max);

        let avg_latency = if !results.is_empty() {
            results
                .iter()
                .map(|r| r.metrics.first_byte_latency_ms as f64)
                .sum::<f64>()
                / results.len() as f64
        } else {
            0.0
        };

        Self {
            total_tests,
            passed,
            failed,
            success_rate: if total_tests > 0 {
                passed as f64 / total_tests as f64 * 100.0
            } else {
                0.0
            },
            avg_throughput_mbps: avg_throughput,
            min_throughput_mbps: if min_throughput.is_infinite() {
                0.0
            } else {
                min_throughput
            },
            max_throughput_mbps: max_throughput,
            avg_latency_ms: avg_latency,
            total_duration_secs: total_duration.as_secs_f64(),
        }
    }
}
