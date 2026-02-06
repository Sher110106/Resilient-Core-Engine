//! Compression utilities for chunk data
//!
//! Provides LZ4 compression for reducing transfer sizes

use bytes::Bytes;

/// Compression mode for chunk data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressionMode {
    /// No compression (default)
    #[default]
    None,
    /// LZ4 fast compression
    Lz4,
}

/// Compress data using the specified mode
pub fn compress(data: &[u8], mode: CompressionMode) -> Bytes {
    match mode {
        CompressionMode::None => Bytes::copy_from_slice(data),
        CompressionMode::Lz4 => {
            let compressed = lz4_flex::compress_prepend_size(data);
            Bytes::from(compressed)
        }
    }
}

/// Decompress data using the specified mode
pub fn decompress(data: &[u8], mode: CompressionMode) -> Result<Bytes, CompressionError> {
    match mode {
        CompressionMode::None => Ok(Bytes::copy_from_slice(data)),
        CompressionMode::Lz4 => {
            let decompressed = lz4_flex::decompress_size_prepended(data)
                .map_err(|e| CompressionError::DecompressionFailed(e.to_string()))?;
            Ok(Bytes::from(decompressed))
        }
    }
}

/// Calculate compression ratio
pub fn compression_ratio(original_size: usize, compressed_size: usize) -> f64 {
    if compressed_size == 0 {
        return 0.0;
    }
    1.0 - (compressed_size as f64 / original_size as f64)
}

/// Compression error types
#[derive(Debug, thiserror::Error)]
pub enum CompressionError {
    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_compression() {
        let data = b"Hello, World!";
        let compressed = compress(data, CompressionMode::None);
        let decompressed = decompress(&compressed, CompressionMode::None).unwrap();

        assert_eq!(&decompressed[..], data);
    }

    #[test]
    fn test_lz4_compression() {
        // Create compressible data (repeated pattern)
        let data: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();

        let compressed = compress(&data, CompressionMode::Lz4);
        let decompressed = decompress(&compressed, CompressionMode::Lz4).unwrap();

        assert_eq!(&decompressed[..], &data[..]);

        // Compressed should be smaller for repetitive data
        println!(
            "Original: {} bytes, Compressed: {} bytes, Ratio: {:.1}%",
            data.len(),
            compressed.len(),
            compression_ratio(data.len(), compressed.len()) * 100.0
        );
    }

    #[test]
    fn test_lz4_highly_compressible() {
        // Highly compressible data (all zeros)
        let data = vec![0u8; 100000];

        let compressed = compress(&data, CompressionMode::Lz4);
        let decompressed = decompress(&compressed, CompressionMode::Lz4).unwrap();

        assert_eq!(&decompressed[..], &data[..]);

        // Should achieve significant compression
        let ratio = compression_ratio(data.len(), compressed.len());
        println!(
            "Zeros: {} -> {} bytes ({:.1}% reduction)",
            data.len(),
            compressed.len(),
            ratio * 100.0
        );
        assert!(ratio > 0.9, "Expected >90% compression for zeros");
    }

    #[test]
    fn test_lz4_random_data() {
        // Random data (not very compressible)
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        let data: Vec<u8> = (0..10000)
            .map(|i| {
                i.hash(&mut hasher);
                hasher.finish() as u8
            })
            .collect();

        let compressed = compress(&data, CompressionMode::Lz4);
        let decompressed = decompress(&compressed, CompressionMode::Lz4).unwrap();

        assert_eq!(&decompressed[..], &data[..]);

        println!("Random: {} -> {} bytes", data.len(), compressed.len());
    }
}
