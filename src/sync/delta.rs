//! Delta computation and application for efficient file transfer
//!
//! Computes the difference between two file versions and generates
//! a compact patch that can reconstruct the new file from the old.

use crate::sync::rolling_hash::Adler32Rolling;
use crate::sync::signature::{FileSignature, SignatureBuilder, SignatureLookup};
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom, Write};

/// Instructions for reconstructing a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeltaInstruction {
    /// Copy data from the source file at given offset and length
    Copy { offset: u64, length: u32 },
    /// Insert new literal data
    Insert { data: Vec<u8> },
}

impl DeltaInstruction {
    /// Get the size of data this instruction produces
    pub fn output_size(&self) -> usize {
        match self {
            DeltaInstruction::Copy { length, .. } => *length as usize,
            DeltaInstruction::Insert { data } => data.len(),
        }
    }
}

/// A delta patch containing instructions to transform source to target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaPatch {
    /// Target file size
    pub target_size: u64,
    /// Target file hash (Blake3)
    pub target_hash: [u8; 32],
    /// Block size used for matching
    pub block_size: u32,
    /// Delta instructions
    pub instructions: Vec<DeltaInstruction>,
}

impl DeltaPatch {
    /// Calculate the size of the patch itself (for efficiency metrics)
    pub fn patch_size(&self) -> usize {
        let mut size = 44; // header fields

        for instr in &self.instructions {
            size += 1; // discriminant
            match instr {
                DeltaInstruction::Copy { .. } => size += 12, // offset + length
                DeltaInstruction::Insert { data } => size += 4 + data.len(), // len prefix + data
            }
        }

        size
    }

    /// Calculate compression ratio (patch_size / target_size)
    pub fn compression_ratio(&self) -> f64 {
        if self.target_size == 0 {
            return 1.0;
        }
        self.patch_size() as f64 / self.target_size as f64
    }

    /// Count how many bytes are copied vs inserted
    pub fn stats(&self) -> DeltaStats {
        let mut copied = 0u64;
        let mut inserted = 0u64;
        let mut copy_count = 0usize;
        let mut insert_count = 0usize;

        for instr in &self.instructions {
            match instr {
                DeltaInstruction::Copy { length, .. } => {
                    copied += *length as u64;
                    copy_count += 1;
                }
                DeltaInstruction::Insert { data } => {
                    inserted += data.len() as u64;
                    insert_count += 1;
                }
            }
        }

        DeltaStats {
            copied_bytes: copied,
            inserted_bytes: inserted,
            copy_operations: copy_count,
            insert_operations: insert_count,
            patch_size: self.patch_size(),
            target_size: self.target_size,
        }
    }

    /// Serialize the patch to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize delta")
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }

    /// Apply the patch to source data to produce target data
    pub fn apply(&self, source: &[u8]) -> Result<Vec<u8>, DeltaError> {
        let mut result = Vec::with_capacity(self.target_size as usize);

        for instr in &self.instructions {
            match instr {
                DeltaInstruction::Copy { offset, length } => {
                    let start = *offset as usize;
                    let end = start + *length as usize;

                    if end > source.len() {
                        return Err(DeltaError::SourceTooSmall {
                            required: end,
                            available: source.len(),
                        });
                    }

                    result.extend_from_slice(&source[start..end]);
                }
                DeltaInstruction::Insert { data } => {
                    result.extend_from_slice(data);
                }
            }
        }

        // Verify result
        if result.len() != self.target_size as usize {
            return Err(DeltaError::SizeMismatch {
                expected: self.target_size as usize,
                actual: result.len(),
            });
        }

        let hash = blake3::hash(&result);
        if hash.as_bytes() != &self.target_hash {
            return Err(DeltaError::HashMismatch);
        }

        Ok(result)
    }

    /// Apply patch using readers/writers for large files
    pub fn apply_streaming<R: Read + Seek, W: Write>(
        &self,
        source: &mut R,
        target: &mut W,
    ) -> Result<(), DeltaError> {
        let mut hasher = blake3::Hasher::new();
        let mut total_written = 0u64;

        for instr in &self.instructions {
            match instr {
                DeltaInstruction::Copy { offset, length } => {
                    source.seek(SeekFrom::Start(*offset))?;
                    let mut buffer = vec![0u8; *length as usize];
                    source.read_exact(&mut buffer)?;
                    target.write_all(&buffer)?;
                    hasher.update(&buffer);
                    total_written += *length as u64;
                }
                DeltaInstruction::Insert { data } => {
                    target.write_all(data)?;
                    hasher.update(data);
                    total_written += data.len() as u64;
                }
            }
        }

        // Verify
        if total_written != self.target_size {
            return Err(DeltaError::SizeMismatch {
                expected: self.target_size as usize,
                actual: total_written as usize,
            });
        }

        let hash = hasher.finalize();
        if hash.as_bytes() != &self.target_hash {
            return Err(DeltaError::HashMismatch);
        }

        Ok(())
    }
}

/// Statistics about a delta patch
#[derive(Debug, Clone)]
pub struct DeltaStats {
    pub copied_bytes: u64,
    pub inserted_bytes: u64,
    pub copy_operations: usize,
    pub insert_operations: usize,
    pub patch_size: usize,
    pub target_size: u64,
}

impl DeltaStats {
    /// Percentage of target that could be copied from source
    pub fn copy_ratio(&self) -> f64 {
        if self.target_size == 0 {
            return 0.0;
        }
        self.copied_bytes as f64 / self.target_size as f64 * 100.0
    }

    /// Bandwidth savings compared to full transfer
    pub fn savings_ratio(&self) -> f64 {
        if self.target_size == 0 {
            return 0.0;
        }
        (1.0 - self.patch_size as f64 / self.target_size as f64) * 100.0
    }
}

impl std::fmt::Display for DeltaStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Delta: {} copied ({:.1}%), {} inserted, {} total ops, {:.1}% savings",
            self.copied_bytes,
            self.copy_ratio(),
            self.inserted_bytes,
            self.copy_operations + self.insert_operations,
            self.savings_ratio()
        )
    }
}

/// Errors that can occur during delta operations
#[derive(Debug, thiserror::Error)]
pub enum DeltaError {
    #[error("Source file too small: required {required} bytes, available {available}")]
    SourceTooSmall { required: usize, available: usize },

    #[error("Size mismatch: expected {expected}, got {actual}")]
    SizeMismatch { expected: usize, actual: usize },

    #[error("Hash verification failed")]
    HashMismatch,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Builder for computing delta patches
pub struct DeltaBuilder {
    block_size: usize,
}

impl DeltaBuilder {
    /// Create a new delta builder
    pub fn new() -> Self {
        Self { block_size: 4096 }
    }

    /// Set the block size (should match signature block size)
    pub fn block_size(mut self, size: usize) -> Self {
        self.block_size = size;
        self
    }

    /// Compute delta between source signature and target data
    pub fn build(&self, source_sig: &FileSignature, target: &[u8]) -> DeltaPatch {
        let target_hash_full = blake3::hash(target);
        let mut target_hash = [0u8; 32];
        target_hash.copy_from_slice(target_hash_full.as_bytes());

        // Quick check: if files are identical, return single copy
        if source_sig.file_hash == target_hash && source_sig.file_size == target.len() as u64 {
            return DeltaPatch {
                target_size: target.len() as u64,
                target_hash,
                block_size: self.block_size as u32,
                instructions: vec![DeltaInstruction::Copy {
                    offset: 0,
                    length: target.len() as u32,
                }],
            };
        }

        let lookup = source_sig.build_lookup();
        let instructions = self.compute_instructions(&lookup, target);

        // Optimize: merge adjacent copy/insert operations
        let optimized = self.optimize_instructions(instructions);

        DeltaPatch {
            target_size: target.len() as u64,
            target_hash,
            block_size: self.block_size as u32,
            instructions: optimized,
        }
    }

    /// Compute delta given source data and target data
    pub fn build_from_data(&self, source: &[u8], target: &[u8]) -> DeltaPatch {
        let sig = SignatureBuilder::new()
            .block_size(self.block_size)
            .build_from_bytes(source);

        self.build(&sig, target)
    }

    /// Core delta computation algorithm (simplified and correct)
    fn compute_instructions(
        &self,
        lookup: &SignatureLookup,
        target: &[u8],
    ) -> Vec<DeltaInstruction> {
        if target.is_empty() {
            return vec![];
        }

        let block_size = lookup.block_size();
        let mut instructions = Vec::new();
        let mut pending_literal = Vec::new();
        let mut pos = 0;

        while pos < target.len() {
            let remaining = target.len() - pos;

            // Try to find a matching block if we have enough data
            let mut found_match = false;

            if remaining >= block_size {
                let target_block = &target[pos..pos + block_size];
                let weak_hash = Adler32Rolling::checksum(target_block);

                // Look for matching blocks
                if let Some(candidates) = lookup.find_weak_matches(weak_hash) {
                    let target_strong = blake3::hash(target_block);
                    let target_strong_truncated: [u8; 16] =
                        target_strong.as_bytes()[..16].try_into().unwrap();

                    for &block_idx in candidates {
                        if let Some(block) = lookup.get_block(block_idx) {
                            if block.strong_hash == target_strong_truncated
                                && block.length as usize == block_size
                            {
                                // Found a match!

                                // Flush pending literals first
                                if !pending_literal.is_empty() {
                                    instructions.push(DeltaInstruction::Insert {
                                        data: std::mem::take(&mut pending_literal),
                                    });
                                }

                                // Add copy instruction
                                instructions.push(DeltaInstruction::Copy {
                                    offset: block.offset,
                                    length: block.length,
                                });

                                pos += block.length as usize;
                                found_match = true;
                                break;
                            }
                        }
                    }
                }
            }

            // If no match, add byte to pending literals
            if !found_match {
                pending_literal.push(target[pos]);
                pos += 1;
            }
        }

        // Flush remaining literals
        if !pending_literal.is_empty() {
            instructions.push(DeltaInstruction::Insert {
                data: pending_literal,
            });
        }

        instructions
    }

    /// Optimize instructions by merging adjacent operations
    fn optimize_instructions(&self, instructions: Vec<DeltaInstruction>) -> Vec<DeltaInstruction> {
        if instructions.len() <= 1 {
            return instructions;
        }

        let mut optimized = Vec::with_capacity(instructions.len());

        for instr in instructions {
            match (&mut optimized.last_mut(), &instr) {
                // Merge adjacent copies if they're contiguous
                (
                    Some(DeltaInstruction::Copy { offset, length }),
                    DeltaInstruction::Copy {
                        offset: next_offset,
                        length: next_length,
                    },
                ) if *offset + *length as u64 == *next_offset => {
                    *length += next_length;
                }
                // Merge adjacent inserts
                (
                    Some(DeltaInstruction::Insert { data }),
                    DeltaInstruction::Insert { data: next_data },
                ) => {
                    data.extend_from_slice(next_data);
                }
                // Otherwise, add new instruction
                _ => {
                    optimized.push(instr);
                }
            }
        }

        optimized
    }
}

impl Default for DeltaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_files() {
        let data = b"Hello, World! This is test data.";
        let builder = DeltaBuilder::new().block_size(8);

        let sig = SignatureBuilder::new().block_size(8).build_from_bytes(data);
        let patch = builder.build(&sig, data);

        let stats = patch.stats();
        assert_eq!(stats.copy_ratio(), 100.0);
        assert_eq!(stats.inserted_bytes, 0);
    }

    #[test]
    fn test_completely_different_files() {
        let source = b"AAAAAAAAAAAAAAAA";
        let target = b"BBBBBBBBBBBBBBBB";

        let builder = DeltaBuilder::new().block_size(4);
        let patch = builder.build_from_data(source, target);

        let stats = patch.stats();
        assert_eq!(stats.copied_bytes, 0);
        assert_eq!(stats.inserted_bytes, target.len() as u64);

        // Apply and verify
        let result = patch.apply(source).unwrap();
        assert_eq!(result, target);
    }

    #[test]
    fn test_partial_modification() {
        // Source: AAAA BBBB CCCC DDDD (4 blocks of 4 bytes each)
        let source = b"AAAABBBBCCCCDDDD";
        // Target: AAAA XXXX CCCC DDDD (second block changed)
        let target = b"AAAAXXXXCCCCDDDD";

        let builder = DeltaBuilder::new().block_size(4);
        let patch = builder.build_from_data(source, target);

        let stats = patch.stats();
        println!("{}", stats);

        // Should copy 12 bytes (AAAA + CCCC + DDDD) and insert 4 (XXXX)
        assert_eq!(stats.copied_bytes, 12);
        assert_eq!(stats.inserted_bytes, 4);

        // Apply and verify
        let result = patch.apply(source).unwrap();
        assert_eq!(result, target);
    }

    #[test]
    fn test_appended_data() {
        let source = b"AAAABBBBCCCCDDDD";
        let mut target = source.to_vec();
        target.extend_from_slice(b"EEEE");

        let builder = DeltaBuilder::new().block_size(4);
        let patch = builder.build_from_data(source, &target);

        let stats = patch.stats();
        println!("{}", stats);

        // All original blocks should be copied, only EEEE inserted
        assert_eq!(stats.copied_bytes, 16);
        assert_eq!(stats.inserted_bytes, 4);

        // Apply and verify
        let result = patch.apply(source).unwrap();
        assert_eq!(result, target);
    }

    #[test]
    fn test_prepended_data() {
        let source = b"AAAABBBBCCCCDDDD";
        let mut target = b"XXXX".to_vec();
        target.extend_from_slice(source);

        let builder = DeltaBuilder::new().block_size(4);
        let patch = builder.build_from_data(source, &target);

        // Apply and verify
        let result = patch.apply(source).unwrap();
        assert_eq!(result, target);
    }

    #[test]
    fn test_large_file_delta() {
        // Create a large source file with repeated pattern
        let pattern = b"ABCDEFGHIJKLMNOP"; // 16 bytes
        let source: Vec<u8> = pattern.iter().cycle().take(10000).copied().collect();

        // Create target with small modifications
        let mut target = source.clone();
        // Modify some bytes in the middle (at block boundary)
        let modify_start = 5000 - (5000 % 64); // Align to block
        target[modify_start..modify_start + 64].fill(b'X');

        let builder = DeltaBuilder::new().block_size(64);
        let patch = builder.build_from_data(&source, &target);

        let stats = patch.stats();
        println!("Large file delta: {}", stats);

        // Most data should be copied
        assert!(stats.copy_ratio() > 90.0);

        // Apply and verify
        let result = patch.apply(&source).unwrap();
        assert_eq!(result, target);
    }

    #[test]
    fn test_patch_serialization() {
        let source = b"Source file content";
        let target = b"Target file content";

        let builder = DeltaBuilder::new().block_size(8);
        let patch = builder.build_from_data(source, target);

        // Serialize and deserialize
        let bytes = patch.to_bytes();
        let restored = DeltaPatch::from_bytes(&bytes).unwrap();

        // Verify restored patch works
        let result = restored.apply(source).unwrap();
        assert_eq!(result, target);
    }

    #[test]
    fn test_empty_files() {
        let builder = DeltaBuilder::new();

        // Empty source, non-empty target
        let patch = builder.build_from_data(b"", b"Some data");
        let result = patch.apply(b"").unwrap();
        assert_eq!(result, b"Some data");

        // Non-empty source, empty target
        let patch = builder.build_from_data(b"Some data", b"");
        let result = patch.apply(b"Some data").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_hash_verification() {
        let source = b"Source";
        let target = b"Target";

        let builder = DeltaBuilder::new();
        let mut patch = builder.build_from_data(source, target);

        // Corrupt the hash
        patch.target_hash[0] ^= 0xFF;

        // Should fail verification
        let result = patch.apply(source);
        assert!(matches!(result, Err(DeltaError::HashMismatch)));
    }

    #[test]
    fn test_compression_ratio() {
        // Create similar files - source and target differ only slightly
        // Using larger blocks (64 bytes) for more realistic efficiency
        let source: Vec<u8> = (0..100)
            .flat_map(|_| {
                b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz++".to_vec()
            })
            .collect();
        let mut target = source.clone();
        // Modify one block in the middle
        target[3200..3264].fill(b'X');

        let builder = DeltaBuilder::new().block_size(64);
        let patch = builder.build_from_data(&source, &target);

        let stats = patch.stats();
        println!("Compression test: {}", stats);

        // Most data should be copied, minimal inserted
        assert!(
            stats.copy_ratio() > 95.0,
            "Copy ratio should be > 95%, got {:.1}%",
            stats.copy_ratio()
        );

        // Verify patch works correctly
        let result = patch.apply(&source).unwrap();
        assert_eq!(result, target);
    }
}
