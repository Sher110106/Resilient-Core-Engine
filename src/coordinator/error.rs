use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoordinatorError {
    #[error("Transfer not found: {0}")]
    TransferNotFound(String),

    #[error("Invalid state transition: {0}")]
    InvalidStateTransition(String),

    #[error("Cannot resume: {0}")]
    CannotResume(String),

    #[error("Transfer already in progress: {0}")]
    AlreadyInProgress(String),

    #[error("Chunk error: {0}")]
    ChunkError(#[from] crate::chunk::ChunkError),

    #[error("Network error: {0}")]
    NetworkError(#[from] crate::network::NetworkError),

    #[error("Queue error: {0}")]
    QueueError(#[from] crate::priority::QueueError),

    #[error("Session error: {0}")]
    SessionError(#[from] crate::session::SessionError),

    #[error("Integrity error: {0}")]
    IntegrityError(#[from] crate::integrity::IntegrityError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type CoordinatorResult<T> = Result<T, CoordinatorError>;
