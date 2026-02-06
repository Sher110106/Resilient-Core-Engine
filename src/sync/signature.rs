//! File signature generation for delta transfer
//!
//! Generates block signatures that can be used to compute deltas
//! between file versions, similar to rsync's signature/delta approach.

use crate::sync::rolling_hash::Adler32Rolling;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};

/// Default block size for signatures (4KB)
pub const DEFAULT_BLOCK_SIZE: usize = 4096;

/// Signature for a single block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockSignature {
    /// Block index in the file
    pub index: u32,
    /// Byte offset in the file
    pub offset: u64,
    /// Block length (may be less than block_size for last block)
    pub length: u32,
    /// Weak rolling checksum (Adler-32)
    pub weak_hash: u32,
    /// Strong cryptographic hash (Blake3, truncated to 128 bits)
    pub strong_hash: [u8; 16],
}

impl BlockSignature {
    /// Create a new block signature
    pub fn new(index: u32, offset: u64, data: &[u8]) -> Self {
        let weak_hash = Adler32Rolling::checksum(data);
        let full_hash = blake3::hash(data);
        let mut strong_hash = [0u8; 16];
        strong_hash.copy_from_slice(&full_hash.as_bytes()[..16]);

        Self {
            index,
            offset,
            length: data.len() as u32,
            weak_hash,
            strong_hash,
        }
    }

    /// Verify if data matches this signature
    pub fn matches(&self, data: &[u8]) -> bool {
        if data.len() != self.length as usize {
            return false;
        }

        // First check weak hash (fast)
        let weak = Adler32Rolling::checksum(data);
        if weak != self.weak_hash {
            return false;
        }

        // Then verify with strong hash (slower but definitive)
        let full_hash = blake3::hash(data);
        full_hash.as_bytes()[..16] == self.strong_hash
    }
}

/// Complete file signature containing all block signatures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSignature {
    /// Block size used for this signature
    pub block_size: u32,
    /// Total file size
    pub file_size: u64,
    /// Full file hash (Blake3)
    pub file_hash: [u8; 32],
    /// All block signatures
    pub blocks: Vec<BlockSignature>,
}

impl FileSignature {
    /// Check if the file signature is empty (no blocks)
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Get number of blocks
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Build a lookup table for fast weak hash matching
    pub fn build_lookup(&self) -> SignatureLookup {
        let mut lookup: HashMap<u32, Vec<usize>> = HashMap::new();

        for (idx, block) in self.blocks.iter().enumerate() {
            lookup.entry(block.weak_hash).or_default().push(idx);
        }

        SignatureLookup {
            signature: self,
            weak_lookup: lookup,
        }
    }

    /// Serialize to bytes (using bincode)
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize signature")
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }
}

/// Lookup structure for fast block matching
pub struct SignatureLookup<'a> {
    signature: &'a FileSignature,
    weak_lookup: HashMap<u32, Vec<usize>>,
}

impl<'a> SignatureLookup<'a> {
    /// Find blocks matching the given weak hash
    pub fn find_weak_matches(&self, weak_hash: u32) -> Option<&Vec<usize>> {
        self.weak_lookup.get(&weak_hash)
    }

    /// Get a block signature by index
    pub fn get_block(&self, idx: usize) -> Option<&BlockSignature> {
        self.signature.blocks.get(idx)
    }

    /// Get the block size
    pub fn block_size(&self) -> usize {
        self.signature.block_size as usize
    }
}

/// Builder for file signatures
pub struct SignatureBuilder {
    block_size: usize,
}

impl SignatureBuilder {
    /// Create a new signature builder with default block size
    pub fn new() -> Self {
        Self {
            block_size: DEFAULT_BLOCK_SIZE,
        }
    }

    /// Set the block size
    pub fn block_size(mut self, size: usize) -> Self {
        self.block_size = size;
        self
    }

    /// Build signature from a byte slice
    pub fn build_from_bytes(&self, data: &[u8]) -> FileSignature {
        let file_hash = blake3::hash(data);
        let mut file_hash_array = [0u8; 32];
        file_hash_array.copy_from_slice(file_hash.as_bytes());

        let mut blocks = Vec::new();
        let mut offset = 0u64;
        let mut index = 0u32;

        for chunk in data.chunks(self.block_size) {
            blocks.push(BlockSignature::new(index, offset, chunk));
            offset += chunk.len() as u64;
            index += 1;
        }

        FileSignature {
            block_size: self.block_size as u32,
            file_size: data.len() as u64,
            file_hash: file_hash_array,
            blocks,
        }
    }

    /// Build signature from a reader (for large files)
    pub fn build_from_reader<R: Read + Seek>(
        &self,
        reader: &mut R,
    ) -> std::io::Result<FileSignature> {
        // Get file size
        let file_size = reader.seek(SeekFrom::End(0))?;
        reader.seek(SeekFrom::Start(0))?;

        // Calculate file hash
        let mut hasher = blake3::Hasher::new();
        let mut buffer = vec![0u8; self.block_size];

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let file_hash = hasher.finalize();
        let mut file_hash_array = [0u8; 32];
        file_hash_array.copy_from_slice(file_hash.as_bytes());

        // Reset and build block signatures
        reader.seek(SeekFrom::Start(0))?;

        let mut blocks = Vec::new();
        let mut offset = 0u64;
        let mut index = 0u32;

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            blocks.push(BlockSignature::new(index, offset, &buffer[..bytes_read]));
            offset += bytes_read as u64;
            index += 1;
        }

        Ok(FileSignature {
            block_size: self.block_size as u32,
            file_size,
            file_hash: file_hash_array,
            blocks,
        })
    }
}

impl Default for SignatureBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_block_signature() {
        let data = b"Hello, World! This is a test block.";
        let sig = BlockSignature::new(0, 0, data);

        assert!(sig.matches(data));
        assert!(!sig.matches(b"Different data"));
        assert!(!sig.matches(b"Hello, World! This is a test block")); // One char less
    }

    #[test]
    fn test_signature_builder_bytes() {
        let data = b"The quick brown fox jumps over the lazy dog. ".repeat(100);
        let builder = SignatureBuilder::new().block_size(64);

        let sig = builder.build_from_bytes(&data);

        assert_eq!(sig.file_size, data.len() as u64);
        assert_eq!(sig.block_size, 64);

        // Verify each block signature
        for (i, block) in sig.blocks.iter().enumerate() {
            let start = i * 64;
            let end = (start + 64).min(data.len());
            assert!(block.matches(&data[start..end]));
        }
    }

    #[test]
    fn test_signature_builder_reader() {
        let data = b"Test data for reader-based signature building.".repeat(50);
        let mut cursor = Cursor::new(&data);

        let builder = SignatureBuilder::new().block_size(128);
        let sig = builder.build_from_reader(&mut cursor).unwrap();

        assert_eq!(sig.file_size, data.len() as u64);

        // Verify file hash
        let expected_hash = blake3::hash(&data);
        assert_eq!(sig.file_hash, *expected_hash.as_bytes());
    }

    #[test]
    fn test_signature_lookup() {
        let data = b"AAAA".repeat(100); // Repetitive data
        let builder = SignatureBuilder::new().block_size(4);

        let sig = builder.build_from_bytes(&data);
        let lookup = sig.build_lookup();

        // All blocks have the same content, so they should all have the same weak hash
        let first_weak = sig.blocks[0].weak_hash;
        let matches = lookup.find_weak_matches(first_weak).unwrap();

        // All blocks should match
        assert_eq!(matches.len(), sig.blocks.len());
    }

    #[test]
    fn test_signature_serialization() {
        let data = b"Test data for serialization";
        let sig = SignatureBuilder::new().build_from_bytes(data);

        let bytes = sig.to_bytes();
        let restored = FileSignature::from_bytes(&bytes).unwrap();

        assert_eq!(restored.file_size, sig.file_size);
        assert_eq!(restored.file_hash, sig.file_hash);
        assert_eq!(restored.blocks.len(), sig.blocks.len());
    }
}
