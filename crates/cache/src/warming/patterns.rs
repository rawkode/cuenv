//! Pattern learning and predictive warming

use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

/// Warming patterns for predictive loading
pub(crate) struct WarmingPatterns {
    /// Sequential access patterns
    pub sequential_patterns: HashMap<String, Vec<String>>,
    /// Time-based patterns (hour of day -> keys)
    pub temporal_patterns: HashMap<u8, HashSet<String>>,
    /// Related keys that are often accessed together
    pub related_keys: HashMap<String, HashSet<String>>,
}

impl WarmingPatterns {
    pub fn new() -> Self {
        Self {
            sequential_patterns: HashMap::new(),
            temporal_patterns: HashMap::new(),
            related_keys: HashMap::new(),
        }
    }

    /// Learn access pattern from a sequence of keys
    pub fn learn_pattern(&mut self, keys: &[String]) {
        if keys.len() < 2 {
            return;
        }

        // Learn sequential patterns
        for window in keys.windows(2) {
            let pattern_key = window[0].clone();
            self.sequential_patterns
                .entry(pattern_key)
                .or_default()
                .push(window[1].clone());
        }

        // Learn temporal patterns
        let current_hour = Self::get_current_hour();

        for key in keys {
            self.temporal_patterns
                .entry(current_hour)
                .or_default()
                .insert(key.clone());
        }

        // Learn related keys (accessed in same batch)
        if keys.len() > 1 {
            for i in 0..keys.len() {
                for j in 0..keys.len() {
                    if i != j {
                        self.related_keys
                            .entry(keys[i].clone())
                            .or_default()
                            .insert(keys[j].clone());
                    }
                }
            }
        }
    }

    /// Get predictive candidates based on current time and selected keys
    pub fn get_predictive_candidates(&self, selected_keys: &HashSet<String>) -> Vec<String> {
        let mut candidates = Vec::new();

        // Add temporal patterns for current hour
        let current_hour = Self::get_current_hour();
        if let Some(hourly_keys) = self.temporal_patterns.get(&current_hour) {
            for key in hourly_keys {
                if !selected_keys.contains(key) {
                    candidates.push(key.clone());
                }
            }
        }

        // Add related keys for already-selected candidates
        for key in selected_keys {
            if let Some(related) = self.related_keys.get(key) {
                for related_key in related {
                    if !selected_keys.contains(related_key)
                        && !candidates.contains(related_key)
                    {
                        candidates.push(related_key.clone());
                    }
                }
            }
        }

        candidates
    }

    fn get_current_hour() -> u8 {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        ((now.as_secs() / 3600) % 24) as u8
    }
}