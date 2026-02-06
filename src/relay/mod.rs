//! Store-and-Forward Relay Module
//!
//! Provides intermediate relay nodes for disaster scenarios where
//! direct connectivity between sender and receiver is intermittent.
//!
//! Key features:
//! - Local storage of chunks until delivery is possible
//! - Priority-aware forwarding
//! - Automatic retry with exponential backoff
//! - Mesh network support for multi-hop delivery

pub mod node;
pub mod storage;
pub mod types;

pub use node::RelayNode;
pub use storage::{RelayStorage, StoredChunk};
pub use types::{ForwardingPolicy, RelayConfig, RelayError, RelayResult, RelayStats, RouteInfo};
