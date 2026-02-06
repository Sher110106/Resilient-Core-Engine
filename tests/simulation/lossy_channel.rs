//! Simulated lossy network channel for in-process testing
//!
//! This simulates network conditions like packet loss, latency, jitter,
//! and bandwidth limits without requiring external tools like tc/netem.

#![allow(dead_code)]

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Configuration for the lossy channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LossyChannelConfig {
    /// Packet loss rate (0.0 - 1.0)
    pub loss_rate: f32,
    /// Base latency in milliseconds
    pub latency_ms: u64,
    /// Jitter (variance in latency) in milliseconds
    pub jitter_ms: u64,
    /// Bandwidth limit in bytes per second (0 = unlimited)
    pub bandwidth_bps: u64,
    /// Corruption rate (0.0 - 1.0) - chance of bit flip
    pub corruption_rate: f32,
    /// Duplicate packet rate (0.0 - 1.0)
    pub duplicate_rate: f32,
    /// Reorder rate (0.0 - 1.0) - chance of out-of-order delivery
    pub reorder_rate: f32,
}

impl Default for LossyChannelConfig {
    fn default() -> Self {
        Self {
            loss_rate: 0.0,
            latency_ms: 0,
            jitter_ms: 0,
            bandwidth_bps: 0,
            corruption_rate: 0.0,
            duplicate_rate: 0.0,
            reorder_rate: 0.0,
        }
    }
}

impl LossyChannelConfig {
    /// Create a perfect network (no loss, no latency)
    pub fn perfect() -> Self {
        Self::default()
    }

    /// Create a typical LAN network
    pub fn lan() -> Self {
        Self {
            loss_rate: 0.001,
            latency_ms: 1,
            jitter_ms: 1,
            bandwidth_bps: 1_000_000_000, // 1 Gbps
            ..Default::default()
        }
    }

    /// Create a typical WiFi network
    pub fn wifi() -> Self {
        Self {
            loss_rate: 0.02,
            latency_ms: 5,
            jitter_ms: 10,
            bandwidth_bps: 100_000_000, // 100 Mbps
            ..Default::default()
        }
    }

    /// Create a degraded 4G/LTE network
    pub fn mobile_4g() -> Self {
        Self {
            loss_rate: 0.05,
            latency_ms: 50,
            jitter_ms: 30,
            bandwidth_bps: 10_000_000, // 10 Mbps
            ..Default::default()
        }
    }

    /// Create a disaster scenario network (your target)
    pub fn disaster_scenario() -> Self {
        Self {
            loss_rate: 0.20,
            latency_ms: 200,
            jitter_ms: 100,
            bandwidth_bps: 1_000_000, // 1 Mbps
            ..Default::default()
        }
    }

    /// Create a severe disaster scenario
    pub fn severe_disaster() -> Self {
        Self {
            loss_rate: 0.30,
            latency_ms: 500,
            jitter_ms: 200,
            bandwidth_bps: 500_000, // 500 Kbps
            ..Default::default()
        }
    }

    /// Create custom loss rate config
    pub fn with_loss(loss_rate: f32) -> Self {
        Self {
            loss_rate,
            ..Default::default()
        }
    }

    /// Create config with specific parameters
    pub fn custom(loss_rate: f32, latency_ms: u64, bandwidth_bps: u64) -> Self {
        Self {
            loss_rate,
            latency_ms,
            jitter_ms: latency_ms / 4,
            bandwidth_bps,
            ..Default::default()
        }
    }
}

/// Statistics collected by the lossy channel
#[derive(Debug, Default)]
pub struct ChannelStats {
    pub packets_sent: AtomicU64,
    pub packets_lost: AtomicU64,
    pub packets_corrupted: AtomicU64,
    pub packets_duplicated: AtomicU64,
    pub packets_reordered: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub total_latency_ms: AtomicU64,
}

impl ChannelStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn actual_loss_rate(&self) -> f32 {
        let sent = self.packets_sent.load(Ordering::Relaxed);
        let lost = self.packets_lost.load(Ordering::Relaxed);
        if sent > 0 {
            lost as f32 / sent as f32
        } else {
            0.0
        }
    }

    pub fn average_latency_ms(&self) -> f64 {
        let sent = self.packets_sent.load(Ordering::Relaxed);
        let total = self.total_latency_ms.load(Ordering::Relaxed);
        if sent > 0 {
            total as f64 / sent as f64
        } else {
            0.0
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "Sent: {}, Lost: {} ({:.1}%), Corrupted: {}, Duplicated: {}, Avg Latency: {:.1}ms",
            self.packets_sent.load(Ordering::Relaxed),
            self.packets_lost.load(Ordering::Relaxed),
            self.actual_loss_rate() * 100.0,
            self.packets_corrupted.load(Ordering::Relaxed),
            self.packets_duplicated.load(Ordering::Relaxed),
            self.average_latency_ms()
        )
    }
}

/// Error type for lossy channel operations
#[derive(Debug, Clone)]
pub enum ChannelError {
    PacketLost,
    PacketCorrupted,
    Timeout,
}

impl std::fmt::Display for ChannelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelError::PacketLost => write!(f, "Packet lost"),
            ChannelError::PacketCorrupted => write!(f, "Packet corrupted"),
            ChannelError::Timeout => write!(f, "Operation timed out"),
        }
    }
}

impl std::error::Error for ChannelError {}

/// A simulated lossy network channel
pub struct LossyChannel {
    config: LossyChannelConfig,
    stats: Arc<ChannelStats>,
}

impl LossyChannel {
    /// Create a new lossy channel with the given configuration
    pub fn new(config: LossyChannelConfig) -> Self {
        Self {
            config,
            stats: Arc::new(ChannelStats::new()),
        }
    }

    /// Get a reference to the channel statistics
    pub fn stats(&self) -> Arc<ChannelStats> {
        self.stats.clone()
    }

    /// Get the configuration
    pub fn config(&self) -> &LossyChannelConfig {
        &self.config
    }

    /// Simulate sending data through the lossy channel
    /// Returns the data if successful, or an error if the packet was lost/corrupted
    pub async fn send(&self, data: &[u8]) -> Result<Vec<u8>, ChannelError> {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::from_entropy();

        self.stats.packets_sent.fetch_add(1, Ordering::Relaxed);
        self.stats
            .bytes_sent
            .fetch_add(data.len() as u64, Ordering::Relaxed);

        // 1. Simulate packet loss
        if rng.gen::<f32>() < self.config.loss_rate {
            self.stats.packets_lost.fetch_add(1, Ordering::Relaxed);
            return Err(ChannelError::PacketLost);
        }

        // 2. Simulate latency with jitter
        let latency = if self.config.latency_ms > 0 {
            let jitter = if self.config.jitter_ms > 0 {
                rng.gen_range(0..self.config.jitter_ms)
            } else {
                0
            };
            let total_latency = self.config.latency_ms + jitter;
            self.stats
                .total_latency_ms
                .fetch_add(total_latency, Ordering::Relaxed);
            total_latency
        } else {
            0
        };

        if latency > 0 {
            sleep(Duration::from_millis(latency)).await;
        }

        // 3. Simulate bandwidth limit
        if self.config.bandwidth_bps > 0 {
            let transfer_time_ms = (data.len() as u64 * 8 * 1000) / self.config.bandwidth_bps;
            if transfer_time_ms > 0 {
                sleep(Duration::from_millis(transfer_time_ms)).await;
            }
        }

        // 4. Simulate corruption
        let mut result = data.to_vec();
        if self.config.corruption_rate > 0.0 && rng.gen::<f32>() < self.config.corruption_rate {
            self.stats.packets_corrupted.fetch_add(1, Ordering::Relaxed);
            // Flip a random bit
            if !result.is_empty() {
                let byte_idx = rng.gen_range(0..result.len());
                let bit_idx = rng.gen_range(0..8);
                result[byte_idx] ^= 1 << bit_idx;
            }
        }

        Ok(result)
    }

    /// Simulate sending data with automatic retry on loss
    pub async fn send_with_retry(
        &self,
        data: &[u8],
        max_retries: u32,
    ) -> Result<Vec<u8>, ChannelError> {
        let mut attempts = 0;
        loop {
            match self.send(data).await {
                Ok(result) => return Ok(result),
                Err(ChannelError::PacketLost) if attempts < max_retries => {
                    attempts += 1;
                    // Exponential backoff
                    let backoff = Duration::from_millis(10 * (1 << attempts.min(6)));
                    sleep(backoff).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Simulate sending multiple chunks and return which ones succeeded
    pub async fn send_chunks(&self, chunks: &[Vec<u8>]) -> Vec<Result<Vec<u8>, ChannelError>> {
        let mut results = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            results.push(self.send(chunk).await);
        }
        results
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.stats.packets_sent.store(0, Ordering::Relaxed);
        self.stats.packets_lost.store(0, Ordering::Relaxed);
        self.stats.packets_corrupted.store(0, Ordering::Relaxed);
        self.stats.packets_duplicated.store(0, Ordering::Relaxed);
        self.stats.packets_reordered.store(0, Ordering::Relaxed);
        self.stats.bytes_sent.store(0, Ordering::Relaxed);
        self.stats.total_latency_ms.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_perfect_channel() {
        let channel = LossyChannel::new(LossyChannelConfig::perfect());
        let data = vec![1, 2, 3, 4, 5];

        let result = channel.send(&data).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), data);

        let stats = channel.stats();
        assert_eq!(stats.packets_sent.load(Ordering::Relaxed), 1);
        assert_eq!(stats.packets_lost.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_total_loss_channel() {
        let config = LossyChannelConfig {
            loss_rate: 1.0, // 100% loss
            ..Default::default()
        };
        let channel = LossyChannel::new(config);
        let data = vec![1, 2, 3, 4, 5];

        let result = channel.send(&data).await;
        assert!(matches!(result, Err(ChannelError::PacketLost)));

        let stats = channel.stats();
        assert_eq!(stats.packets_sent.load(Ordering::Relaxed), 1);
        assert_eq!(stats.packets_lost.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_partial_loss() {
        let config = LossyChannelConfig {
            loss_rate: 0.5, // 50% loss
            ..Default::default()
        };
        let channel = LossyChannel::new(config);
        let data = vec![1, 2, 3, 4, 5];

        // Send many packets to get statistical significance
        let mut successes = 0;
        let total = 1000;

        for _ in 0..total {
            if channel.send(&data).await.is_ok() {
                successes += 1;
            }
        }

        // Should be roughly 50% success rate (with some variance)
        let success_rate = successes as f64 / total as f64;
        assert!(success_rate > 0.4 && success_rate < 0.6);
    }

    #[tokio::test]
    async fn test_disaster_scenario() {
        let channel = LossyChannel::new(LossyChannelConfig::disaster_scenario());
        let data = vec![1, 2, 3, 4, 5];

        // Send many packets
        let total = 100;
        for _ in 0..total {
            let _ = channel.send(&data).await;
        }

        let stats = channel.stats();
        let loss_rate = stats.actual_loss_rate();
        // Should be approximately 20% loss
        assert!(loss_rate > 0.10 && loss_rate < 0.30);
    }

    #[tokio::test]
    async fn test_send_with_retry() {
        let config = LossyChannelConfig {
            loss_rate: 0.5,
            ..Default::default()
        };
        let channel = LossyChannel::new(config);
        let data = vec![1, 2, 3, 4, 5];

        // With retries, should eventually succeed most of the time
        let mut successes = 0;
        let total = 100;

        for _ in 0..total {
            if channel.send_with_retry(&data, 5).await.is_ok() {
                successes += 1;
            }
        }

        // With 5 retries and 50% loss, should succeed most of the time
        // P(fail all 6 attempts) = 0.5^6 = 1.5%
        assert!(successes > 90);
    }
}
