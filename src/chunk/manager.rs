use std::path::Path;

use blake3::Hasher;
use bytes::Bytes;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::erasure::ErasureCoder;
use super::error::{ChunkError, Result};
use super::types::{Chunk, ChunkMetadata, FileManifest, Priority};

pub struct ChunkManager {
    erasure_coder: ErasureCoder,
    chunk_size: usize,
}

impl ChunkManager {
    pub fn new(chunk_size: usize, data_shards: usize, parity_shards: usize) -> Result<Self> {
        let erasure_coder = ErasureCoder::new(data_shards, parity_shards)?;
        Ok(Self {
            erasure_coder,
            chunk_size,
        })
    }

    /// Split file into chunks with erasure coding
    pub async fn split_file(
        &self,
        file_path: &Path,
        file_id: String,
        priority: Priority,
    ) -> Result<(FileManifest, Vec<Chunk>)> {
        // 1. Read file and calculate file-level checksum
        let mut file = File::open(file_path).await?;
        let metadata = file.metadata().await?;
        let total_size = metadata.len();

        let mut file_hasher = Hasher::new();
        let mut file_data = Vec::new();
        file.read_to_end(&mut file_data).await?;
        file_hasher.update(&file_data);
        let file_checksum = *file_hasher.finalize().as_bytes();

        // 2. Split into chunks
        let mut data_chunks_vec = Vec::new();
        let mut offset = 0;

        while offset < file_data.len() {
            let end = std::cmp::min(offset + self.chunk_size, file_data.len());
            let chunk_data = Bytes::from(file_data[offset..end].to_vec());
            data_chunks_vec.push(chunk_data);
            offset = end;
        }

        let data_chunks_count = data_chunks_vec.len();

        // 3. Apply erasure coding
        let encoded_chunks = self.erasure_coder.encode(data_chunks_vec)?;
        let total_chunks = encoded_chunks.len();
        let parity_chunks_count = total_chunks - data_chunks_count;

        // 4. Create chunks with metadata
        let mut chunks = Vec::new();
        let created_at = chrono::Utc::now().timestamp();

        for (seq_num, chunk_data) in encoded_chunks.into_iter().enumerate() {
            let is_parity = seq_num >= data_chunks_count;

            // Calculate chunk checksum
            let mut chunk_hasher = Hasher::new();
            chunk_hasher.update(&chunk_data);
            let checksum = *chunk_hasher.finalize().as_bytes();

            let metadata = ChunkMetadata {
                chunk_id: uuid::Uuid::new_v4().as_u128() as u64,
                file_id: file_id.clone(),
                sequence_number: seq_num as u32,
                total_chunks: total_chunks as u32,
                data_size: chunk_data.len(),
                checksum,
                is_parity,
                priority,
                created_at,
                // File-level metadata for receiver reconstruction
                file_size: total_size,
                file_checksum,
                data_chunks: data_chunks_count as u32,
            };

            chunks.push(Chunk {
                metadata,
                data: chunk_data,
            });
        }

        // 5. Create manifest
        let manifest = FileManifest {
            file_id: file_id.clone(),
            filename: file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            total_size,
            chunk_size: self.chunk_size,
            total_chunks: total_chunks as u32,
            data_chunks: data_chunks_count as u32,
            parity_chunks: parity_chunks_count as u32,
            priority,
            checksum: file_checksum,
        };

        Ok((manifest, chunks))
    }

    /// Reconstruct file from chunks (even with missing chunks)
    pub async fn reconstruct_file(
        &self,
        manifest: &FileManifest,
        chunks: Vec<Chunk>,
        output_path: &Path,
    ) -> Result<()> {
        // 1. Validate we have enough chunks
        if chunks.len() < self.erasure_coder.data_shards() {
            return Err(ChunkError::InsufficientChunks {
                needed: self.erasure_coder.data_shards(),
                available: chunks.len(),
            });
        }

        // 2. Sort chunks by sequence number and prepare for decoding
        let mut sorted_chunks = chunks;
        sorted_chunks.sort_by_key(|c| c.metadata.sequence_number);

        // Create Option<Bytes> vector for all possible chunks
        let mut chunk_map: Vec<Option<Bytes>> = vec![None; manifest.total_chunks as usize];
        for chunk in sorted_chunks {
            let seq = chunk.metadata.sequence_number as usize;
            if seq < chunk_map.len() {
                // Verify chunk checksum
                let mut hasher = Hasher::new();
                hasher.update(&chunk.data);
                let calculated_checksum = *hasher.finalize().as_bytes();

                if calculated_checksum == chunk.metadata.checksum {
                    chunk_map[seq] = Some(chunk.data);
                }
            }
        }

        // 3. Apply Reed-Solomon decoding if chunks are missing
        let decoded = self.erasure_coder.decode(chunk_map)?;

        // 4. Assemble chunks in order and write to file
        let mut output_file = File::create(output_path).await?;
        let mut file_hasher = Hasher::new();
        let mut bytes_written = 0u64;

        for chunk_data in decoded {
            // Calculate how much to write from this chunk
            let remaining = manifest.total_size - bytes_written;
            let to_write = std::cmp::min(chunk_data.len() as u64, remaining) as usize;

            if to_write > 0 {
                output_file.write_all(&chunk_data[..to_write]).await?;
                file_hasher.update(&chunk_data[..to_write]);
                bytes_written += to_write as u64;
            }

            if bytes_written >= manifest.total_size {
                break;
            }
        }

        output_file.flush().await?;

        // 5. Verify file-level checksum (skip if manifest checksum is all zeros/placeholder)
        let calculated_checksum = *file_hasher.finalize().as_bytes();
        let zero_checksum = [0u8; 32];
        if manifest.checksum != zero_checksum && calculated_checksum != manifest.checksum {
            return Err(ChunkError::ChecksumMismatch {
                file_id: manifest.file_id.clone(),
            });
        }

        Ok(())
    }

    /// Adaptive chunk sizing based on network conditions
    pub fn calculate_optimal_chunk_size(&self, rtt_ms: u64, loss_rate: f32) -> usize {
        match (rtt_ms, loss_rate) {
            (rtt, loss) if rtt > 200 || loss > 0.1 => 64 * 1024, // 64KB
            (rtt, loss) if rtt > 100 || loss > 0.05 => 256 * 1024, // 256KB
            _ => 1024 * 1024,                                    // 1MB
        }
    }

    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_file(path: &Path, size: usize) -> Result<()> {
        let mut file = File::create(path).await?;
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        file.write_all(&data).await?;
        Ok(())
    }

    async fn files_equal(path1: &Path, path2: &Path) -> Result<bool> {
        let mut file1 = File::open(path1).await?;
        let mut file2 = File::open(path2).await?;

        let mut data1 = Vec::new();
        let mut data2 = Vec::new();

        file1.read_to_end(&mut data1).await?;
        file2.read_to_end(&mut data2).await?;

        Ok(data1 == data2)
    }

    #[tokio::test]
    async fn test_split_and_reconstruct() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.bin");

        // Create 1MB test file
        create_test_file(&file_path, 1024 * 1024).await.unwrap();

        let manager = ChunkManager::new(256 * 1024, 10, 3).unwrap();

        // Split
        let (manifest, chunks) = manager
            .split_file(&file_path, "test-123".into(), Priority::Normal)
            .await
            .unwrap();

        assert_eq!(chunks.len(), manifest.total_chunks as usize);
        assert_eq!(manifest.data_chunks, 4); // 1MB / 256KB = 4 chunks
                                             // With 10 data + 3 parity config, we pad to 10 data shards + 3 parity = 13 total
        assert_eq!(manifest.total_chunks, 13);

        // Reconstruct
        let output_path = temp_dir.path().join("reconstructed.bin");
        manager
            .reconstruct_file(&manifest, chunks, &output_path)
            .await
            .unwrap();

        // Verify
        assert!(files_equal(&file_path, &output_path).await.unwrap());
    }

    #[tokio::test]
    async fn test_reconstruct_with_missing_chunks() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.bin");

        // Create test file
        create_test_file(&file_path, 512 * 1024).await.unwrap();

        let manager = ChunkManager::new(128 * 1024, 4, 2).unwrap();

        // Split
        let (manifest, mut chunks) = manager
            .split_file(&file_path, "test-456".into(), Priority::High)
            .await
            .unwrap();

        // Remove 2 chunks (should still work with 4 data + 2 parity)
        chunks.remove(1);
        chunks.remove(3);

        // Reconstruct
        let output_path = temp_dir.path().join("reconstructed.bin");
        manager
            .reconstruct_file(&manifest, chunks, &output_path)
            .await
            .unwrap();

        // Verify
        assert!(files_equal(&file_path, &output_path).await.unwrap());
    }

    #[tokio::test]
    async fn test_insufficient_chunks_error() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.bin");

        create_test_file(&file_path, 256 * 1024).await.unwrap();

        let manager = ChunkManager::new(128 * 1024, 4, 2).unwrap();

        let (manifest, mut chunks) = manager
            .split_file(&file_path, "test-789".into(), Priority::Normal)
            .await
            .unwrap();

        // Remove too many chunks
        chunks.truncate(3); // Only 3 chunks, need 4

        let output_path = temp_dir.path().join("reconstructed.bin");
        let result = manager
            .reconstruct_file(&manifest, chunks, &output_path)
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ChunkError::InsufficientChunks { .. }
        ));
    }

    #[test]
    fn test_adaptive_chunk_sizing() {
        let manager = ChunkManager::new(256 * 1024, 10, 3).unwrap();

        // Good network
        assert_eq!(manager.calculate_optimal_chunk_size(50, 0.01), 1024 * 1024);

        // Medium network
        assert_eq!(manager.calculate_optimal_chunk_size(150, 0.07), 256 * 1024);

        // Poor network
        assert_eq!(manager.calculate_optimal_chunk_size(300, 0.15), 64 * 1024);
    }

    #[tokio::test]
    async fn test_small_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("small.txt");

        // Create small file (< chunk size)
        create_test_file(&file_path, 1024).await.unwrap();

        let manager = ChunkManager::new(256 * 1024, 4, 2).unwrap();

        let (manifest, chunks) = manager
            .split_file(&file_path, "small-file".into(), Priority::Critical)
            .await
            .unwrap();

        assert_eq!(manifest.data_chunks, 1); // Should be 1 chunk
        assert!(manifest.parity_chunks > 0); // Should have parity

        let output_path = temp_dir.path().join("small_reconstructed.txt");
        manager
            .reconstruct_file(&manifest, chunks, &output_path)
            .await
            .unwrap();

        assert!(files_equal(&file_path, &output_path).await.unwrap());
    }
}
