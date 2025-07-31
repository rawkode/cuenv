//! Network-related utilities.
//!
//! This module provides tools for managing network interactions, including
//! rate limiting to prevent overwhelming services and retry logic to handle
//! transient network failures gracefully.
//!
//! ## Key Components
//!
//! - **`rate_limit`**: Implements various rate-limiting strategies, such as
//!   token bucket and sliding window, to control the frequency of operations.
//! - **`retry`**: Offers flexible, exponential backoff retry mechanisms to
//!   robustly handle temporary errors.

pub mod rate_limit;
pub mod retry;
