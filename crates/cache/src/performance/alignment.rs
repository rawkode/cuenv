//! Cache-line alignment utilities for preventing false sharing

use std::sync::atomic::AtomicU64;

/// Cache line size for x86_64 processors (64 bytes)
#[cfg(target_arch = "x86_64")]
pub const CACHE_LINE_SIZE: usize = 64;

/// Cache line size for ARM processors (typically 128 bytes)
#[cfg(target_arch = "aarch64")]
pub const CACHE_LINE_SIZE: usize = 128;

/// Default cache line size for other architectures
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub const CACHE_LINE_SIZE: usize = 64;

/// Align a type to cache line boundaries to prevent false sharing
#[cfg(target_arch = "x86_64")]
#[repr(align(64))]
pub struct CacheLineAligned<T>(pub T);

/// Align a type to cache line boundaries to prevent false sharing
#[cfg(target_arch = "aarch64")]
#[repr(align(128))]
pub struct CacheLineAligned<T>(pub T);

/// Align a type to cache line boundaries to prevent false sharing
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
#[repr(align(64))]
pub struct CacheLineAligned<T>(pub T);

impl<T> CacheLineAligned<T> {
    /// Create a new cache-line aligned value
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Get a reference to the inner value
    pub fn get(&self) -> &T {
        &self.0
    }

    /// Get a mutable reference to the inner value
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl CacheLineAligned<AtomicU64> {
    /// Create a new cache-line aligned atomic counter
    pub const fn new_atomic(value: u64) -> Self {
        Self(AtomicU64::new(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment() {
        let aligned = CacheLineAligned::new(42u64);
        let addr = &aligned as *const _ as usize;
        assert_eq!(addr % CACHE_LINE_SIZE, 0);
    }

    #[test]
    fn test_atomic_alignment() {
        let aligned = CacheLineAligned::new_atomic(0);
        let addr = &aligned as *const _ as usize;
        assert_eq!(addr % CACHE_LINE_SIZE, 0);
    }
}
