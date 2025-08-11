//! Inline cache for extremely hot values

/// Inline cache for extremely hot values
pub struct InlineCache<const N: usize> {
    entries: [(Option<String>, Option<Vec<u8>>); N],
    index: usize,
}

impl<const N: usize> Default for InlineCache<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> InlineCache<N> {
    pub const fn new() -> Self {
        const EMPTY: (Option<String>, Option<Vec<u8>>) = (None, None);
        Self {
            entries: [EMPTY; N],
            index: 0,
        }
    }

    #[inline(always)]
    pub fn get(&self, key: &str) -> Option<&[u8]> {
        for (k, v) in &self.entries {
            if let (Some(k), Some(v)) = (k, v) {
                if k == key {
                    return Some(v);
                }
            }
        }
        None
    }

    #[inline(always)]
    pub fn put(&mut self, key: String, value: Vec<u8>) {
        self.entries[self.index] = (Some(key), Some(value));
        self.index = (self.index + 1) % N;
    }
}
