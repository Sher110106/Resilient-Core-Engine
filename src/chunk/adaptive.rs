//! Adaptive erasure coding configuration
//!
//! Automatically adjusts parity shards based on observed network conditions

use crate::chunk::ErasureCoder;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Configuration for adaptive erasure coding
#[derive(Debug, Clone)]
pub struct AdaptiveErasureConfig {
    /// Base number of data shards
    pub data_shards: usize,
    /// Minimum parity shards (used under good conditions)
    pub min_parity_shards: usize,
    /// Maximum parity shards (used under severe loss)
    pub max_parity_shards: usize,
    /// Loss rate thresholds for adaptation
    pub thresholds: Vec<(f32, usize)>,
}

impl Default for AdaptiveErasureConfig {
    fn default() -> Self {
        Self {
            data_shards: 50,
            min_parity_shards: 5,
            max_parity_shards: 25,
            // (loss_rate_threshold, parity_shards)
            thresholds: vec![
                (0.05, 5),  // 0-5% loss: 5 parity (9% overhead)
                (0.10, 10), // 5-10% loss: 10 parity (17% overhead)
                (0.15, 15), // 10-15% loss: 15 parity (23% overhead)
                (0.20, 20), // 15-20% loss: 20 parity (29% overhead)
                (1.00, 25), // 20%+ loss: 25 parity (33% overhead)
            ],
        }
    }
}

impl AdaptiveErasureConfig {
    /// Create a new adaptive config with custom settings
    pub fn new(data_shards: usize, min_parity: usize, max_parity: usize) -> Self {
        Self {
            data_shards,
            min_parity_shards: min_parity,
            max_parity_shards: max_parity,
            ..Default::default()
        }
    }

    /// Get recommended parity shards for a given loss rate
    pub fn parity_for_loss_rate(&self, loss_rate: f32) -> usize {
        for &(threshold, parity) in &self.thresholds {
            if loss_rate <= threshold {
                return parity.clamp(self.min_parity_shards, self.max_parity_shards);
            }
        }
        self.max_parity_shards
    }

    /// Calculate overhead percentage for given parity
    pub fn overhead_percent(&self, parity_shards: usize) -> f64 {
        parity_shards as f64 / (self.data_shards + parity_shards) as f64 * 100.0
    }

    /// Calculate theoretical recovery capability
    pub fn recovery_capability(&self, parity_shards: usize) -> f64 {
        parity_shards as f64 / (self.data_shards + parity_shards) as f64 * 100.0
    }
}

/// Adaptive erasure coder that adjusts to network conditions
pub struct AdaptiveErasureCoder {
    config: AdaptiveErasureConfig,
    /// Current parity level
    current_parity: AtomicU32,
    /// Observed loss rate (smoothed)
    observed_loss_rate: Arc<std::sync::RwLock<f32>>,
    /// Sample count for loss rate calculation
    sample_count: AtomicU32,
    /// Lost count for loss rate calculation
    lost_count: AtomicU32,
}

impl AdaptiveErasureCoder {
    /// Create a new adaptive coder
    pub fn new(config: AdaptiveErasureConfig) -> Self {
        let initial_parity = config.min_parity_shards as u32;
        Self {
            config,
            current_parity: AtomicU32::new(initial_parity),
            observed_loss_rate: Arc::new(std::sync::RwLock::new(0.0)),
            sample_count: AtomicU32::new(0),
            lost_count: AtomicU32::new(0),
        }
    }

    /// Record a successful chunk delivery
    pub fn record_success(&self) {
        self.sample_count.fetch_add(1, Ordering::Relaxed);
        self.update_loss_rate();
    }

    /// Record a lost chunk
    pub fn record_loss(&self) {
        self.sample_count.fetch_add(1, Ordering::Relaxed);
        self.lost_count.fetch_add(1, Ordering::Relaxed);
        self.update_loss_rate();
    }

    /// Update the smoothed loss rate
    fn update_loss_rate(&self) {
        let samples = self.sample_count.load(Ordering::Relaxed);
        let lost = self.lost_count.load(Ordering::Relaxed);

        if samples >= 10 {
            // Calculate current loss rate
            let current_rate = lost as f32 / samples as f32;

            // Exponential moving average (alpha = 0.3)
            let mut rate = self.observed_loss_rate.write().unwrap();
            *rate = *rate * 0.7 + current_rate * 0.3;

            // Update parity based on new rate
            let new_parity = self.config.parity_for_loss_rate(*rate);
            self.current_parity
                .store(new_parity as u32, Ordering::Relaxed);

            // Reset counters periodically
            if samples >= 100 {
                self.sample_count.store(0, Ordering::Relaxed);
                self.lost_count.store(0, Ordering::Relaxed);
            }
        }
    }

    /// Get the current recommended parity shards
    pub fn current_parity(&self) -> usize {
        self.current_parity.load(Ordering::Relaxed) as usize
    }

    /// Get the current observed loss rate
    pub fn observed_loss_rate(&self) -> f32 {
        *self.observed_loss_rate.read().unwrap()
    }

    /// Create an ErasureCoder with current settings
    pub fn create_coder(&self) -> crate::chunk::Result<ErasureCoder> {
        ErasureCoder::new(self.config.data_shards, self.current_parity())
    }

    /// Get current configuration status
    pub fn status(&self) -> AdaptiveStatus {
        let parity = self.current_parity();
        AdaptiveStatus {
            data_shards: self.config.data_shards,
            parity_shards: parity,
            observed_loss_rate: self.observed_loss_rate(),
            overhead_percent: self.config.overhead_percent(parity),
            recovery_capability: self.config.recovery_capability(parity),
        }
    }
}

/// Status of adaptive erasure coding
#[derive(Debug, Clone)]
pub struct AdaptiveStatus {
    pub data_shards: usize,
    pub parity_shards: usize,
    pub observed_loss_rate: f32,
    pub overhead_percent: f64,
    pub recovery_capability: f64,
}

impl std::fmt::Display for AdaptiveStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}+{} shards (loss: {:.1}%, overhead: {:.1}%, recovery: {:.1}%)",
            self.data_shards,
            self.parity_shards,
            self.observed_loss_rate * 100.0,
            self.overhead_percent,
            self.recovery_capability
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AdaptiveErasureConfig::default();

        assert_eq!(config.parity_for_loss_rate(0.0), 5);
        assert_eq!(config.parity_for_loss_rate(0.05), 5);
        assert_eq!(config.parity_for_loss_rate(0.08), 10);
        assert_eq!(config.parity_for_loss_rate(0.12), 15);
        assert_eq!(config.parity_for_loss_rate(0.18), 20);
        assert_eq!(config.parity_for_loss_rate(0.25), 25);
    }

    #[test]
    fn test_adaptive_coder() {
        let config = AdaptiveErasureConfig::default();
        let coder = AdaptiveErasureCoder::new(config);

        // Initial state
        assert_eq!(coder.current_parity(), 5);

        // Simulate some losses
        for _ in 0..15 {
            coder.record_success();
        }
        for _ in 0..5 {
            coder.record_loss();
        }

        // After recording 25% loss over 20 samples
        let rate = coder.observed_loss_rate();
        println!("Observed loss rate: {:.2}%", rate * 100.0);
        println!("Current parity: {}", coder.current_parity());
        println!("Status: {}", coder.status());
    }

    #[test]
    fn test_overhead_calculation() {
        let config = AdaptiveErasureConfig::default();

        // 50 data + 10 parity = 16.7% overhead
        let overhead = config.overhead_percent(10);
        assert!((overhead - 16.67).abs() < 0.1);

        // 50 data + 25 parity = 33.3% overhead
        let overhead = config.overhead_percent(25);
        assert!((overhead - 33.33).abs() < 0.1);
    }
}
