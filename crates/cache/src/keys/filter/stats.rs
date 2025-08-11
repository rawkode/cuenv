//! Statistics tracking for environment variable filtering

/// Statistics about environment variable filtering
#[derive(Debug, Clone)]
pub struct FilterStats {
    pub total_vars: usize,
    pub filtered_vars: usize,
    pub excluded_vars: usize,
}

impl FilterStats {
    /// Calculate the exclusion rate as a percentage
    pub fn exclusion_rate(&self) -> f64 {
        if self.total_vars == 0 {
            0.0
        } else {
            (self.excluded_vars as f64) / (self.total_vars as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_stats() {
        let stats = FilterStats {
            total_vars: 10,
            filtered_vars: 6,
            excluded_vars: 4,
        };

        assert_eq!(stats.total_vars, 10);
        assert_eq!(stats.filtered_vars, 6);
        assert_eq!(stats.excluded_vars, 4);
        assert!((stats.exclusion_rate() - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_filter_stats_empty() {
        let stats = FilterStats {
            total_vars: 0,
            filtered_vars: 0,
            excluded_vars: 0,
        };

        assert_eq!(stats.exclusion_rate(), 0.0);
    }
}
