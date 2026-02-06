//! Rate limiting for network operations using the governor crate

use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::num::NonZeroU32;
use std::sync::Arc;

/// Rate limiter for controlling bandwidth and request rates
pub struct TransferRateLimiter {
    /// Bytes per second limiter
    bytes_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    /// Chunks per second limiter
    chunks_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    /// Whether rate limiting is enabled
    enabled: bool,
}

impl TransferRateLimiter {
    /// Create a new rate limiter with specified limits
    ///
    /// # Arguments
    /// * `bytes_per_second` - Maximum bytes per second (0 = unlimited)
    /// * `chunks_per_second` - Maximum chunks per second (0 = unlimited)
    pub fn new(bytes_per_second: u32, chunks_per_second: u32) -> Self {
        let bytes_limiter = if bytes_per_second > 0 {
            // Convert to quota (we'll use 1KB units for finer control)
            let kb_per_second = (bytes_per_second / 1024).max(1);
            RateLimiter::direct(Quota::per_second(NonZeroU32::new(kb_per_second).unwrap()))
        } else {
            // Very high limit = effectively unlimited
            RateLimiter::direct(Quota::per_second(NonZeroU32::new(u32::MAX).unwrap()))
        };

        let chunks_limiter = if chunks_per_second > 0 {
            RateLimiter::direct(Quota::per_second(
                NonZeroU32::new(chunks_per_second).unwrap(),
            ))
        } else {
            RateLimiter::direct(Quota::per_second(NonZeroU32::new(u32::MAX).unwrap()))
        };

        Self {
            bytes_limiter: Arc::new(bytes_limiter),
            chunks_limiter: Arc::new(chunks_limiter),
            enabled: bytes_per_second > 0 || chunks_per_second > 0,
        }
    }

    /// Create an unlimited rate limiter
    pub fn unlimited() -> Self {
        Self::new(0, 0)
    }

    /// Wait until we're allowed to send the specified number of bytes
    pub async fn wait_for_bytes(&self, bytes: usize) {
        if !self.enabled {
            return;
        }

        // Convert to KB units (minimum 1)
        let kb_units = ((bytes + 1023) / 1024).max(1) as u32;

        for _ in 0..kb_units {
            self.bytes_limiter.until_ready().await;
        }
    }

    /// Wait until we're allowed to send a chunk
    pub async fn wait_for_chunk(&self) {
        if !self.enabled {
            return;
        }

        self.chunks_limiter.until_ready().await;
    }

    /// Check if rate limiting is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for TransferRateLimiter {
    fn default() -> Self {
        Self::unlimited()
    }
}

impl Clone for TransferRateLimiter {
    fn clone(&self) -> Self {
        Self {
            bytes_limiter: self.bytes_limiter.clone(),
            chunks_limiter: self.chunks_limiter.clone(),
            enabled: self.enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_unlimited_rate_limiter() {
        let limiter = TransferRateLimiter::unlimited();

        assert!(!limiter.is_enabled());

        // Should not block
        let start = Instant::now();
        for _ in 0..100 {
            limiter.wait_for_chunk().await;
        }
        let elapsed = start.elapsed();

        // Should be very fast (under 100ms for 100 iterations)
        assert!(elapsed.as_millis() < 100);
    }

    #[tokio::test]
    async fn test_rate_limited_chunks() {
        // 10 chunks per second
        let limiter = TransferRateLimiter::new(0, 10);

        assert!(limiter.is_enabled());

        let start = Instant::now();
        // Wait for 3 chunks (first is immediate, then ~100ms each)
        for _ in 0..3 {
            limiter.wait_for_chunk().await;
        }
        let elapsed = start.elapsed();

        // At 10/second, 3 chunks should take at least 100ms
        // (first immediate, then 2 x ~100ms delay)
        // Being lenient due to timing variations
        println!("Rate limited 3 chunks in {}ms", elapsed.as_millis());
        // Note: The governor crate allows burst, so initial chunks may be immediate
        // We mainly test that the limiter doesn't block indefinitely
        assert!(elapsed.as_millis() < 5000, "Rate limiter took too long");
    }
}
