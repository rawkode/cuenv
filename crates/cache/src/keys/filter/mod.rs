//! Environment variable filtering logic and smart defaults

pub mod patterns;
pub mod smart_defaults;
pub mod stats;

pub use patterns::PatternMatcher;
pub use smart_defaults::SmartDefaults;
pub use stats::FilterStats;
