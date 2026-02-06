//! Predefined network profiles for testing

#![allow(dead_code)]

use super::lossy_channel::LossyChannelConfig;
use serde::{Deserialize, Serialize};

/// Named network profile for benchmarking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkProfile {
    pub name: String,
    pub description: String,
    pub config: LossyChannelConfig,
}

impl NetworkProfile {
    /// Get all predefined profiles
    pub fn all_profiles() -> Vec<NetworkProfile> {
        vec![
            Self::perfect(),
            Self::lan(),
            Self::wifi(),
            Self::mobile_4g(),
            Self::satellite(),
            Self::disaster_5_percent(),
            Self::disaster_10_percent(),
            Self::disaster_15_percent(),
            Self::disaster_20_percent(),
            Self::disaster_25_percent(),
            Self::disaster_30_percent(),
            Self::severe_disaster(),
        ]
    }

    /// Perfect network - no issues
    pub fn perfect() -> Self {
        Self {
            name: "perfect".to_string(),
            description: "Perfect network with no loss or latency".to_string(),
            config: LossyChannelConfig::perfect(),
        }
    }

    /// LAN network
    pub fn lan() -> Self {
        Self {
            name: "lan".to_string(),
            description: "Typical LAN network (1Gbps, <1ms latency)".to_string(),
            config: LossyChannelConfig::lan(),
        }
    }

    /// WiFi network
    pub fn wifi() -> Self {
        Self {
            name: "wifi".to_string(),
            description: "Typical WiFi network (100Mbps, 5ms latency, 2% loss)".to_string(),
            config: LossyChannelConfig::wifi(),
        }
    }

    /// Mobile 4G network
    pub fn mobile_4g() -> Self {
        Self {
            name: "mobile_4g".to_string(),
            description: "Degraded 4G/LTE network (10Mbps, 50ms latency, 5% loss)".to_string(),
            config: LossyChannelConfig::mobile_4g(),
        }
    }

    /// Satellite link
    pub fn satellite() -> Self {
        Self {
            name: "satellite".to_string(),
            description: "Satellite link (5Mbps, 600ms latency, 3% loss)".to_string(),
            config: LossyChannelConfig {
                loss_rate: 0.03,
                latency_ms: 600,
                jitter_ms: 50,
                bandwidth_bps: 5_000_000,
                ..Default::default()
            },
        }
    }

    /// Disaster scenario with 5% packet loss
    pub fn disaster_5_percent() -> Self {
        Self {
            name: "disaster_5pct".to_string(),
            description: "Disaster scenario with 5% packet loss".to_string(),
            config: LossyChannelConfig {
                loss_rate: 0.05,
                latency_ms: 100,
                jitter_ms: 50,
                bandwidth_bps: 5_000_000,
                ..Default::default()
            },
        }
    }

    /// Disaster scenario with 10% packet loss
    pub fn disaster_10_percent() -> Self {
        Self {
            name: "disaster_10pct".to_string(),
            description: "Disaster scenario with 10% packet loss".to_string(),
            config: LossyChannelConfig {
                loss_rate: 0.10,
                latency_ms: 150,
                jitter_ms: 75,
                bandwidth_bps: 3_000_000,
                ..Default::default()
            },
        }
    }

    /// Disaster scenario with 15% packet loss
    pub fn disaster_15_percent() -> Self {
        Self {
            name: "disaster_15pct".to_string(),
            description: "Disaster scenario with 15% packet loss".to_string(),
            config: LossyChannelConfig {
                loss_rate: 0.15,
                latency_ms: 200,
                jitter_ms: 100,
                bandwidth_bps: 2_000_000,
                ..Default::default()
            },
        }
    }

    /// Disaster scenario with 20% packet loss (target claim)
    pub fn disaster_20_percent() -> Self {
        Self {
            name: "disaster_20pct".to_string(),
            description: "Disaster scenario with 20% packet loss (TARGET CLAIM)".to_string(),
            config: LossyChannelConfig::disaster_scenario(),
        }
    }

    /// Disaster scenario with 25% packet loss
    pub fn disaster_25_percent() -> Self {
        Self {
            name: "disaster_25pct".to_string(),
            description: "Disaster scenario with 25% packet loss".to_string(),
            config: LossyChannelConfig {
                loss_rate: 0.25,
                latency_ms: 300,
                jitter_ms: 150,
                bandwidth_bps: 500_000,
                ..Default::default()
            },
        }
    }

    /// Disaster scenario with 30% packet loss
    pub fn disaster_30_percent() -> Self {
        Self {
            name: "disaster_30pct".to_string(),
            description: "Disaster scenario with 30% packet loss".to_string(),
            config: LossyChannelConfig {
                loss_rate: 0.30,
                latency_ms: 400,
                jitter_ms: 200,
                bandwidth_bps: 300_000,
                ..Default::default()
            },
        }
    }

    /// Severe disaster with extreme conditions
    pub fn severe_disaster() -> Self {
        Self {
            name: "severe_disaster".to_string(),
            description: "Severe disaster with extreme network degradation".to_string(),
            config: LossyChannelConfig::severe_disaster(),
        }
    }
}

/// Test matrix parameters
#[derive(Debug, Clone)]
pub struct TestMatrixParams {
    pub file_sizes: Vec<usize>,
    pub loss_rates: Vec<f32>,
    pub latencies_ms: Vec<u64>,
    pub bandwidths_bps: Vec<u64>,
    pub concurrent_transfers: Vec<usize>,
}

impl Default for TestMatrixParams {
    fn default() -> Self {
        Self {
            file_sizes: vec![
                1024,              // 1 KB
                10 * 1024,         // 10 KB
                100 * 1024,        // 100 KB
                1024 * 1024,       // 1 MB
                10 * 1024 * 1024,  // 10 MB
                100 * 1024 * 1024, // 100 MB
            ],
            loss_rates: vec![0.0, 0.01, 0.05, 0.10, 0.15, 0.17, 0.20, 0.25, 0.30],
            latencies_ms: vec![0, 10, 50, 100, 200, 500],
            bandwidths_bps: vec![
                0,             // Unlimited
                1_000_000,     // 1 Mbps
                10_000_000,    // 10 Mbps
                100_000_000,   // 100 Mbps
                1_000_000_000, // 1 Gbps
            ],
            concurrent_transfers: vec![1, 5, 10, 25, 50],
        }
    }
}

impl TestMatrixParams {
    /// Get minimal test matrix for quick validation
    pub fn minimal() -> Self {
        Self {
            file_sizes: vec![1024 * 1024], // 1 MB only
            loss_rates: vec![0.0, 0.10, 0.20],
            latencies_ms: vec![0, 100],
            bandwidths_bps: vec![0],
            concurrent_transfers: vec![1],
        }
    }

    /// Get standard test matrix for thorough testing
    pub fn standard() -> Self {
        Self {
            file_sizes: vec![
                100 * 1024,       // 100 KB
                1024 * 1024,      // 1 MB
                10 * 1024 * 1024, // 10 MB
            ],
            loss_rates: vec![0.0, 0.05, 0.10, 0.15, 0.20, 0.25],
            latencies_ms: vec![0, 50, 100, 200],
            bandwidths_bps: vec![0, 10_000_000, 100_000_000],
            concurrent_transfers: vec![1, 5, 10],
        }
    }

    /// Get comprehensive test matrix (2000+ combinations)
    pub fn comprehensive() -> Self {
        Self::default()
    }

    /// Calculate total number of test combinations
    pub fn total_combinations(&self) -> usize {
        self.file_sizes.len()
            * self.loss_rates.len()
            * self.latencies_ms.len()
            * self.bandwidths_bps.len()
            * self.concurrent_transfers.len()
    }

    /// Generate all test case configurations
    pub fn generate_configs(&self) -> Vec<LossyChannelConfig> {
        let mut configs = Vec::with_capacity(self.total_combinations());

        for &loss_rate in &self.loss_rates {
            for &latency in &self.latencies_ms {
                for &bandwidth in &self.bandwidths_bps {
                    configs.push(LossyChannelConfig {
                        loss_rate,
                        latency_ms: latency,
                        jitter_ms: latency / 4,
                        bandwidth_bps: bandwidth,
                        ..Default::default()
                    });
                }
            }
        }

        configs
    }
}
