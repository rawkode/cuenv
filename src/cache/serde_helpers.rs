// Serde helpers for types that don't implement Serialize/Deserialize

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
