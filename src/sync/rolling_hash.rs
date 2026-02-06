//! Rolling hash implementation for delta transfer
//!
//! Uses Adler-32 style rolling checksum for fast block matching,
//! similar to rsync's rolling checksum algorithm.

/// Trait for rolling hash implementations
pub trait RollingHash {
    /// Create a new rolling hash instance
    fn new() -> Self;

    /// Update the hash with initial data (full block)
    fn update(&mut self, data: &[u8]);

    /// Roll the hash: remove old byte, add new byte
    fn roll(&mut self, old_byte: u8, new_byte: u8);

    /// Get the current hash value
    fn digest(&self) -> u32;

    /// Reset the hash state
    fn reset(&mut self);
}

/// Adler-32 style rolling checksum
///
/// This is a fast, weak checksum used for initial block matching.
/// When a potential match is found, a strong hash (Blake3) confirms it.
///
/// The algorithm maintains two sums:
/// - `a`: sum of all bytes + 1
/// - `b`: weighted sum
///
/// Both are computed modulo 65521 (largest prime < 2^16).
#[derive(Debug, Clone)]
pub struct Adler32Rolling {
    a: u32,
    b: u32,
    block_size: usize,
}

const MOD_ADLER: u32 = 65521;

impl Adler32Rolling {
    /// Create with a specific block size
    pub fn with_block_size(block_size: usize) -> Self {
        Self {
            a: 1,
            b: 0,
            block_size,
        }
    }

    /// Get the current block size
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Update with a full block of data
    pub fn update_block(&mut self, data: &[u8]) {
        self.a = 1;
        self.b = 0;
        for &byte in data {
            self.a = (self.a + byte as u32) % MOD_ADLER;
            self.b = (self.b + self.a) % MOD_ADLER;
        }
    }

    /// Roll the window forward by one byte
    ///
    /// Given: window [x1, x2, ..., xn], we're sliding to [x2, x3, ..., xn, new]
    ///
    /// For Adler-32:
    /// - a_old = 1 + x1 + x2 + ... + xn
    /// - a_new = 1 + x2 + x3 + ... + xn + new = a_old - x1 + new
    ///
    /// - b_old = n*x1 + (n-1)*x2 + ... + 1*xn + n (the +n comes from the n 1's in the a values)
    /// - b_new = n*x2 + (n-1)*x3 + ... + 1*new + n
    ///         = (n-1)*x2 + (n-2)*x3 + ... + 0*xn + x2 + x3 + ... + xn + new + n
    ///         = b_old - n*x1 - (x2+x3+...+xn) - (a_old - 1 - x1) + (x2+...+xn+new) + n
    ///
    /// Simplified: b_new = b_old - n*x1 - a_old + 1 + a_new
    ///                   = b_old + a_new - a_old - n*x1 + 1
    ///                   = b_old + (new - x1) - n*x1 + 1
    ///
    /// Actually, the correct formula (from rsync/librsync):
    /// a_new = a_old - x1 + new
    /// b_new = b_old - n*x1 + a_new - 1
    pub fn roll_byte(&mut self, old_byte: u8, new_byte: u8) {
        let old = old_byte as u32;
        let new = new_byte as u32;
        let n = self.block_size as u32;

        // New a: remove old byte, add new byte
        self.a = (self.a + MOD_ADLER - old + new) % MOD_ADLER;

        // New b: the formula is b_new = b_old - n*old + a_new - 1
        // We need to be careful with modular arithmetic
        let subtract = (n * old + 1) % MOD_ADLER;
        self.b = (self.b + MOD_ADLER + self.a - subtract) % MOD_ADLER;
    }

    /// Compute checksum without rolling state (for verification)
    pub fn checksum(data: &[u8]) -> u32 {
        let mut a: u32 = 1;
        let mut b: u32 = 0;

        for &byte in data {
            a = (a + byte as u32) % MOD_ADLER;
            b = (b + a) % MOD_ADLER;
        }

        (b << 16) | a
    }
}

impl Default for Adler32Rolling {
    fn default() -> Self {
        Self::with_block_size(4096)
    }
}

impl RollingHash for Adler32Rolling {
    fn new() -> Self {
        Self::default()
    }

    fn update(&mut self, data: &[u8]) {
        self.update_block(data);
    }

    fn roll(&mut self, old_byte: u8, new_byte: u8) {
        self.roll_byte(old_byte, new_byte);
    }

    fn digest(&self) -> u32 {
        (self.b << 16) | self.a
    }

    fn reset(&mut self) {
        self.a = 1;
        self.b = 0;
    }
}

/// Fast rolling hash using simpler polynomial approach
///
/// This is faster than Adler-32 but less standard.
/// Uses cyclic polynomial (Buzhash) style rolling.
#[derive(Debug, Clone)]
pub struct SimpleRollingHash {
    hash: u32,
    block_size: usize,
    /// Circular buffer to track bytes in window
    window: Vec<u8>,
    /// Current position in window
    pos: usize,
    /// Whether the window is filled
    filled: bool,
}

impl SimpleRollingHash {
    /// Create a new rolling hash with given block size
    pub fn new(block_size: usize) -> Self {
        Self {
            hash: 0,
            block_size,
            window: vec![0; block_size],
            pos: 0,
            filled: false,
        }
    }

    /// Update with a full block
    pub fn update_block(&mut self, data: &[u8]) {
        assert!(data.len() <= self.block_size);
        self.hash = 0;
        self.pos = 0;
        self.filled = data.len() == self.block_size;

        for (i, &byte) in data.iter().enumerate() {
            self.window[i] = byte;
            self.hash = self.hash.wrapping_mul(31).wrapping_add(byte as u32);
        }
        self.pos = data.len() % self.block_size;
    }

    /// Roll the window forward by one byte
    pub fn roll_byte(&mut self, _old_byte: u8, new_byte: u8) {
        // For simple polynomial hash, rolling is:
        // new_hash = (old_hash - old_byte * base^(n-1)) * base + new_byte
        // We use the window to track the old byte
        let old = self.window[self.pos] as u32;
        let base_power = 31u32.wrapping_pow(self.block_size as u32 - 1);

        self.hash = self
            .hash
            .wrapping_sub(old.wrapping_mul(base_power))
            .wrapping_mul(31)
            .wrapping_add(new_byte as u32);

        self.window[self.pos] = new_byte;
        self.pos = (self.pos + 1) % self.block_size;
        self.filled = true;
    }

    /// Get current hash value
    pub fn digest(&self) -> u32 {
        self.hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adler32_basic() {
        let data = b"Hello, World!";
        let checksum = Adler32Rolling::checksum(data);

        let mut rolling = Adler32Rolling::with_block_size(data.len());
        rolling.update_block(data);

        assert_eq!(rolling.digest(), checksum);
    }

    #[test]
    fn test_adler32_rolling() {
        let data = b"ABCDEFGHIJ";
        let block_size = 4;

        // Compute checksums for all windows using full computation
        let mut expected_checksums = Vec::new();
        for i in 0..=data.len() - block_size {
            expected_checksums.push(Adler32Rolling::checksum(&data[i..i + block_size]));
        }

        // Verify rolling computation matches
        let mut rolling = Adler32Rolling::with_block_size(block_size);
        rolling.update_block(&data[0..block_size]);
        assert_eq!(
            rolling.digest(),
            expected_checksums[0],
            "Initial block mismatch"
        );

        for i in 1..expected_checksums.len() {
            let old_byte = data[i - 1];
            let new_byte = data[i + block_size - 1];
            rolling.roll_byte(old_byte, new_byte);

            assert_eq!(
                rolling.digest(),
                expected_checksums[i],
                "Mismatch at position {}: got {}, expected {}",
                i,
                rolling.digest(),
                expected_checksums[i]
            );
        }
    }

    #[test]
    fn test_adler32_different_block_sizes() {
        let data = b"The quick brown fox jumps over the lazy dog";

        for block_size in [4, 8, 16] {
            let mut rolling = Adler32Rolling::with_block_size(block_size);

            for i in 0..=data.len() - block_size {
                if i == 0 {
                    rolling.update_block(&data[0..block_size]);
                } else {
                    rolling.roll_byte(data[i - 1], data[i + block_size - 1]);
                }

                let expected = Adler32Rolling::checksum(&data[i..i + block_size]);
                assert_eq!(
                    rolling.digest(),
                    expected,
                    "Block size {}, position {}",
                    block_size,
                    i
                );
            }
        }
    }

    #[test]
    fn test_simple_rolling_hash() {
        let data = b"ABCDEFGHIJKLMNOP";
        let block_size = 4;

        // Compute hashes for all windows by full update
        let mut full_hashes = Vec::new();
        let mut hash = SimpleRollingHash::new(block_size);

        for i in 0..=data.len() - block_size {
            hash.update_block(&data[i..i + block_size]);
            full_hashes.push(hash.digest());
        }

        // Verify rolling gives same results
        let mut rolling = SimpleRollingHash::new(block_size);
        rolling.update_block(&data[0..block_size]);
        assert_eq!(rolling.digest(), full_hashes[0]);

        for i in 1..full_hashes.len() {
            rolling.roll_byte(data[i - 1], data[i + block_size - 1]);
            assert_eq!(
                rolling.digest(),
                full_hashes[i],
                "Mismatch at position {}",
                i
            );
        }
    }

    #[test]
    fn test_rolling_hash_trait() {
        fn test_hasher<H: RollingHash>() {
            let mut hasher = H::new();
            hasher.update(b"test");
            let d1 = hasher.digest();
            hasher.reset();
            hasher.update(b"test");
            assert_eq!(hasher.digest(), d1);
        }

        test_hasher::<Adler32Rolling>();
    }
}
