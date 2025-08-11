//! CPU hints for optimization - prefetch and branch prediction

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
pub fn fast_path_get<F1, F2, T>(condition: bool, fast: F1, slow: F2) -> T
where
    F1: FnOnce() -> T,
    F2: FnOnce() -> T,
{
    if condition {
        fast()
    } else {
        unlikely();
        slow()
    }
}

/// Likely condition - helps the compiler optimize for the common case
#[inline(always)]
pub fn likely_condition(condition: bool) -> bool {
    if condition {
        likely();
        true
    } else {
        false
    }
}

/// Unlikely condition - helps the compiler optimize for the uncommon case
#[inline(always)]
pub fn unlikely_condition(condition: bool) -> bool {
    if condition {
        false
    } else {
        unlikely();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_path() {
        let result = fast_path_get(true, || 42, || 0);
        assert_eq!(result, 42);

        let result = fast_path_get(false, || 42, || 0);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_likely_condition() {
        assert!(likely_condition(true));
        assert!(!likely_condition(false));
    }

    #[test]
    fn test_unlikely_condition() {
        assert!(!unlikely_condition(true));
        assert!(unlikely_condition(false));
    }

    #[test]
    fn test_prefetch_no_crash() {
        let value = 42u64;
        let ptr = &value as *const u64;
        
        // These should not crash
        prefetch_read(ptr);
        prefetch_write(ptr);
    }
}