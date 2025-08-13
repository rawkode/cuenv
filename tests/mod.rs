// Test modules organization
mod behaviours;
mod examples;
mod shells;
mod snapshots;

// Re-export existing integration tests that will be reorganized
pub mod integration {
    include!("integration_test.rs");
    include!("monorepo_integration_test.rs");
    include!("hooks_integration_test.rs");
    include!("task_integration_test.rs");
}

pub mod security {
    include!("security_integration_test.rs");
    include!("landlock_integration_test.rs");
    include!("security_fixes_test.rs");
}

pub mod cache {
    include!("cache_phase_integration_test.rs");
    include!("cache_security_integration_test.rs");
    include!("cache_key_integration_test.rs");
    include!("concurrent_cache_test.rs");
}

pub mod performance {
    include!("phase3_performance_test.rs");
    include!("phase4_eviction_test.rs");
}

pub mod property {
    include!("cue_parser_property_tests.rs");
    include!("cache_property_tests.rs");
    include!("access_restrictions_property_tests.rs");
}
