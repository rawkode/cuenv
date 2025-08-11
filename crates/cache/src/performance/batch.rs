//! Batch processing for improved throughput

use std::mem;

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
            let batch = mem::replace(&mut self.batch, Vec::with_capacity(self.batch_size));
            (self.processor)(batch);
        }
    }

    /// Get the current batch size
    pub fn current_size(&self) -> usize {
        self.batch.len()
    }

    /// Check if the batch is empty
    pub fn is_empty(&self) -> bool {
        self.batch.is_empty()
    }

    /// Check if the batch is full
    pub fn is_full(&self) -> bool {
        self.batch.len() >= self.batch_size
    }

    /// Get the configured batch size
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// Process items without batching (immediately)
    pub fn process_immediate(&self, items: Vec<T>) {
        if !items.is_empty() {
            (self.processor)(items);
        }
    }
}

impl<T: Send> Drop for BatchProcessor<T> {
    fn drop(&mut self) {
        self.flush();
    }
}

/// Builder for BatchProcessor with fluent API
pub struct BatchProcessorBuilder<T: Send> {
    batch_size: usize,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Send> BatchProcessorBuilder<T> {
    pub fn new() -> Self {
        Self {
            batch_size: 100, // Default batch size
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    pub fn build(self, processor: impl Fn(Vec<T>) + Send + Sync + 'static) -> BatchProcessor<T> {
        BatchProcessor::new(self.batch_size, processor)
    }
}

impl<T: Send> Default for BatchProcessorBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_batch_processing() {
        let processed = Arc::new(Mutex::new(Vec::new()));
        let processed_clone = processed.clone();

        let mut processor = BatchProcessor::new(3, move |batch| {
            processed_clone.lock().unwrap().extend(batch);
        });

        assert!(processor.is_empty());
        assert!(!processor.is_full());

        processor.add(1);
        processor.add(2);
        assert_eq!(processor.current_size(), 2);
        assert!(processed.lock().unwrap().is_empty()); // Not flushed yet

        processor.add(3); // This triggers flush
        assert!(processor.is_empty());
        assert_eq!(*processed.lock().unwrap(), vec![1, 2, 3]);

        processor.add(4);
        drop(processor); // Drop triggers final flush
        assert_eq!(*processed.lock().unwrap(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_builder() {
        let processed = Arc::new(Mutex::new(Vec::new()));
        let processed_clone = processed.clone();

        let mut processor = BatchProcessorBuilder::new()
            .batch_size(2)
            .build(move |batch| {
                processed_clone.lock().unwrap().extend(batch);
            });

        processor.add(1);
        processor.add(2); // Should trigger flush at size 2
        assert_eq!(*processed.lock().unwrap(), vec![1, 2]);
    }

    #[test]
    fn test_immediate_processing() {
        let processed = Arc::new(Mutex::new(Vec::new()));
        let processed_clone = processed.clone();

        let processor = BatchProcessor::new(10, move |batch| {
            processed_clone.lock().unwrap().extend(batch);
        });

        processor.process_immediate(vec![1, 2, 3]);
        assert_eq!(*processed.lock().unwrap(), vec![1, 2, 3]);
    }
}