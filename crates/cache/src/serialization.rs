// Serde helpers for types that don't implement Serialize/Deserialize

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime};

/// A wrapper for Instant that can be serialized/deserialized
#[derive(Debug, Clone, Copy)]
pub struct SerializableInstant {
    /// Elapsed time since creation (as Instant doesn't have an absolute value)
    elapsed_nanos: u64,
}

impl SerializableInstant {
    pub fn now() -> Self {
        Self { elapsed_nanos: 0 }
    }

    pub fn from_instant(base: Instant, instant: Instant) -> Self {
        let elapsed = instant.duration_since(base);
        Self {
            elapsed_nanos: elapsed.as_nanos() as u64,
        }
    }

    pub fn to_instant(&self, base: Instant) -> Instant {
        base + Duration::from_nanos(self.elapsed_nanos)
    }
}

impl Serialize for SerializableInstant {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.elapsed_nanos.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SerializableInstant {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let elapsed_nanos = u64::deserialize(deserializer)?;
        Ok(Self { elapsed_nanos })
    }
}

// Re-export a module with serde-compatible time helpers
pub mod time {
    use super::*;

    /// Serialize an Instant as elapsed nanoseconds from a base instant
    pub mod instant_as_nanos {
        use super::*;
        use serde::{Deserializer, Serializer};
        use std::sync::OnceLock;

        static BASE_INSTANT: OnceLock<Instant> = OnceLock::new();

        fn get_base_instant() -> Instant {
            *BASE_INSTANT.get_or_init(Instant::now)
        }

        pub fn serialize<S>(instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let base = get_base_instant();
            let elapsed = instant.duration_since(base).as_nanos() as u64;
            elapsed.serialize(serializer)
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
        where
            D: Deserializer<'de>,
        {
            let elapsed_nanos = u64::deserialize(deserializer)?;
            let base = get_base_instant();
            Ok(base + Duration::from_nanos(elapsed_nanos))
        }
    }

    /// Serialize an Option<Instant> as elapsed nanoseconds
    pub mod option_instant_as_nanos {
        use super::*;
        use serde::{Deserializer, Serializer};

        pub fn serialize<S>(instant: &Option<Instant>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match instant {
                Some(i) => instant_as_nanos::serialize(i, serializer),
                None => serializer.serialize_none(),
            }
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Instant>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let opt = Option::<u64>::deserialize(deserializer)?;
            match opt {
                Some(nanos) => {
                    let start = std::time::UNIX_EPOCH;
                    let duration = Duration::from_nanos(nanos);
                    Ok(Some(
                        Instant::now()
                            - (SystemTime::now().duration_since(start).unwrap() - duration),
                    ))
                }
                None => Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_serializable_instant_creation() {
        let instant = SerializableInstant::now();
        assert_eq!(instant.elapsed_nanos, 0);
    }

    #[test]
    fn test_serializable_instant_from_instant() {
        let base = Instant::now();
        thread::sleep(Duration::from_nanos(1_000_000)); // 1ms
        let later = Instant::now();
        
        let serializable = SerializableInstant::from_instant(base, later);
        assert!(serializable.elapsed_nanos > 0);
        assert!(serializable.elapsed_nanos >= 1_000_000); // At least 1ms
    }

    #[test]
    fn test_serializable_instant_roundtrip() {
        let base = Instant::now();
        let original = base + Duration::from_millis(100);
        
        let serializable = SerializableInstant::from_instant(base, original);
        let recovered = serializable.to_instant(base);
        
        // Should be very close (within nanosecond precision limits)
        let diff = if recovered > original { 
            recovered - original 
        } else { 
            original - recovered 
        };
        assert!(diff < Duration::from_nanos(1000), "Roundtrip should preserve instant within 1000ns");
    }

    #[test]
    fn test_serializable_instant_serialization() {
        let instant = SerializableInstant { elapsed_nanos: 42_000_000_000 }; // 42 seconds
        
        let serialized = serde_json::to_string(&instant).expect("Should serialize");
        let deserialized: SerializableInstant = serde_json::from_str(&serialized).expect("Should deserialize");
        
        assert_eq!(instant.elapsed_nanos, deserialized.elapsed_nanos);
    }

    #[test] 
    fn test_serializable_instant_zero() {
        let base = Instant::now();
        let same_instant = base;
        
        let serializable = SerializableInstant::from_instant(base, same_instant);
        assert_eq!(serializable.elapsed_nanos, 0);
        
        let recovered = serializable.to_instant(base);
        assert_eq!(recovered, base);
    }

    #[test]
    fn test_instant_as_nanos_serialization() {
        #[derive(Serialize, Deserialize)]
        struct TestStruct {
            #[serde(with = "instant_as_nanos")]
            timestamp: Instant,
        }
        
        let test_struct = TestStruct {
            timestamp: Instant::now(),
        };
        
        let serialized = serde_json::to_string(&test_struct).expect("Should serialize");
        let deserialized: TestStruct = serde_json::from_str(&serialized).expect("Should deserialize");
        
        // The deserialized instant should be close to the original (within reasonable bounds)
        let diff = if deserialized.timestamp > test_struct.timestamp {
            deserialized.timestamp - test_struct.timestamp
        } else {
            test_struct.timestamp - deserialized.timestamp
        };
        assert!(diff < Duration::from_secs(1), "Timestamps should be close after serialization");
    }

    #[test]
    fn test_option_instant_some_serialization() {
        #[derive(Serialize, Deserialize)]
        struct TestStruct {
            #[serde(with = "option_instant_as_nanos")]
            maybe_timestamp: Option<Instant>,
        }
        
        let test_struct = TestStruct {
            maybe_timestamp: Some(Instant::now()),
        };
        
        let serialized = serde_json::to_string(&test_struct).expect("Should serialize Some");
        let deserialized: TestStruct = serde_json::from_str(&serialized).expect("Should deserialize Some");
        
        assert!(deserialized.maybe_timestamp.is_some(), "Should deserialize as Some");
    }

    #[test]
    fn test_option_instant_none_serialization() {
        #[derive(Serialize, Deserialize)]
        struct TestStruct {
            #[serde(with = "option_instant_as_nanos")]
            maybe_timestamp: Option<Instant>,
        }
        
        let test_struct = TestStruct {
            maybe_timestamp: None,
        };
        
        let serialized = serde_json::to_string(&test_struct).expect("Should serialize None");
        let deserialized: TestStruct = serde_json::from_str(&serialized).expect("Should deserialize None");
        
        assert!(deserialized.maybe_timestamp.is_none(), "Should deserialize as None");
    }

    #[test]
    fn test_large_elapsed_time() {
        let instant = SerializableInstant { 
            elapsed_nanos: u64::MAX / 2 // Very large but safe value
        };
        
        let serialized = serde_json::to_string(&instant).expect("Should serialize large values");
        let deserialized: SerializableInstant = serde_json::from_str(&serialized).expect("Should deserialize large values");
        
        assert_eq!(instant.elapsed_nanos, deserialized.elapsed_nanos);
    }
}
