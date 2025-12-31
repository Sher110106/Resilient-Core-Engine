use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChecksumType {
    Blake3,
    Sha256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityCheck {
    pub checksum_type: ChecksumType,
    pub value: Vec<u8>,
    pub verified_at: Option<i64>,
}

impl IntegrityCheck {
    pub fn new(checksum_type: ChecksumType, value: Vec<u8>) -> Self {
        Self {
            checksum_type,
            value,
            verified_at: None,
        }
    }

    pub fn mark_verified(&mut self) {
        self.verified_at = Some(chrono::Utc::now().timestamp());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub success: bool,
    pub checksum_type: ChecksumType,
    pub expected: Vec<u8>,
    pub actual: Option<Vec<u8>>,
    pub verified_at: i64,
}

impl VerificationResult {
    pub fn success(checksum_type: ChecksumType, checksum: Vec<u8>) -> Self {
        Self {
            success: true,
            checksum_type,
            expected: checksum.clone(),
            actual: Some(checksum),
            verified_at: chrono::Utc::now().timestamp(),
        }
    }

    pub fn failure(checksum_type: ChecksumType, expected: Vec<u8>, actual: Vec<u8>) -> Self {
        Self {
            success: false,
            checksum_type,
            expected,
            actual: Some(actual),
            verified_at: chrono::Utc::now().timestamp(),
        }
    }
}
