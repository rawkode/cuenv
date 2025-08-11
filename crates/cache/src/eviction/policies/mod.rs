//! Eviction policy implementations

mod arc;
mod lfu;
mod lru;

pub use arc::ArcPolicy;
pub use lfu::LfuPolicy;
pub use lru::LruPolicy;
