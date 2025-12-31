use bytes::Bytes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Priority {
    Critical = 0,
    High = 1,
    Normal = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub chunk_id: u64,
    pub file_id: String,
    pub sequence_number: u32,
    pub total_chunks: u32,
    pub data_size: usize,
    pub checksum: [u8; 32], // BLAKE3 hash
    pub is_parity: bool,
    pub priority: Priority,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub metadata: ChunkMetadata,
    pub data: Bytes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileManifest {
    pub file_id: String,
    pub filename: String,
    pub total_size: u64,
    pub chunk_size: usize,
    pub total_chunks: u32,
    pub data_chunks: u32,
    pub parity_chunks: u32,
    pub priority: Priority,
    pub checksum: [u8; 32], // File-level checksum
}
