//! Batch get operations for improved throughput

/// Batch get operations for improved throughput
pub struct BatchGet<'a> {
    keys: Vec<&'a str>,
    results: Vec<Option<Vec<u8>>>,
}

impl<'a> BatchGet<'a> {
    pub fn new(capacity: usize) -> Self {
        Self {
            keys: Vec::with_capacity(capacity),
            results: Vec::with_capacity(capacity),
        }
    }

    #[inline(always)]
    pub fn add_key(&mut self, key: &'a str) {
        self.keys.push(key);
    }

    pub async fn execute<F>(&mut self, getter: F) -> Vec<Option<Vec<u8>>>
    where
        F: Fn(&str) -> Option<Vec<u8>>,
    {
        // Prefetch all keys
        for key in &self.keys {
            self.results.push(getter(key));
        }

        std::mem::take(&mut self.results)
    }
}
