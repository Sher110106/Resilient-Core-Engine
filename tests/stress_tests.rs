//! Stress tests for RESILIENT file transfer system
//!
//! Run with: cargo test --test stress_tests -- --nocapture

#[path = "simulation/mod.rs"]
mod simulation;

#[path = "stress/max_packet_loss.rs"]
mod max_packet_loss;

#[path = "stress/large_file_stress.rs"]
mod large_file_stress;

#[path = "stress/concurrent_stress.rs"]
mod concurrent_stress;

// Re-export tests from submodules
pub use concurrent_stress::*;
pub use large_file_stress::*;
pub use max_packet_loss::*;
