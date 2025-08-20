use cuenv_env::state::StateManager;
use std::collections::HashMap;
use uuid::Uuid;

/// Test isolation helper that provides each test with its own environment variable namespace
pub struct TestIsolation {
    prefix: String,
    original_vars: HashMap<String, Option<String>>,
}

impl TestIsolation {
    /// Create a new isolated test environment
    pub fn new() -> Self {
        let prefix = format!("test_{}", Uuid::new_v4().simple());

        // Store original values of any existing cuenv variables
        let mut original_vars = HashMap::new();
        for var in &[
            "CUENV_PREFIX",
            "CUENV_DIR",
            "CUENV_FILE",
            "CUENV_DIFF",
            "CUENV_WATCHES",
            "CUENV_STATE",
        ] {
            original_vars.insert(var.to_string(), std::env::var(var).ok());
        }

        // Set the test prefix
        std::env::set_var("CUENV_PREFIX", &prefix);

        Self {
            prefix,
            original_vars,
        }
    }

    /// Get the prefix being used for this test
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Clean up the test environment when done
    pub async fn cleanup(self) {
        // First unload any cuenv state
        let _ = StateManager::unload().await;

        // Remove any variables that were set with our prefix
        let prefixed_vars = [
            format!("{}_CUENV_DIR", self.prefix),
            format!("{}_CUENV_FILE", self.prefix),
            format!("{}_CUENV_DIFF", self.prefix),
            format!("{}_CUENV_WATCHES", self.prefix),
            format!("{}_CUENV_STATE", self.prefix),
        ];

        for var in &prefixed_vars {
            std::env::remove_var(var);
        }

        // Restore original values
        for (var, original_value) in self.original_vars.clone() {
            match original_value {
                Some(value) => std::env::set_var(var, value),
                None => std::env::remove_var(var),
            }
        }
    }
}

impl Drop for TestIsolation {
    fn drop(&mut self) {
        // Emergency cleanup if async cleanup wasn't called
        // This is best effort - can't be async in Drop
        let prefixed_vars = [
            format!("{}_CUENV_DIR", self.prefix),
            format!("{}_CUENV_FILE", self.prefix),
            format!("{}_CUENV_DIFF", self.prefix),
            format!("{}_CUENV_WATCHES", self.prefix),
            format!("{}_CUENV_STATE", self.prefix),
        ];

        for var in &prefixed_vars {
            std::env::remove_var(var);
        }

        // Restore original CUENV_PREFIX
        if let Some(original) = self.original_vars.get("CUENV_PREFIX") {
            match original {
                Some(value) => std::env::set_var("CUENV_PREFIX", value),
                None => std::env::remove_var("CUENV_PREFIX"),
            }
        }
    }
}

/// Macro to set up test isolation in a test function
#[macro_export]
macro_rules! isolated_test {
    ($test_name:ident, $body:block) => {
        #[tokio::test]
        async fn $test_name() {
            let _isolation = $crate::common::test_isolation::TestIsolation::new();

            $body

            _isolation.cleanup().await;
        }
    };
}

// Removed with_isolation function - use TestIsolation::new() directly

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_isolation_creates_unique_prefixes() {
        let isolation1 = TestIsolation::new();
        let isolation2 = TestIsolation::new();

        assert_ne!(isolation1.prefix(), isolation2.prefix());
        assert!(isolation1.prefix().starts_with("test_"));
        assert!(isolation2.prefix().starts_with("test_"));

        isolation1.cleanup().await;
        isolation2.cleanup().await;
    }

    #[tokio::test]
    async fn test_isolation_sets_and_cleans_prefix() {
        let original_prefix = std::env::var("CUENV_PREFIX").ok();

        {
            let isolation = TestIsolation::new();
            let current_prefix = std::env::var("CUENV_PREFIX").ok();
            assert!(current_prefix.is_some());
            assert_eq!(current_prefix.as_ref().unwrap(), isolation.prefix());

            isolation.cleanup().await;
        }

        let final_prefix = std::env::var("CUENV_PREFIX").ok();
        assert_eq!(final_prefix, original_prefix);
    }
}
