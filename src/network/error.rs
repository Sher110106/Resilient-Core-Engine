use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Connection closed: {0}")]
    ConnectionClosed(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Receive failed: {0}")]
    ReceiveFailed(String),

    #[error("No path available for transfer")]
    NoPathAvailable,

    #[error("Path {0} is unavailable")]
    PathUnavailable(String),

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Timeout after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("QUIC error: {0}")]
    QuicError(String),

    #[error("Certificate error: {0}")]
    CertificateError(String),

    #[error("Max retries exceeded ({0} attempts)")]
    MaxRetriesExceeded(u32),
}

impl From<quinn::ConnectionError> for NetworkError {
    fn from(err: quinn::ConnectionError) -> Self {
        NetworkError::QuicError(err.to_string())
    }
}

impl From<quinn::WriteError> for NetworkError {
    fn from(err: quinn::WriteError) -> Self {
        NetworkError::SendFailed(err.to_string())
    }
}

impl From<quinn::ReadError> for NetworkError {
    fn from(err: quinn::ReadError) -> Self {
        NetworkError::ReceiveFailed(err.to_string())
    }
}

impl From<bincode::Error> for NetworkError {
    fn from(err: bincode::Error) -> Self {
        NetworkError::SerializationError(err.to_string())
    }
}

pub type NetworkResult<T> = Result<T, NetworkError>;
