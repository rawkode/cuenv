//! Memory pool for reducing allocation overhead

use super::alignment::CACHE_LINE_SIZE;
use parking_lot::Mutex;
use std::alloc::{alloc, dealloc, Layout};
use std::sync::atomic::{AtomicUsize, Ordering};

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
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `ptr` was previously allocated by this pool's `allocate` method
    /// - `ptr` has not been deallocated already
    /// - No other code is accessing the memory pointed to by `ptr`
    pub unsafe fn deallocate(&self, ptr: *mut u8) {
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

    /// Get the current number of allocated blocks
    pub fn allocated_count(&self) -> usize {
        self.allocated.load(Ordering::Relaxed)
    }

    /// Get the current number of blocks in the pool
    pub fn pooled_count(&self) -> usize {
        self.blocks.lock().len()
    }

    /// Clear all pooled blocks (deallocate them)
    pub fn clear(&self) {
        let layout =
            Layout::from_size_align(self.block_size, CACHE_LINE_SIZE).expect("Invalid layout");

        let mut blocks = self.blocks.lock();
        for ptr in blocks.drain(..) {
            unsafe {
                dealloc(ptr, layout);
            }
            self.allocated.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

impl Drop for MemoryPool {
    fn drop(&mut self) {
        let layout =
            Layout::from_size_align(self.block_size, CACHE_LINE_SIZE).expect("Invalid layout");

        let blocks = self.blocks.lock();
        for ptr in blocks.iter() {
            unsafe {
                dealloc(*ptr, layout);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_allocation() {
        let pool = MemoryPool::new(1024, 10);

        let ptr1 = pool.allocate().expect("Failed to allocate");
        let ptr2 = pool.allocate().expect("Failed to allocate");
        
        assert_eq!(pool.allocated_count(), 2);
        assert_eq!(pool.pooled_count(), 0);

        unsafe {
            pool.deallocate(ptr1);
            pool.deallocate(ptr2);
        }

        assert_eq!(pool.pooled_count(), 2);
    }

    #[test]
    fn test_pool_reuse() {
        let pool = MemoryPool::new(1024, 10);

        let ptr1 = pool.allocate().expect("Failed to allocate");
        unsafe {
            pool.deallocate(ptr1);
        }

        let ptr2 = pool.allocate().expect("Failed to allocate");
        assert_eq!(ptr1, ptr2); // Should reuse the same block
    }
}