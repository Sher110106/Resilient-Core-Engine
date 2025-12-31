use thiserror::Error;

#[derive(Error, Debug)]
pub enum IntegrityError {
    #[error("Checksum mismatch: expected {expected:?}, got {actual:?}")]
    ChecksumMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid checksum length: expected 32 bytes, got {0}")]
    InvalidChecksumLength(usize),

    #[error("Verification failed for chunk {chunk_id}: {reason}")]
    VerificationFailed {
        chunk_id: u64,
        reason: String,
    },

    #[error("Batch verification failed: {passed} passed, {failed} failed")]
    BatchVerificationFailed {
        passed: usize,
        failed: usize,
    },
}

pub type IntegrityResult<T> = Result<T, IntegrityError>;
