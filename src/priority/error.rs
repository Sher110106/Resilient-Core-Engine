use thiserror::Error;

#[derive(Error, Debug)]
pub enum QueueError {
    #[error("Queue is full (capacity: {0})")]
    QueueFull(usize),

    #[error("Queue is empty")]
    QueueEmpty,

    #[error("Max retries exceeded for chunk {chunk_id} (retries: {retries})")]
    MaxRetriesExceeded { chunk_id: u64, retries: u32 },

    #[error("Invalid priority index: {0}")]
    InvalidPriority(usize),

    #[error("Chunk not found: {0}")]
    ChunkNotFound(u64),

    #[error("Operation timeout after {0:?}")]
    Timeout(std::time::Duration),
}

pub type QueueResult<T> = Result<T, QueueError>;
