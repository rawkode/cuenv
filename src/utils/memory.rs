use crate::core::types::SharedString;
use std::borrow::Cow;
use std::sync::Arc;

/// Memory-efficient string handling utilities
pub struct StringPool {
    /// Pool of interned strings for deduplication
    interned: std::collections::HashMap<String, SharedString>,
}

impl StringPool {
    /// Create a new string pool
    #[must_use]
    pub fn new() -> Self {
        Self {
            interned: std::collections::HashMap::new(),
        }
    }

    /// Get or create an interned string
    pub fn intern(&mut self, s: &str) -> SharedString {
        if let Some(existing) = self.interned.get(s) {
            existing.clone()
        } else {
            let arc: SharedString = Arc::from(s);
            self.interned.insert(s.to_string(), arc.clone());
            arc
        }
    }

    /// Get the number of interned strings
    #[must_use]
    pub fn len(&self) -> usize {
        self.interned.len()
    }

    /// Check if the pool is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.interned.is_empty()
    }
}

impl Default for StringPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for efficient string operations
pub trait EfficientString {
    /// Convert to a Cow string, avoiding allocation when possible
    fn to_cow(&self) -> Cow<'_, str>;

    /// Convert to a shared string (Arc<str>)
    fn to_shared(&self) -> SharedString;
}

impl EfficientString for str {
    fn to_cow(&self) -> Cow<'_, str> {
        Cow::Borrowed(self)
    }

    fn to_shared(&self) -> SharedString {
        Arc::from(self)
    }
}

impl EfficientString for String {
    fn to_cow(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.as_str())
    }

    fn to_shared(&self) -> SharedString {
        Arc::from(self.as_str())
    }
}

impl EfficientString for Cow<'_, str> {
    fn to_cow(&self) -> Cow<'_, str> {
        self.clone()
    }

    fn to_shared(&self) -> SharedString {
        Arc::from(self.as_ref())
    }
}

/// Memory-efficient environment variable storage
pub struct EfficientEnvVar<'a> {
    pub key: Cow<'a, str>,
    pub value: Cow<'a, str>,
}

impl<'a> EfficientEnvVar<'a> {
    /// Create a new environment variable with borrowed data
    #[must_use]
    pub fn borrowed(key: &'a str, value: &'a str) -> Self {
        Self {
            key: Cow::Borrowed(key),
            value: Cow::Borrowed(value),
        }
    }

    /// Create a new environment variable with owned data
    #[must_use]
    pub fn owned(key: String, value: String) -> Self {
        Self {
            key: Cow::Owned(key),
            value: Cow::Owned(value),
        }
    }

    /// Create from mixed ownership
    #[must_use]
    pub fn new(key: impl Into<Cow<'a, str>>, value: impl Into<Cow<'a, str>>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }

    /// Convert to owned strings
    #[must_use]
    pub fn into_owned(self) -> (String, String) {
        (self.key.into_owned(), self.value.into_owned())
    }

    /// Check if both key and value are borrowed
    #[must_use]
    pub fn is_borrowed(&self) -> bool {
        matches!(&self.key, Cow::Borrowed(_)) && matches!(&self.value, Cow::Borrowed(_))
    }
}

/// Efficient string builder that minimizes allocations
pub struct EfficientStringBuilder<'a> {
    parts: Vec<Cow<'a, str>>,
    estimated_capacity: usize,
}

impl<'a> EfficientStringBuilder<'a> {
    /// Create a new string builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            parts: Vec::new(),
            estimated_capacity: 0,
        }
    }

    /// Create with estimated capacity
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            parts: Vec::new(),
            estimated_capacity: capacity,
        }
    }

    /// Append a string part
    pub fn append(&mut self, s: impl Into<Cow<'a, str>>) -> &mut Self {
        let cow = s.into();
        self.estimated_capacity += cow.len();
        self.parts.push(cow);
        self
    }

    /// Append a borrowed string
    pub fn append_borrowed(&mut self, s: &'a str) -> &mut Self {
        self.estimated_capacity += s.len();
        self.parts.push(Cow::Borrowed(s));
        self
    }

    /// Append an owned string
    pub fn append_owned(&mut self, s: String) -> &mut Self {
        self.estimated_capacity += s.len();
        self.parts.push(Cow::Owned(s));
        self
    }

    /// Build the final string
    #[must_use]
    pub fn build(self) -> String {
        let mut result = String::with_capacity(self.estimated_capacity);
        for part in self.parts {
            result.push_str(&part);
        }
        result
    }
}

impl<'a> Default for EfficientStringBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

/// Utilities for working with paths efficiently
pub mod path {
    use std::borrow::Cow;
    use std::path::Path;

    /// Convert a path to a Cow<Path> to avoid allocations
    #[must_use]
    pub fn to_cow_path(path: &Path) -> Cow<'_, Path> {
        Cow::Borrowed(path)
    }

    /// Join paths efficiently, only allocating when necessary
    #[must_use]
    pub fn efficient_join<'a>(base: &'a Path, component: &str) -> Cow<'a, Path> {
        if component.is_empty() {
            Cow::Borrowed(base)
        } else {
            let mut path = base.to_path_buf();
            path.push(component);
            Cow::Owned(path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_pool() {
        let mut pool = StringPool::new();

        let s1 = pool.intern("hello");
        let s2 = pool.intern("hello");
        let s3 = pool.intern("world");

        // Same string should return same Arc
        assert!(Arc::ptr_eq(&s1, &s2));
        assert!(!Arc::ptr_eq(&s1, &s3));

        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn test_efficient_env_var() {
        let borrowed = EfficientEnvVar::borrowed("KEY", "value");
        assert!(borrowed.is_borrowed());

        let owned = EfficientEnvVar::owned("KEY".to_string(), "value".to_string());
        assert!(!owned.is_borrowed());

        let (key, value) = borrowed.into_owned();
        assert_eq!(key, "KEY");
        assert_eq!(value, "value");
    }

    #[test]
    fn test_efficient_string_builder() {
        let mut builder = EfficientStringBuilder::new();
        builder
            .append_borrowed("Hello, ")
            .append_owned("world".to_string())
            .append_borrowed("!");

        let result = builder.build();
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_efficient_string_trait() {
        let s = "hello";
        let cow = s.to_cow();
        assert!(matches!(cow, Cow::Borrowed(_)));

        let owned = String::from("hello");
        let shared = owned.to_shared();
        assert_eq!(&*shared, "hello");
    }
}
