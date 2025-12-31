use thiserror::Error;

#[derive(Error, Debug)]
pub enum ChunkError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Erasure coding error: {0}")]
    ErasureCoding(String),

    #[error("Insufficient chunks for reconstruction: need {needed}, have {available}")]
    InsufficientChunks { needed: usize, available: usize },

    #[error("Invalid chunk size: {0}")]
    InvalidChunkSize(String),

    #[error("Checksum mismatch for file {file_id}")]
    ChecksumMismatch { file_id: String },

    #[error("Invalid shard size: all shards must be the same size")]
    InvalidShardSize,
}

pub type Result<T> = std::result::Result<T, ChunkError>;
