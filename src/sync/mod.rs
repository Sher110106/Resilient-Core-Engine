//! Delta synchronization module
//!
//! Provides rsync-style delta transfer capabilities using rolling checksums
//! and strong hashes for efficient block-level file synchronization.

pub mod delta;
pub mod rolling_hash;
pub mod signature;

pub use delta::{DeltaBuilder, DeltaInstruction, DeltaPatch};
pub use rolling_hash::{Adler32Rolling, RollingHash};
pub use signature::{BlockSignature, FileSignature, SignatureBuilder};
