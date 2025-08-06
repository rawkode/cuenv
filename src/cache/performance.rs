//! Performance optimizations for Google-scale cache operations
//!
//! This module contains specialized implementations for high-performance
//! cache operations including:
//! - SIMD-accelerated hashing
//! - Lock-free data structures
//! - Cache-line aware memory layouts
//! - Prefetching and branch prediction hints

#![allow(dead_code)]

use parking_lot::Mutex;
use std::alloc::{alloc, dealloc, Layout};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Cache line size for x86_64 processors (64 bytes)
#[cfg(target_arch = "x86_64")]
const CACHE_LINE_SIZE: usize = 64;

/// Cache line size for ARM processors (typically 128 bytes)
#[cfg(target_arch = "aarch64")]
const CACHE_LINE_SIZE: usize = 128;

/// Default cache line size for other architectures
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
const CACHE_LINE_SIZE: usize = 64;

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

/// High-performance statistics counter with cache-line padding
#[repr(C)]
pub struct PerfStats {
    // Each counter gets its own cache line to prevent false sharing
    pub hits: CacheLineAligned<AtomicU64>,
    pub misses: CacheLineAligned<AtomicU64>,
    pub writes: CacheLineAligned<AtomicU64>,
    pub removals: CacheLineAligned<AtomicU64>,
    pub errors: CacheLineAligned<AtomicU64>,
    pub bytes_read: CacheLineAligned<AtomicU64>,
    pub bytes_written: CacheLineAligned<AtomicU64>,
    pub io_operations: CacheLineAligned<AtomicU64>,
}

impl PerfStats {
    pub const fn new() -> Self {
        Self {
            hits: CacheLineAligned(AtomicU64::new(0)),
            misses: CacheLineAligned(AtomicU64::new(0)),
            writes: CacheLineAligned(AtomicU64::new(0)),
            removals: CacheLineAligned(AtomicU64::new(0)),
            errors: CacheLineAligned(AtomicU64::new(0)),
            bytes_read: CacheLineAligned(AtomicU64::new(0)),
            bytes_written: CacheLineAligned(AtomicU64::new(0)),
            io_operations: CacheLineAligned(AtomicU64::new(0)),
        }
    }

    #[inline(always)]
    pub fn record_hit(&self) {
        self.hits.0.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_miss(&self) {
        self.misses.0.fetch_add(1, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_write(&self, bytes: u64) {
        self.writes.0.fetch_add(1, Ordering::Relaxed);
        self.bytes_written.0.fetch_add(bytes, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_read(&self, bytes: u64) {
        self.bytes_read.0.fetch_add(bytes, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn record_io_op(&self) {
        self.io_operations.0.fetch_add(1, Ordering::Relaxed);
    }
}

/// SIMD-accelerated hash function for cache keys
#[cfg(target_arch = "x86_64")]
pub mod simd_hash {
    use std::arch::x86_64::*;

    /// Fast hash using SSE4.2 CRC32 instructions
    #[target_feature(enable = "sse4.2")]
    pub unsafe fn hash_key_simd(key: &[u8]) -> u64 {
        let mut hash = 0u64;
        let chunks = key.chunks_exact(8);
        let remainder = chunks.remainder();

        // Process 8-byte chunks using CRC32
        for chunk in chunks {
            let value = u64::from_le_bytes(chunk.try_into().unwrap());
            hash = _mm_crc32_u64(hash, value);
        }

        // Process remaining bytes
        for &byte in remainder {
            hash = _mm_crc32_u64(hash, byte as u64);
        }

        hash
    }

    /// Check if SIMD is available
    pub fn is_simd_available() -> bool {
        is_x86_feature_detected!("sse4.2")
    }
}

/// Memory pool for reducing allocation overhead
pub struct MemoryPool {
    /// Pre-allocated memory blocks
    blocks: Mutex<Vec<*mut u8>>,
    /// Size of each block
    block_size: usize,
    /// Maximum number of blocks to keep in pool
    max_blocks: usize,
    /// Current number of allocated blocks
    allocated: AtomicUsize,
}

unsafe impl Send for MemoryPool {}
unsafe impl Sync for MemoryPool {}

impl MemoryPool {
    pub const fn new(block_size: usize, max_blocks: usize) -> Self {
        Self {
            blocks: Mutex::new(Vec::new()),
            block_size,
            max_blocks,
            allocated: AtomicUsize::new(0),
        }
    }

    /// Allocate a block from the pool
    pub fn allocate(&self) -> Option<*mut u8> {
        // Try to get from pool first
        if let Some(mut blocks) = self.blocks.try_lock() {
            if let Some(block) = blocks.pop() {
                return Some(block);
            }
        }

        // Allocate new block if under limit
        let current = self.allocated.load(Ordering::Relaxed);
        if current < self.max_blocks {
            let layout = Layout::from_size_align(self.block_size, CACHE_LINE_SIZE).ok()?;
            let ptr = unsafe { alloc(layout) };
            if !ptr.is_null() {
                self.allocated.fetch_add(1, Ordering::Relaxed);
                return Some(ptr);
            }
        }

        None
    }

    /// Return a block to the pool
    pub fn deallocate(&self, ptr: *mut u8) {
        if let Some(mut blocks) = self.blocks.try_lock() {
            if blocks.len() < self.max_blocks {
                blocks.push(ptr);
                return;
            }
        }

        // Pool is full, deallocate
        let layout =
            Layout::from_size_align(self.block_size, CACHE_LINE_SIZE).expect("Invalid layout");
        unsafe {
            dealloc(ptr, layout);
        }
        self.allocated.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Drop for MemoryPool {
    fn drop(&mut self) {
        let layout =
            Layout::from_size_align(self.block_size, CACHE_LINE_SIZE).expect("Invalid layout");

        let blocks = self.blocks.lock();
        if true {
            for ptr in blocks.iter() {
                unsafe {
                    dealloc(*ptr, layout);
                }
            }
        }
    }
}

/// Prefetch hints for the CPU
#[inline(always)]
pub fn prefetch_read<T>(_ptr: *const T) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use std::arch::x86_64::_mm_prefetch;
        _mm_prefetch(_ptr as *const i8, 0); // _MM_HINT_T0
    }

    // ARM64 prefetch is unstable in Rust 1.88.0
    // TODO: Re-enable when stabilized
    #[cfg(target_arch = "aarch64")]
    {
        // No-op for now
    }
}

#[inline(always)]
pub fn prefetch_write<T>(_ptr: *const T) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use std::arch::x86_64::_mm_prefetch;
        _mm_prefetch(_ptr as *const i8, 1); // _MM_HINT_T1
    }

    // ARM64 prefetch is unstable in Rust 1.88.0
    // TODO: Re-enable when stabilized
    #[cfg(target_arch = "aarch64")]
    {
        // No-op for now
    }
}

/// Branch prediction hints
#[inline(always)]
#[cold]
pub fn unlikely() {
    // This function is marked cold to hint that it's unlikely to be called
}

#[inline(always)]
// #[hot] - custom attribute not available
pub fn likely() {
    // This function is marked hot to hint that it's likely to be called
}

/// Fast path for common operations
#[inline(always)]
pub fn fast_path_get<F, T>(condition: bool, fast: F, slow: F) -> T
where
    F: FnOnce() -> T,
{
    if condition {
        fast()
    } else {
        unlikely();
        slow()
    }
}

/// Optimized memory copy for cache-aligned data
#[cfg(target_arch = "x86_64")]
pub unsafe fn aligned_copy(dst: *mut u8, src: *const u8, len: usize) {
    use std::arch::x86_64::*;

    if is_x86_feature_detected!("avx2") {
        // AVX2 path - copy 32 bytes at a time
        let chunks = len / 32;
        let remainder = len % 32;

        for i in 0..chunks {
            let offset = i * 32;
            let src_ptr = src.add(offset) as *const __m256i;
            let dst_ptr = dst.add(offset) as *mut __m256i;
            let data = _mm256_load_si256(src_ptr);
            _mm256_store_si256(dst_ptr, data);
        }

        // Copy remainder
        if remainder > 0 {
            std::ptr::copy_nonoverlapping(src.add(chunks * 32), dst.add(chunks * 32), remainder);
        }
    } else {
        // Fallback to standard copy
        std::ptr::copy_nonoverlapping(src, dst, len);
    }
}

/// Batch operations for improved throughput
pub struct BatchProcessor<T: Send> {
    batch: Vec<T>,
    batch_size: usize,
    processor: Box<dyn Fn(Vec<T>) + Send + Sync>,
}

impl<T: Send> BatchProcessor<T> {
    pub fn new(batch_size: usize, processor: impl Fn(Vec<T>) + Send + Sync + 'static) -> Self {
        Self {
            batch: Vec::with_capacity(batch_size),
            batch_size,
            processor: Box::new(processor),
        }
    }

    #[inline]
    pub fn add(&mut self, item: T) {
        self.batch.push(item);
        if self.batch.len() >= self.batch_size {
            self.flush();
        }
    }

    pub fn flush(&mut self) {
        if !self.batch.is_empty() {
            let batch = std::mem::replace(&mut self.batch, Vec::with_capacity(self.batch_size));
            (self.processor)(batch);
        }
    }
}

impl<T: Send> Drop for BatchProcessor<T> {
    fn drop(&mut self) {
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_line_alignment() {
        let stats = PerfStats::new();

        // Verify alignment
        let hits_addr = &stats.hits as *const _ as usize;
        let misses_addr = &stats.misses as *const _ as usize;

        assert_eq!(hits_addr % CACHE_LINE_SIZE, 0);
        assert_eq!(misses_addr % CACHE_LINE_SIZE, 0);
        assert!(misses_addr - hits_addr >= CACHE_LINE_SIZE);
    }

    #[test]
    fn test_memory_pool() {
        let pool = MemoryPool::new(1024, 10);

        // Allocate and deallocate
        let ptr1 = pool.allocate().expect("Failed to allocate");
        let ptr2 = pool.allocate().expect("Failed to allocate");

        pool.deallocate(ptr1);
        pool.deallocate(ptr2);

        // Should reuse from pool
        let ptr3 = pool.allocate().expect("Failed to allocate");
        assert_eq!(ptr3, ptr2); // Should get the last deallocated block
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_simd_hash() {
        if simd_hash::is_simd_available() {
            let key = b"test_cache_key";
            let hash = unsafe { simd_hash::hash_key_simd(key) };
            assert_ne!(hash, 0);

            // Same key should produce same hash
            let hash2 = unsafe { simd_hash::hash_key_simd(key) };
            assert_eq!(hash, hash2);
        }
    }

    #[test]
    fn test_batch_processor() {
        use std::sync::{Arc, Mutex};

        let processed = Arc::new(Mutex::new(Vec::new()));
        let processed_clone = processed.clone();

        let mut processor = BatchProcessor::new(3, move |batch| {
            processed_clone.lock().unwrap().extend(batch);
        });

        processor.add(1);
        processor.add(2);
        assert!(processed.lock().unwrap().is_empty()); // Not flushed yet

        processor.add(3); // This triggers flush
        assert_eq!(*processed.lock().unwrap(), vec![1, 2, 3]);

        processor.add(4);
        drop(processor); // Drop triggers final flush
        assert_eq!(*processed.lock().unwrap(), vec![1, 2, 3, 4]);
    }
}
