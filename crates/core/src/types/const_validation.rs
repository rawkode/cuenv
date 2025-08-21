//! Compile-time validation using const generics and type-level programming

use crate::errors::{Error, Result};
use std::marker::PhantomData;

/// Compile-time string length validation
pub struct ValidatedString<const MIN_LEN: usize, const MAX_LEN: usize> {
    value: String,
}

impl<const MIN_LEN: usize, const MAX_LEN: usize> ValidatedString<MIN_LEN, MAX_LEN> {
    /// Create a new validated string with compile-time length bounds
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        let len = value.len();

        if len < MIN_LEN {
            return Err(Error::Configuration {
                message: format!("String length {len} is less than minimum {MIN_LEN}"),
            });
        }

        if len > MAX_LEN {
            return Err(Error::Configuration {
                message: format!("String length {len} is greater than maximum {MAX_LEN}"),
            });
        }

        Ok(Self { value })
    }

    /// Get the inner value
    pub fn get(&self) -> &str {
        &self.value
    }

    /// Convert to String
    pub fn into_string(self) -> String {
        self.value
    }

    /// Get the minimum length as a const
    pub const fn min_len() -> usize {
        MIN_LEN
    }

    /// Get the maximum length as a const
    pub const fn max_len() -> usize {
        MAX_LEN
    }
}

// Type aliases for common string validations
pub type NonEmptyString = ValidatedString<1, 1024>;
pub type TaskNameString = ValidatedString<1, 64>;
pub type CommandString = ValidatedString<1, 256>;
pub type PathString = ValidatedString<1, 4096>;

/// Compile-time numeric range validation
pub struct ValidatedNumber<T, const MIN: i64, const MAX: i64> {
    value: T,
    _phantom: PhantomData<T>,
}

impl<T, const MIN: i64, const MAX: i64> ValidatedNumber<T, MIN, MAX>
where
    T: Copy + Into<i64> + TryFrom<i64>,
    <T as TryFrom<i64>>::Error: std::fmt::Display,
{
    /// Create a new validated number with compile-time range bounds
    pub fn new(value: T) -> Result<Self> {
        let val_i64 = value.into();

        if val_i64 < MIN {
            return Err(Error::Configuration {
                message: format!("Value {val_i64} is less than minimum {MIN}"),
            });
        }

        if val_i64 > MAX {
            return Err(Error::Configuration {
                message: format!("Value {val_i64} is greater than maximum {MAX}"),
            });
        }

        Ok(Self {
            value,
            _phantom: PhantomData,
        })
    }

    /// Get the inner value
    pub fn get(&self) -> T {
        self.value
    }

    /// Get the minimum value as a const
    pub const fn min_val() -> i64 {
        MIN
    }

    /// Get the maximum value as a const
    pub const fn max_val() -> i64 {
        MAX
    }
}

// Type aliases for common numeric validations
pub type PortNumber = ValidatedNumber<u16, 1, 65535>;
pub type TimeoutSeconds = ValidatedNumber<u32, 1, 3600>;
pub type RetryCount = ValidatedNumber<u8, 0, 10>;
pub type ConcurrencyLevel = ValidatedNumber<u16, 1, 1000>;

/// Compile-time array size validation
pub struct ValidatedArray<T, const MIN_SIZE: usize, const MAX_SIZE: usize> {
    items: Vec<T>,
}

impl<T, const MIN_SIZE: usize, const MAX_SIZE: usize> ValidatedArray<T, MIN_SIZE, MAX_SIZE> {
    /// Create a new validated array with compile-time size bounds
    pub fn new(items: Vec<T>) -> Result<Self> {
        let size = items.len();

        if size < MIN_SIZE {
            return Err(Error::Configuration {
                message: format!("Array size {size} is less than minimum {MIN_SIZE}"),
            });
        }

        if size > MAX_SIZE {
            return Err(Error::Configuration {
                message: format!("Array size {size} is greater than maximum {MAX_SIZE}"),
            });
        }

        Ok(Self { items })
    }

    /// Get the items
    pub fn get(&self) -> &[T] {
        &self.items
    }

    /// Convert to Vec
    pub fn into_vec(self) -> Vec<T> {
        self.items
    }

    /// Get the minimum size as a const
    pub const fn min_size() -> usize {
        MIN_SIZE
    }

    /// Get the maximum size as a const
    pub const fn max_size() -> usize {
        MAX_SIZE
    }

    /// Iterate over items
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if empty (only possible if MIN_SIZE is 0)
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

// Type aliases for common array validations
pub type NonEmptyArgs = ValidatedArray<String, 1, 256>;
pub type TaskDependencies = ValidatedArray<String, 0, 50>;
pub type EnvVarList = ValidatedArray<(String, String), 0, 1000>;

/// Compile-time capacity validation for collections
pub trait CapacityBound<const CAPACITY: usize> {
    type Item;

    /// Check if adding an item would exceed capacity
    fn would_exceed_capacity(&self) -> bool;

    /// Get remaining capacity
    fn remaining_capacity(&self) -> usize;

    /// Get the capacity bound as a const
    const CAPACITY: usize = CAPACITY;
}

/// A collection with compile-time capacity bounds
pub struct BoundedVec<T, const CAPACITY: usize> {
    items: Vec<T>,
}

impl<T, const CAPACITY: usize> BoundedVec<T, CAPACITY> {
    /// Create a new bounded vector
    pub fn new() -> Self {
        Self {
            items: Vec::with_capacity(CAPACITY),
        }
    }

    /// Try to push an item, returning error if capacity would be exceeded
    pub fn try_push(&mut self, item: T) -> Result<()> {
        if self.items.len() >= CAPACITY {
            return Err(Error::Configuration {
                message: format!("Cannot add item: capacity {CAPACITY} exceeded"),
            });
        }

        self.items.push(item);
        Ok(())
    }

    /// Get the items
    pub fn get(&self) -> &[T] {
        &self.items
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Check if at capacity
    pub fn is_full(&self) -> bool {
        self.items.len() >= CAPACITY
    }
}

impl<T, const CAPACITY: usize> CapacityBound<CAPACITY> for BoundedVec<T, CAPACITY> {
    type Item = T;

    fn would_exceed_capacity(&self) -> bool {
        self.items.len() >= CAPACITY
    }

    fn remaining_capacity(&self) -> usize {
        CAPACITY.saturating_sub(self.items.len())
    }
}

impl<T, const CAPACITY: usize> Default for BoundedVec<T, CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

/// Type-level boolean for compile-time feature flags
pub trait Bool {
    const VALUE: bool;
}

pub struct True;
pub struct False;

impl Bool for True {
    const VALUE: bool = true;
}

impl Bool for False {
    const VALUE: bool = false;
}

/// Conditionally compiled features using const generics
pub struct ConditionalFeature<T, B: Bool> {
    inner: T,
    _phantom: PhantomData<B>,
}

impl<T, B: Bool> ConditionalFeature<T, B> {
    /// Create a new conditional feature
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            _phantom: PhantomData,
        }
    }

    /// Check if the feature is enabled at compile time
    pub const fn is_enabled() -> bool {
        B::VALUE
    }

    /// Get the inner value
    pub fn get(&self) -> &T {
        &self.inner
    }

    /// Conditionally execute a function based on the feature flag
    pub fn if_enabled<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        if B::VALUE {
            Some(f(&self.inner))
        } else {
            None
        }
    }
}

// Type aliases for common conditional features
pub type DebugFeature<T> = ConditionalFeature<T, True>;
pub type ProductionFeature<T> = ConditionalFeature<T, False>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validated_string() {
        type ShortString = ValidatedString<1, 10>;

        assert!(ShortString::new("hello").is_ok());
        assert!(ShortString::new("").is_err()); // Too short
        assert!(ShortString::new("this is too long").is_err()); // Too long
    }

    #[test]
    fn test_validated_number() {
        type SmallNumber = ValidatedNumber<i32, 1, 100>;

        assert!(SmallNumber::new(50).is_ok());
        assert!(SmallNumber::new(0).is_err()); // Too small
        assert!(SmallNumber::new(150).is_err()); // Too large
    }

    #[test]
    fn test_validated_array() {
        type SmallArray = ValidatedArray<String, 1, 3>;

        assert!(SmallArray::new(vec!["one".to_string(), "two".to_string()]).is_ok());
        assert!(SmallArray::new(vec![]).is_err()); // Too small
        assert!(SmallArray::new(vec![
            "one".to_string(),
            "two".to_string(),
            "three".to_string(),
            "four".to_string()
        ])
        .is_err()); // Too large
    }

    #[test]
    fn test_bounded_vec() {
        let mut vec: BoundedVec<i32, 3> = BoundedVec::new();

        assert!(vec.try_push(1).is_ok());
        assert!(vec.try_push(2).is_ok());
        assert!(vec.try_push(3).is_ok());
        assert!(vec.try_push(4).is_err()); // Exceeds capacity

        assert_eq!(vec.len(), 3);
        assert!(vec.is_full());
    }

    #[test]
    fn test_conditional_feature() {
        let debug_feature: DebugFeature<String> = DebugFeature::new("debug info".to_string());
        let production_feature: ProductionFeature<String> =
            ProductionFeature::new("production info".to_string());

        assert!(DebugFeature::<()>::is_enabled());
        assert!(!ProductionFeature::<()>::is_enabled());

        let debug_result = debug_feature.if_enabled(|s| s.len());
        let production_result = production_feature.if_enabled(|s| s.len());

        assert!(debug_result.is_some());
        assert!(production_result.is_none());
    }
}
