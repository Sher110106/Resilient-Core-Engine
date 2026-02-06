//! In-process network simulation for testing without external tools like tc/netem
//!
//! This module provides a simulated lossy network channel that can be used
//! to test the resilience of the file transfer system under various network conditions.

pub mod lossy_channel;
pub mod metrics;
pub mod network_profile;
pub mod report_generator;

pub use lossy_channel::{LossyChannel, LossyChannelConfig};
pub use metrics::{BenchmarkResult, MetricsCollector};
pub use network_profile::TestMatrixParams;
pub use report_generator::BenchmarkReport;

// Re-export for use in other tests (allow unused for now)
#[allow(unused_imports)]
pub use lossy_channel::{ChannelError, ChannelStats};
#[allow(unused_imports)]
pub use metrics::{BenchmarkMetrics, BenchmarkSummary};
#[allow(unused_imports)]
pub use network_profile::NetworkProfile;
