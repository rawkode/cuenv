//! Performance optimizations for Google-scale cache operations
//!
//! This module contains specialized implementations for high-performance
//! cache operations including:
//! - SIMD-accelerated hashing
//! - Lock-free data structures
//! - Cache-line aware memory layouts
//! - Prefetching and branch prediction hints

#![allow(dead_code)]

pub mod alignment;
pub mod batch;
pub mod hints;
pub mod memory;
pub mod stats;

#[cfg(target_arch = "x86_64")]
pub mod simd;

// Re-export commonly used items
pub use alignment::{CacheLineAligned, CACHE_LINE_SIZE};
pub use batch::{BatchProcessor, BatchProcessorBuilder};
pub use hints::{fast_path_get, likely, prefetch_read, prefetch_write, unlikely};
pub use memory::MemoryPool;
pub use stats::{PerfStats, StatsSnapshot};

#[cfg(target_arch = "x86_64")]
pub use simd::{aligned_copy, get_simd_features, simd_hash, SimdFeatures};
