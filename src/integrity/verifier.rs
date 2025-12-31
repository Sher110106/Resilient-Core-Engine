use crate::chunk::{Chunk, ChunkMetadata, FileManifest};
use crate::integrity::error::{IntegrityError, IntegrityResult};
use crate::integrity::types::{ChecksumType, IntegrityCheck, VerificationResult};
use blake3::Hasher;
use std::path::Path;
use tokio::io::AsyncReadExt;

pub struct IntegrityVerifier;

impl IntegrityVerifier {
    /// Calculate BLAKE3 checksum for byte slice
    pub fn calculate_checksum(data: &[u8]) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(data);
        *hasher.finalize().as_bytes()
    }

    /// Calculate BLAKE3 checksum for file (streaming)
    pub async fn calculate_file_checksum(path: &Path) -> IntegrityResult<[u8; 32]> {
        let mut file = tokio::fs::File::open(path).await.map_err(|e| {
            IntegrityError::FileNotFound(format!("{}: {}", path.display(), e))
        })?;

        let mut hasher = Hasher::new();
        let mut buffer = vec![0u8; 8192];

        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(*hasher.finalize().as_bytes())
    }

    /// Verify chunk integrity
    pub fn verify_chunk(chunk: &Chunk) -> IntegrityResult<()> {
        let calculated = Self::calculate_checksum(&chunk.data);
        if calculated != chunk.metadata.checksum {
            return Err(IntegrityError::ChecksumMismatch {
                expected: chunk.metadata.checksum,
                actual: calculated,
            });
        }
        Ok(())
    }

    /// Verify chunk with detailed result
    pub fn verify_chunk_detailed(chunk: &Chunk) -> VerificationResult {
        let calculated = Self::calculate_checksum(&chunk.data);
        let expected = chunk.metadata.checksum;

        if calculated == expected {
            VerificationResult::success(ChecksumType::Blake3, calculated.to_vec())
        } else {
            VerificationResult::failure(
                ChecksumType::Blake3,
                expected.to_vec(),
                calculated.to_vec(),
            )
        }
    }

    /// Batch verify multiple chunks in parallel
    pub async fn verify_chunks_parallel(chunks: &[Chunk]) -> Vec<IntegrityResult<()>> {
        use futures::stream::{self, StreamExt};

        stream::iter(chunks)
            .map(|chunk| async move { Self::verify_chunk(chunk) })
            .buffer_unordered(num_cpus::get())
            .collect()
            .await
    }

    /// Batch verify with detailed results
    pub async fn verify_chunks_parallel_detailed(chunks: &[Chunk]) -> Vec<VerificationResult> {
        use futures::stream::{self, StreamExt};

        stream::iter(chunks)
            .map(|chunk| async move { Self::verify_chunk_detailed(chunk) })
            .buffer_unordered(num_cpus::get())
            .collect()
            .await
    }

    /// Verify all chunks and return summary
    pub async fn verify_batch_summary(chunks: &[Chunk]) -> IntegrityResult<BatchVerificationSummary> {
        let results = Self::verify_chunks_parallel(chunks).await;

        let passed = results.iter().filter(|r| r.is_ok()).count();
        let failed = results.iter().filter(|r| r.is_err()).count();

        let failed_chunks: Vec<FailedChunk> = results
            .iter()
            .enumerate()
            .filter_map(|(idx, result)| {
                if let Err(e) = result {
                    Some(FailedChunk {
                        index: idx,
                        chunk_id: chunks[idx].metadata.chunk_id,
                        sequence_number: chunks[idx].metadata.sequence_number,
                        error: format!("{}", e),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(BatchVerificationSummary {
            total: chunks.len(),
            passed,
            failed,
            success_rate: (passed as f64 / chunks.len() as f64) * 100.0,
            failed_chunks,
        })
    }

    /// Verify chunk metadata consistency
    pub fn verify_metadata(metadata: &ChunkMetadata) -> IntegrityResult<()> {
        // Check sequence number is within bounds
        if metadata.sequence_number >= metadata.total_chunks {
            return Err(IntegrityError::VerificationFailed {
                chunk_id: metadata.chunk_id,
                reason: format!(
                    "Sequence number {} >= total chunks {}",
                    metadata.sequence_number, metadata.total_chunks
                ),
            });
        }

        // Check data size is reasonable
        if metadata.data_size == 0 {
            return Err(IntegrityError::VerificationFailed {
                chunk_id: metadata.chunk_id,
                reason: "Data size is zero".to_string(),
            });
        }

        Ok(())
    }

    /// Verify file manifest integrity
    pub fn verify_manifest(manifest: &FileManifest) -> IntegrityResult<()> {
        // Check chunk counts
        if manifest.total_chunks != manifest.data_chunks + manifest.parity_chunks {
            return Err(IntegrityError::VerificationFailed {
                chunk_id: 0,
                reason: format!(
                    "Total chunks {} != data chunks {} + parity chunks {}",
                    manifest.total_chunks, manifest.data_chunks, manifest.parity_chunks
                ),
            });
        }

        // Check file size consistency
        let expected_size = manifest.data_chunks as u64 * manifest.chunk_size as u64;
        if manifest.total_size > expected_size + manifest.chunk_size as u64 {
            return Err(IntegrityError::VerificationFailed {
                chunk_id: 0,
                reason: format!(
                    "File size {} inconsistent with chunk count and size",
                    manifest.total_size
                ),
            });
        }

        Ok(())
    }

    /// Create integrity check record
    pub fn create_check(data: &[u8]) -> IntegrityCheck {
        let checksum = Self::calculate_checksum(data);
        IntegrityCheck::new(ChecksumType::Blake3, checksum.to_vec())
    }

    /// Verify data against integrity check
    pub fn verify_check(data: &[u8], check: &IntegrityCheck) -> IntegrityResult<()> {
        let calculated = Self::calculate_checksum(data);
        
        if check.value.len() != 32 {
            return Err(IntegrityError::InvalidChecksumLength(check.value.len()));
        }

        let expected: [u8; 32] = check.value.as_slice().try_into().unwrap();
        
        if calculated != expected {
            return Err(IntegrityError::ChecksumMismatch {
                expected,
                actual: calculated,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct BatchVerificationSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub success_rate: f64,
    pub failed_chunks: Vec<FailedChunk>,
}

#[derive(Debug, Clone)]
pub struct FailedChunk {
    pub index: usize,
    pub chunk_id: u64,
    pub sequence_number: u32,
    pub error: String,
}

impl BatchVerificationSummary {
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }

    pub fn has_failures(&self) -> bool {
        self.failed > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{Chunk, ChunkMetadata, FileManifest, Priority};
    use bytes::Bytes;
    use tempfile::TempDir;
    use tokio::io::AsyncWriteExt;

    fn create_test_chunk(data: &[u8]) -> Chunk {
        let checksum = IntegrityVerifier::calculate_checksum(data);
        Chunk {
            metadata: ChunkMetadata {
                chunk_id: 1,
                file_id: "test-file".to_string(),
                sequence_number: 0,
                total_chunks: 1,
                data_size: data.len(),
                checksum,
                is_parity: false,
                priority: Priority::Normal,
                created_at: chrono::Utc::now().timestamp(),
            },
            data: Bytes::from(data.to_vec()),
        }
    }

    #[test]
    fn test_checksum_calculation() {
        let data = b"Hello, World!";
        let checksum = IntegrityVerifier::calculate_checksum(data);
        assert_eq!(checksum.len(), 32);

        // Same data should produce same checksum
        let checksum2 = IntegrityVerifier::calculate_checksum(data);
        assert_eq!(checksum, checksum2);

        // Different data should produce different checksum
        let data2 = b"Hello, Rust!";
        let checksum3 = IntegrityVerifier::calculate_checksum(data2);
        assert_ne!(checksum, checksum3);
    }

    #[tokio::test]
    async fn test_file_checksum() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let data = b"Test file content for checksum verification";
        let mut file = tokio::fs::File::create(&file_path).await.unwrap();
        file.write_all(data).await.unwrap();
        file.sync_all().await.unwrap();
        drop(file);

        let file_checksum = IntegrityVerifier::calculate_file_checksum(&file_path)
            .await
            .unwrap();
        let data_checksum = IntegrityVerifier::calculate_checksum(data);

        assert_eq!(file_checksum, data_checksum);
    }

    #[test]
    fn test_chunk_verification_success() {
        let data = b"test data";
        let chunk = create_test_chunk(data);

        assert!(IntegrityVerifier::verify_chunk(&chunk).is_ok());
    }

    #[test]
    fn test_chunk_verification_failure() {
        let data = b"test data";
        let mut chunk = create_test_chunk(data);

        // Corrupt the checksum
        chunk.metadata.checksum = [0u8; 32];

        let result = IntegrityVerifier::verify_chunk(&chunk);
        assert!(result.is_err());
        assert!(matches!(result, Err(IntegrityError::ChecksumMismatch { .. })));
    }

    #[test]
    fn test_verify_chunk_detailed() {
        let data = b"test data";
        let chunk = create_test_chunk(data);

        let result = IntegrityVerifier::verify_chunk_detailed(&chunk);
        assert!(result.success);
        assert_eq!(result.checksum_type, ChecksumType::Blake3);
    }

    #[test]
    fn test_verify_chunk_detailed_failure() {
        let data = b"test data";
        let mut chunk = create_test_chunk(data);
        chunk.metadata.checksum = [0u8; 32];

        let result = IntegrityVerifier::verify_chunk_detailed(&chunk);
        assert!(!result.success);
        assert_ne!(result.expected, result.actual.unwrap());
    }

    #[tokio::test]
    async fn test_parallel_verification() {
        let chunks: Vec<Chunk> = (0..10)
            .map(|i| create_test_chunk(format!("chunk {}", i).as_bytes()))
            .collect();

        let results = IntegrityVerifier::verify_chunks_parallel(&chunks).await;

        assert_eq!(results.len(), 10);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[tokio::test]
    async fn test_parallel_verification_with_failures() {
        let mut chunks: Vec<Chunk> = (0..10)
            .map(|i| create_test_chunk(format!("chunk {}", i).as_bytes()))
            .collect();

        // Corrupt some chunks
        chunks[2].metadata.checksum = [0u8; 32];
        chunks[5].metadata.checksum = [0u8; 32];
        chunks[8].metadata.checksum = [0u8; 32];

        let results = IntegrityVerifier::verify_chunks_parallel(&chunks).await;

        let failed_count = results.iter().filter(|r| r.is_err()).count();
        assert_eq!(failed_count, 3);
    }

    #[tokio::test]
    async fn test_batch_verification_summary() {
        let mut chunks: Vec<Chunk> = (0..20)
            .map(|i| create_test_chunk(format!("chunk {}", i).as_bytes()))
            .collect();

        // Corrupt 3 chunks
        chunks[5].metadata.checksum = [0u8; 32];
        chunks[10].metadata.checksum = [0u8; 32];
        chunks[15].metadata.checksum = [0u8; 32];

        let summary = IntegrityVerifier::verify_batch_summary(&chunks).await.unwrap();

        assert_eq!(summary.total, 20);
        assert_eq!(summary.passed, 17);
        assert_eq!(summary.failed, 3);
        assert!((summary.success_rate - 85.0).abs() < 0.1);
        assert_eq!(summary.failed_chunks.len(), 3);
        assert!(!summary.all_passed());
        assert!(summary.has_failures());
    }

    #[test]
    fn test_metadata_verification() {
        let metadata = ChunkMetadata {
            chunk_id: 1,
            file_id: "test".to_string(),
            sequence_number: 5,
            total_chunks: 10,
            data_size: 256 * 1024,
            checksum: [0u8; 32],
            is_parity: false,
            priority: Priority::Normal,
            created_at: chrono::Utc::now().timestamp(),
        };

        assert!(IntegrityVerifier::verify_metadata(&metadata).is_ok());
    }

    #[test]
    fn test_metadata_verification_invalid_sequence() {
        let metadata = ChunkMetadata {
            chunk_id: 1,
            file_id: "test".to_string(),
            sequence_number: 10, // >= total_chunks
            total_chunks: 10,
            data_size: 256 * 1024,
            checksum: [0u8; 32],
            is_parity: false,
            priority: Priority::Normal,
            created_at: chrono::Utc::now().timestamp(),
        };

        let result = IntegrityVerifier::verify_metadata(&metadata);
        assert!(result.is_err());
    }

    #[test]
    fn test_manifest_verification() {
        let manifest = FileManifest {
            file_id: "test".to_string(),
            filename: "test.bin".to_string(),
            total_size: 1024 * 1024,
            chunk_size: 256 * 1024,
            total_chunks: 13,
            data_chunks: 10,
            parity_chunks: 3,
            priority: Priority::Normal,
            checksum: [0u8; 32],
        };

        assert!(IntegrityVerifier::verify_manifest(&manifest).is_ok());
    }

    #[test]
    fn test_manifest_verification_invalid_counts() {
        let manifest = FileManifest {
            file_id: "test".to_string(),
            filename: "test.bin".to_string(),
            total_size: 1024 * 1024,
            chunk_size: 256 * 1024,
            total_chunks: 15, // Should be 13 (10+3)
            data_chunks: 10,
            parity_chunks: 3,
            priority: Priority::Normal,
            checksum: [0u8; 32],
        };

        let result = IntegrityVerifier::verify_manifest(&manifest);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_and_verify_check() {
        let data = b"integrity check test data";
        let check = IntegrityVerifier::create_check(data);

        assert_eq!(check.checksum_type, ChecksumType::Blake3);
        assert_eq!(check.value.len(), 32);
        assert!(check.verified_at.is_none());

        // Verify with correct data
        assert!(IntegrityVerifier::verify_check(data, &check).is_ok());

        // Verify with wrong data
        let wrong_data = b"different data";
        assert!(IntegrityVerifier::verify_check(wrong_data, &check).is_err());
    }

    #[test]
    fn test_verify_check_invalid_length() {
        let data = b"test";
        let mut check = IntegrityVerifier::create_check(data);
        check.value = vec![0u8; 16]; // Wrong length

        let result = IntegrityVerifier::verify_check(data, &check);
        assert!(matches!(result, Err(IntegrityError::InvalidChecksumLength(16))));
    }
}
