//! Metrics and observability module
//!
//! Provides Prometheus-compatible metrics for monitoring RESILIENT transfers.
//!
//! Key metrics exposed:
//! - Transfer throughput (bytes/second)
//! - Chunk operations (sent, received, lost, recovered)
//! - Erasure coding efficiency
//! - Network conditions (latency, loss rate)
//! - Queue depths and priorities

pub mod exporter;
pub mod recorder;

pub use exporter::{start_metrics_server, MetricsConfig};
pub use recorder::{
    record_chunk_received, record_chunk_sent, record_transfer_complete, TransferMetrics,
};
