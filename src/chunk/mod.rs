pub mod adaptive;
pub mod compression;
pub mod erasure;
pub mod error;
pub mod manager;
pub mod types;

pub use adaptive::{AdaptiveErasureCoder, AdaptiveErasureConfig, AdaptiveStatus};
pub use compression::{compress, decompress, CompressionError, CompressionMode};
pub use erasure::ErasureCoder;
pub use error::{ChunkError, Result};
pub use manager::ChunkManager;
pub use types::{Chunk, ChunkMetadata, FileManifest, Priority};
