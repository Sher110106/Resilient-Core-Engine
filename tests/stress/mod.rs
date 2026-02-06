//! Stress tests for RESILIENT file transfer system
//!
//! These tests validate system behavior under extreme conditions:
//! - Maximum packet loss thresholds
//! - Large file transfers (100MB+)
//! - Memory pressure scenarios
//! - Rapid connection/disconnection cycles

pub mod max_packet_loss;
pub mod large_file_stress;
pub mod concurrent_stress;
