//! Pattern matching for key access control

/// Check pattern matching for key access
pub fn matches_pattern(key: &str, pattern: &str) -> bool {
    // Simple glob pattern matching
    if pattern == "*" {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix('*') {
        return key.starts_with(prefix);
    }

    if let Some(suffix) = pattern.strip_prefix('*') {
        return key.ends_with(suffix);
    }

    key == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        // Test wildcard patterns
        assert!(matches_pattern("any/key", "*"));
        assert!(matches_pattern("prefix/test", "prefix/*"));
        assert!(matches_pattern("test/suffix", "*/suffix"));
        assert!(matches_pattern("exact", "exact"));

        // Test non-matches
        assert!(!matches_pattern("other/key", "prefix/*"));
        assert!(!matches_pattern("prefix/test", "*/suffix"));
        assert!(!matches_pattern("almost", "exact"));
    }
}
