//! Types for hit rate analysis

/// Hit rate analysis report
#[derive(Debug, Clone)]
pub struct HitRateReport {
    pub one_minute: f64,
    pub five_minutes: f64,
    pub one_hour: f64,
    pub one_day: f64,
    pub key_patterns: Vec<PatternStats>,
    pub operation_types: Vec<OperationStats>,
}

#[derive(Debug, Clone)]
pub struct PatternStats {
    pub pattern: String,
    pub hit_rate: f64,
    pub total_accesses: u64,
}

#[derive(Debug, Clone)]
pub struct OperationStats {
    pub operation: String,
    pub hit_rate: f64,
    pub total_calls: u64,
}
