use bytes::Bytes;
use reed_solomon_erasure::galois_8::ReedSolomon;

use super::error::{ChunkError, Result};

pub struct ErasureCoder {
    data_shards: usize,   // e.g., 10
    parity_shards: usize, // e.g., 3
}

impl ErasureCoder {
    pub fn new(data_shards: usize, parity_shards: usize) -> Result<Self> {
        if data_shards == 0 || parity_shards == 0 {
            return Err(ChunkError::InvalidChunkSize(
                "Data and parity shards must be > 0".into(),
            ));
        }
        Ok(Self {
            data_shards,
            parity_shards,
        })
    }

    /// Encode chunks with parity
    pub fn encode(&self, data_chunks: Vec<Bytes>) -> Result<Vec<Bytes>> {
        if data_chunks.is_empty() {
            return Ok(Vec::new());
        }

        let rs = ReedSolomon::new(self.data_shards, self.parity_shards)
            .map_err(|e| ChunkError::ErasureCoding(e.to_string()))?;

        // Prepare shards - all must be same size
        let shard_size = data_chunks.iter().map(|c| c.len()).max().unwrap_or(0);
        let mut shards = self.prepare_shards(data_chunks, shard_size)?;

        // Encode to generate parity shards
        rs.encode(&mut shards)
            .map_err(|e| ChunkError::ErasureCoding(e.to_string()))?;

        // Convert back to Bytes
        Ok(shards.into_iter().map(Bytes::from).collect())
    }

    /// Decode chunks even with missing data
    pub fn decode(&self, chunks: Vec<Option<Bytes>>) -> Result<Vec<Bytes>> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        let rs = ReedSolomon::new(self.data_shards, self.parity_shards)
            .map_err(|e| ChunkError::ErasureCoding(e.to_string()))?;

        // Convert to Option<Vec<u8>> for reed-solomon
        let mut shards: Vec<Option<Vec<u8>>> = chunks
            .into_iter()
            .map(|opt_chunk| opt_chunk.map(|b| b.to_vec()))
            .collect();

        // Check if we have enough shards
        let present_count = shards.iter().filter(|s| s.is_some()).count();
        if present_count < self.data_shards {
            return Err(ChunkError::InsufficientChunks {
                needed: self.data_shards,
                available: present_count,
            });
        }

        // Reconstruct missing shards
        rs.reconstruct(&mut shards)
            .map_err(|e| ChunkError::ErasureCoding(e.to_string()))?;

        // Return only data shards
        Ok(shards
            .into_iter()
            .take(self.data_shards)
            .filter_map(|s| s.map(Bytes::from))
            .collect())
    }

    /// Prepare shards for encoding - pad to same size
    fn prepare_shards(&self, data_chunks: Vec<Bytes>, shard_size: usize) -> Result<Vec<Vec<u8>>> {
        let mut shards = Vec::with_capacity(self.data_shards + self.parity_shards);

        // Add data shards with padding
        for chunk in data_chunks {
            let mut shard = chunk.to_vec();
            if shard.len() < shard_size {
                shard.resize(shard_size, 0);
            }
            shards.push(shard);
        }

        // Pad with empty shards if needed
        while shards.len() < self.data_shards {
            shards.push(vec![0u8; shard_size]);
        }

        // Add empty parity shards
        for _ in 0..self.parity_shards {
            shards.push(vec![0u8; shard_size]);
        }

        Ok(shards)
    }

    pub fn data_shards(&self) -> usize {
        self.data_shards
    }

    pub fn parity_shards(&self) -> usize {
        self.parity_shards
    }

    pub fn total_shards(&self) -> usize {
        self.data_shards + self.parity_shards
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_erasure_coder_creation() {
        let coder = ErasureCoder::new(10, 3).unwrap();
        assert_eq!(coder.data_shards(), 10);
        assert_eq!(coder.parity_shards(), 3);
        assert_eq!(coder.total_shards(), 13);
    }

    #[test]
    fn test_encode_decode_no_loss() {
        let coder = ErasureCoder::new(4, 2).unwrap();

        let data = vec![
            Bytes::from_static(b"chunk1"),
            Bytes::from_static(b"chunk2"),
            Bytes::from_static(b"chunk3"),
            Bytes::from_static(b"chunk4"),
        ];

        // Encode
        let encoded = coder.encode(data.clone()).unwrap();
        assert_eq!(encoded.len(), 6); // 4 data + 2 parity

        // Decode with all chunks present
        let to_decode: Vec<Option<Bytes>> = encoded.into_iter().map(Some).collect();
        let decoded = coder.decode(to_decode).unwrap();

        assert_eq!(decoded.len(), 4);
        // Check first chunk (accounting for padding)
        assert_eq!(&decoded[0][..6], b"chunk1");
        assert_eq!(&decoded[1][..6], b"chunk2");
    }

    #[test]
    fn test_decode_with_missing_chunks() {
        let coder = ErasureCoder::new(4, 2).unwrap();

        let data = vec![
            Bytes::from_static(b"chunk1"),
            Bytes::from_static(b"chunk2"),
            Bytes::from_static(b"chunk3"),
            Bytes::from_static(b"chunk4"),
        ];

        // Encode
        let encoded = coder.encode(data.clone()).unwrap();

        // Simulate loss of 2 chunks
        let mut to_decode: Vec<Option<Bytes>> = encoded.into_iter().map(Some).collect();
        to_decode[1] = None; // Missing chunk 2
        to_decode[3] = None; // Missing chunk 4

        // Should still reconstruct successfully
        let decoded = coder.decode(to_decode).unwrap();
        assert_eq!(decoded.len(), 4);
    }

    #[test]
    fn test_decode_insufficient_chunks() {
        let coder = ErasureCoder::new(4, 2).unwrap();

        let data = vec![
            Bytes::from_static(b"chunk1"),
            Bytes::from_static(b"chunk2"),
            Bytes::from_static(b"chunk3"),
            Bytes::from_static(b"chunk4"),
        ];

        let encoded = coder.encode(data).unwrap();
        let mut to_decode: Vec<Option<Bytes>> = encoded.into_iter().map(Some).collect();

        // Remove too many chunks (3 out of 6, leaving only 3, need 4)
        to_decode[0] = None;
        to_decode[2] = None;
        to_decode[4] = None;

        let result = coder.decode(to_decode);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ChunkError::InsufficientChunks { .. }
        ));
    }
}
