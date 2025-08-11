//! SIMD-accelerated operations for x86_64 architecture

use std::arch::x86_64::*;

/// SIMD-accelerated hash function for cache keys
pub mod simd_hash {
    use std::arch::x86_64::*;

    /// Fast hash using SSE4.2 CRC32 instructions
    ///
    /// # Safety
    ///
    /// This function is unsafe because it uses SSE4.2 intrinsics which require
    /// the target CPU to support these instructions. The caller must ensure that
    /// the CPU supports SSE4.2 before calling this function.
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

/// Optimized memory copy for cache-aligned data
///
/// # Safety
///
/// This function is unsafe because it directly manipulates raw pointers.
/// The caller must ensure that:
/// - Both `dst` and `src` are valid pointers
/// - `dst` points to at least `len` bytes of writable memory
/// - `src` points to at least `len` bytes of readable memory
/// - The memory regions do not overlap
pub unsafe fn aligned_copy(dst: *mut u8, src: *const u8, len: usize) {
    if is_x86_feature_detected!("avx2") {
        aligned_copy_avx2(dst, src, len);
    } else if is_x86_feature_detected!("sse2") {
        aligned_copy_sse2(dst, src, len);
    } else {
        // Fallback to standard copy
        std::ptr::copy_nonoverlapping(src, dst, len);
    }
}

/// AVX2 optimized copy
#[target_feature(enable = "avx2")]
unsafe fn aligned_copy_avx2(dst: *mut u8, src: *const u8, len: usize) {
    // AVX2 path - copy 32 bytes at a time
    let chunks = len / 32;
    let remainder = len % 32;

    for i in 0..chunks {
        let offset = i * 32;
        let src_ptr = src.add(offset) as *const __m256i;
        let dst_ptr = dst.add(offset) as *mut __m256i;
        let data = _mm256_loadu_si256(src_ptr);
        _mm256_storeu_si256(dst_ptr, data);
    }

    // Copy remainder
    if remainder > 0 {
        std::ptr::copy_nonoverlapping(src.add(chunks * 32), dst.add(chunks * 32), remainder);
    }
}

/// SSE2 optimized copy
#[target_feature(enable = "sse2")]
unsafe fn aligned_copy_sse2(dst: *mut u8, src: *const u8, len: usize) {
    // SSE2 path - copy 16 bytes at a time
    let chunks = len / 16;
    let remainder = len % 16;

    for i in 0..chunks {
        let offset = i * 16;
        let src_ptr = src.add(offset) as *const __m128i;
        let dst_ptr = dst.add(offset) as *mut __m128i;
        let data = _mm_loadu_si128(src_ptr);
        _mm_storeu_si128(dst_ptr, data);
    }

    // Copy remainder
    if remainder > 0 {
        std::ptr::copy_nonoverlapping(src.add(chunks * 16), dst.add(chunks * 16), remainder);
    }
}

/// Check available SIMD features
pub fn get_simd_features() -> SimdFeatures {
    SimdFeatures {
        sse2: is_x86_feature_detected!("sse2"),
        sse42: is_x86_feature_detected!("sse4.2"),
        avx: is_x86_feature_detected!("avx"),
        avx2: is_x86_feature_detected!("avx2"),
    }
}

/// Available SIMD features on the current CPU
#[derive(Debug, Clone, Copy)]
pub struct SimdFeatures {
    pub sse2: bool,
    pub sse42: bool,
    pub avx: bool,
    pub avx2: bool,
}

impl SimdFeatures {
    /// Check if any SIMD acceleration is available
    pub fn has_any(&self) -> bool {
        self.sse2 || self.sse42 || self.avx || self.avx2
    }

    /// Get the best available copy strategy
    pub fn best_copy_strategy(&self) -> &'static str {
        if self.avx2 {
            "AVX2"
        } else if self.sse2 {
            "SSE2"
        } else {
            "Standard"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_features() {
        let features = get_simd_features();
        // At least SSE2 should be available on x86_64
        assert!(features.sse2);
    }

    #[test]
    fn test_aligned_copy() {
        let src = vec![1u8; 128];
        let mut dst = vec![0u8; 128];

        unsafe {
            aligned_copy(dst.as_mut_ptr(), src.as_ptr(), 128);
        }

        assert_eq!(src, dst);
    }

    #[test]
    fn test_simd_hash_availability() {
        let available = simd_hash::is_simd_available();
        if available {
            let key = b"test_key";
            let hash = unsafe { simd_hash::hash_key_simd(key) };
            assert_ne!(hash, 0);
        }
    }
}