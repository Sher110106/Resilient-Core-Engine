use thiserror::Error;

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("Session not found: {0}")]
    NotFound(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Session cannot be resumed: {0}")]
    CannotResume(String),

    #[error("Session already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid session state: {0}")]
    InvalidState(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl From<sqlx::Error> for SessionError {
    fn from(err: sqlx::Error) -> Self {
        SessionError::DatabaseError(err.to_string())
    }
}

impl From<serde_json::Error> for SessionError {
    fn from(err: serde_json::Error) -> Self {
        SessionError::SerializationError(err.to_string())
    }
}

pub type SessionResult<T> = Result<T, SessionError>;
