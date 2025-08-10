//! Improved versions of existing tests that address quality issues
//! 
//! This module demonstrates how to enhance existing test cases to provide
//! better coverage, more realistic scenarios, and stronger assertions.

use cuenv_core::{Error, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use serde::{Deserialize, Serialize};

/// Improved version of basic cache operations test
/// Original version only tested happy path with artificial data
#[tokio::test]
async fn test_cache_operations_improved() {
    let temp_dir = TempDir::new().unwrap();
    
    // Use realistic data instead of artificial test patterns
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct ProjectConfig {
        name: String,
        version: String,
        dependencies: Vec<String>,
        build_script: String,
        metadata: HashMap<String, String>,
    }
    
    let config = ProjectConfig {
        name: "my-web-app".to_string(),
        version: "1.2.3".to_string(),
        dependencies: vec![
            "react@18.2.0".to_string(),
            "typescript@4.9.5".to_string(),
            "webpack@5.75.0".to_string(),
        ],
        build_script: "npm run build && npm run test".to_string(),
        metadata: [
            ("author".to_string(), "John Doe".to_string()),
            ("license".to_string(), "MIT".to_string()),
            ("repository".to_string(), "https://github.com/user/repo".to_string()),
        ].into_iter().collect(),
    };
    
    let cache = cuenv::cache::CacheBuilder::new(temp_dir.path())
        .build_async()
        .await
        .unwrap();
    
    let key = "project:my-web-app:config";
    
    // Test 1: Cache miss behavior
    let start = Instant::now();
    let result: Option<ProjectConfig> = cache.get(key).await.unwrap();
    let miss_duration = start.elapsed();
    
    assert_eq!(result, None);
    assert!(!cache.contains(key).await.unwrap());
    
    // Test 2: Cache put and hit behavior
    cache.put(key, &config, Some(Duration::from_secs(3600))).await.unwrap();
    
    let start = Instant::now();
    let result: Option<ProjectConfig> = cache.get(key).await.unwrap();
    let hit_duration = start.elapsed();
    
    assert_eq!(result, Some(config.clone()));
    assert!(cache.contains(key).await.unwrap());
    
    // Cache hit should be faster than miss (heuristic validation)
    assert!(hit_duration < miss_duration + Duration::from_millis(50));
    
    // Test 3: Metadata validation
    let metadata = cache.metadata(key).await.unwrap().unwrap();
    assert!(metadata.size_bytes > 100); // Realistic size check
    assert!(!metadata.content_hash.is_empty());
    assert!(metadata.created_at <= std::time::SystemTime::now());
    assert!(metadata.accessed_at <= std::time::SystemTime::now());
    
    // Test 4: Cache invalidation
    assert!(cache.remove(key).await.unwrap());
    assert!(!cache.contains(key).await.unwrap());
    assert!(!cache.remove(key).await.unwrap()); // Double remove should return false
    
    // Test 5: Error handling with invalid keys
    let invalid_keys = ["", "key with spaces", "key/with/too/many/slashes/a/b/c/d/e/f/g/h"];
    for invalid_key in invalid_keys {
        let result = cache.get::<ProjectConfig>(invalid_key).await;
        assert!(result.is_err(), "Should reject invalid key: {}", invalid_key);
    }
}

/// Improved error message testing with specific validation
/// Original tests only checked that error occurred, not error quality
#[test]
fn test_error_message_quality() {
    let temp_dir = TempDir::new().unwrap();
    
    // Test 1: Missing CUE file
    let output = Command::new(env!("CARGO_BIN_EXE_cuenv"))
        .current_dir(temp_dir.path())
        .arg("load")
        .output()
        .expect("Failed to execute cuenv");
    
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Validate error message quality
    assert!(stderr.contains("env.cue"), "Should mention the expected file name");
    assert!(stderr.contains("not found") || stderr.contains("No such file"), 
        "Should clearly state the file is missing");
    assert!(!stderr.contains("panic") && !stderr.contains("unwrap"), 
        "Should not expose internal error details");
    
    // Test 2: Invalid CUE syntax with specific location
    let invalid_cue = r#"package env
env: {
    DATABASE_URL: "postgres://localhost/db"
    API_KEY: "missing quote
    PORT: 3000
}
"#;
    fs::write(temp_dir.path().join("env.cue"), invalid_cue).unwrap();
    
    let output = Command::new(env!("CARGO_BIN_EXE_cuenv"))
        .current_dir(temp_dir.path())
        .arg("load")
        .output()
        .expect("Failed to execute cuenv");
    
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Validate syntax error reporting
    assert!(stderr.contains("syntax") || stderr.contains("parse"), 
        "Should identify this as a syntax error");
    assert!(stderr.contains("line") || stderr.contains("position"), 
        "Should provide location information");
    assert!(stderr.contains("quote") || stderr.contains("string"), 
        "Should hint at the specific issue");
    
    // Test 3: Permission denied scenario
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        
        let restricted_file = temp_dir.path().join("restricted.cue");
        fs::write(&restricted_file, "package env\nenv: {}").unwrap();
        
        // Remove read permissions
        let mut perms = fs::metadata(&restricted_file).unwrap().permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&restricted_file, perms).unwrap();
        
        let output = Command::new(env!("CARGO_BIN_EXE_cuenv"))
            .current_dir(temp_dir.path())
            .arg("load")
            .arg("--config")
            .arg("restricted.cue")
            .output()
            .expect("Failed to execute cuenv");
        
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        assert!(stderr.contains("permission") || stderr.contains("access"), 
            "Should identify permission issue");
        assert!(stderr.contains("restricted.cue"), 
            "Should mention the specific file");
        
        // Restore permissions for cleanup
        let mut perms = fs::metadata(&restricted_file).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&restricted_file, perms).unwrap();
    }
    
    // Test 4: Invalid capability reference
    let invalid_capability_cue = r#"package env
env: {
    SECRET_VALUE: "secret" @capability("nonexistent")
}
"#;
    fs::write(temp_dir.path().join("env.cue"), invalid_capability_cue).unwrap();
    
    let output = Command::new(env!("CARGO_BIN_EXE_cuenv"))
        .current_dir(temp_dir.path())
        .arg("exec")
        .arg("--capability")
        .arg("nonexistent")
        .arg("echo")
        .arg("test")
        .output()
        .expect("Failed to execute cuenv");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Should either warn about unknown capability or fail gracefully
    if !output.status.success() {
        assert!(stderr.contains("capability") && stderr.contains("nonexistent"), 
            "Should identify the unknown capability");
    } else {
        assert!(stderr.contains("warning") || stderr.contains("unknown"), 
            "Should warn about unknown capability");
    }
}

/// Improved property-based test with realistic data generation
/// Original version used artificial random data
#[cfg(test)]
mod realistic_property_tests {
    use super::*;
    use proptest::prelude::*;
    
    // Generate realistic environment variable configurations
    prop_compose! {
        fn arb_realistic_env_config()
            (service_name in "[a-z][a-z0-9-]{2,20}",
             environment in prop_oneof!["development", "staging", "production"],
             port in 1000u16..65535,
             log_level in prop_oneof!["trace", "debug", "info", "warn", "error"],
             database_type in prop_oneof!["postgres", "mysql", "mongodb", "redis"])
            (config in arb_env_vars(service_name, environment, port, log_level, database_type)) -> HashMap<String, String>
        {
            config
        }
    }
    
    fn arb_env_vars(
        service_name: String,
        environment: String, 
        port: u16,
        log_level: String,
        database_type: String,
    ) -> impl Strategy<Value = HashMap<String, String>> {
        let mut vars = HashMap::new();
        
        vars.insert("SERVICE_NAME".to_string(), service_name.clone());
        vars.insert("ENVIRONMENT".to_string(), environment.clone());
        vars.insert("PORT".to_string(), port.to_string());
        vars.insert("LOG_LEVEL".to_string(), log_level);
        
        // Generate realistic database URLs
        let db_url = match database_type.as_str() {
            "postgres" => format!("postgres://localhost:5432/{}_{}",
                service_name, environment),
            "mysql" => format!("mysql://localhost:3306/{}_{}",
                service_name, environment),
            "mongodb" => format!("mongodb://localhost:27017/{}_{}",
                service_name, environment),
            "redis" => "redis://localhost:6379".to_string(),
            _ => "sqlite://./app.db".to_string(),
        };
        vars.insert("DATABASE_URL".to_string(), db_url);
        
        // Add environment-specific variables
        match environment.as_str() {
            "production" => {
                vars.insert("DEBUG".to_string(), "false".to_string());
                vars.insert("METRICS_ENABLED".to_string(), "true".to_string());
            },
            "development" => {
                vars.insert("DEBUG".to_string(), "true".to_string());
                vars.insert("HOT_RELOAD".to_string(), "true".to_string());
            },
            _ => {
                vars.insert("DEBUG".to_string(), "false".to_string());
            }
        }
        
        Just(vars)
    }
    
    proptest! {
        #[test]
        fn test_realistic_env_configurations(config in arb_realistic_env_config()) {
            let temp_dir = TempDir::new().unwrap();
            
            // Generate CUE content from the configuration
            let mut cue_content = String::from("package env\n\nenv: {\n");
            for (key, value) in &config {
                cue_content.push_str(&format!("    {}: \"{}\"\n", key, value));
            }
            cue_content.push_str("}\n");
            
            fs::write(temp_dir.path().join("env.cue"), &cue_content).unwrap();
            
            // Test that cuenv can parse and load this configuration
            let output = Command::new(env!("CARGO_BIN_EXE_cuenv"))
                .current_dir(temp_dir.path())
                .arg("exec")
                .arg("sh")
                .arg("-c")
                .arg("env | sort")
                .output()
                .expect("Failed to execute cuenv");
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                panic!("Failed to load config: {}\nConfig: {:?}", stderr, config);
            }
            
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            // Verify all expected variables are present
            for (key, expected_value) in &config {
                assert!(stdout.contains(&format!("{}={}", key, expected_value)),
                    "Missing or incorrect variable: {}={}", key, expected_value);
            }
            
            // Verify environment-specific constraints
            let environment = config.get("ENVIRONMENT").unwrap();
            let debug = config.get("DEBUG").unwrap();
            
            match environment.as_str() {
                "production" => {
                    assert_eq!(debug, "false", "Production should have DEBUG=false");
                    assert!(stdout.contains("METRICS_ENABLED=true"));
                },
                "development" => {
                    assert_eq!(debug, "true", "Development should have DEBUG=true");
                    assert!(stdout.contains("HOT_RELOAD=true"));
                },
                _ => {} // staging can have either
            }
        }
    }
}

/// Improved concurrency test with realistic contention scenarios
/// Original tests used artificial workloads
#[tokio::test]
async fn test_realistic_cache_concurrency() {
    let temp_dir = TempDir::new().unwrap();
    let cache = std::sync::Arc::new(
        cuenv::cache::CacheBuilder::new(temp_dir.path())
            .build_async()
            .await
            .unwrap()
    );
    
    // Simulate realistic development workflow concurrency:
    // Multiple developers running builds, tests, and deployments simultaneously
    
    let tasks = vec![
        // Frontend build cache
        ("frontend:build:abc123", serde_json::json!({
            "type": "build",
            "inputs": ["src/", "package.json"],
            "outputs": ["dist/"],
            "duration_ms": 15000
        })),
        
        // Backend test cache  
        ("backend:test:def456", serde_json::json!({
            "type": "test",
            "inputs": ["src/", "tests/", "go.mod"],
            "coverage": 85.5,
            "duration_ms": 8000
        })),
        
        // Database migration cache
        ("db:migrate:migration_123", serde_json::json!({
            "type": "migration",
            "schema_version": 123,
            "applied_at": "2023-11-15T10:30:00Z"
        })),
        
        // Deployment artifact cache
        ("deploy:prod:v1.2.3", serde_json::json!({
            "type": "deployment",
            "version": "1.2.3",
            "environment": "production",
            "artifacts": ["app.tar.gz", "config.yaml"]
        })),
    ];
    
    // Run concurrent operations
    let handles = tasks.into_iter().map(|(key, value)| {
        let cache = cache.clone();
        let key = key.to_string();
        
        tokio::spawn(async move {
            // Simulate realistic access pattern:
            // 1. Check if cached
            // 2. If not, compute and cache
            // 3. Multiple reads
            
            for iteration in 0..5 {
                if !cache.contains(&key).await.unwrap() {
                    // Simulate work time (e.g., compilation, testing)
                    let work_duration = match key.split(':').nth(1) {
                        Some("build") => Duration::from_millis(100), // Build takes longer
                        Some("test") => Duration::from_millis(50),   // Tests are faster
                        Some("migrate") => Duration::from_millis(20), // Quick migrations
                        _ => Duration::from_millis(30),
                    };
                    tokio::time::sleep(work_duration).await;
                    
                    // Cache the result
                    cache.put(&key, &value, Some(Duration::from_secs(3600))).await.unwrap();
                }
                
                // Read from cache (this should be fast after first iteration)
                let start = Instant::now();
                let result: Option<serde_json::Value> = cache.get(&key).await.unwrap();
                let read_duration = start.elapsed();
                
                assert!(result.is_some(), "Cache miss on iteration {}", iteration);
                
                // After first iteration, reads should be much faster
                if iteration > 0 {
                    assert!(read_duration < Duration::from_millis(10), 
                        "Cache read too slow: {:?}", read_duration);
                }
                
                // Small delay between iterations
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            
            key
        })
    }).collect::<Vec<_>>();
    
    // Wait for all concurrent operations to complete
    let results = futures::future::join_all(handles).await;
    
    // Verify all operations completed successfully
    for result in results {
        assert!(result.is_ok(), "Concurrent operation failed: {:?}", result);
    }
    
    // Verify cache state is consistent
    for key in ["frontend:build:abc123", "backend:test:def456", 
                "db:migrate:migration_123", "deploy:prod:v1.2.3"] {
        assert!(cache.contains(key).await.unwrap(), 
            "Cache missing expected key: {}", key);
        
        let metadata = cache.metadata(key).await.unwrap().unwrap();
        assert!(metadata.size_bytes > 0);
        assert!(!metadata.content_hash.is_empty());
    }
}

/// Improved test demonstrating better assertions and validation
/// Original tests had weak assertions that didn't validate intended behavior
#[test]
fn test_environment_isolation_comprehensive() {
    let temp_dir = TempDir::new().unwrap();
    
    let env_content = r#"package env
env: {
    APP_NAME: "test-app"
    DATABASE_URL: "postgres://localhost/testdb"
    SECRET_KEY: "test-secret-123"
    DEBUG: true
}
"#;
    fs::write(temp_dir.path().join("env.cue"), env_content).unwrap();
    
    // Set some environment variables that should NOT leak into cuenv execution
    std::env::set_var("PARENT_SECRET", "should-not-leak");
    std::env::set_var("DATABASE_URL", "postgres://wrong-db/wrong");
    std::env::set_var("DEBUG", "false");
    
    let output = Command::new(env!("CARGO_BIN_EXE_cuenv"))
        .current_dir(temp_dir.path())
        .arg("exec")
        .arg("sh")
        .arg("-c")
        // Comprehensive environment check
        .arg(r#"
            echo "=== CUENV VARS ==="
            echo "APP_NAME=$APP_NAME"
            echo "DATABASE_URL=$DATABASE_URL" 
            echo "SECRET_KEY=$SECRET_KEY"
            echo "DEBUG=$DEBUG"
            echo "=== PARENT VARS ==="
            echo "PARENT_SECRET=${PARENT_SECRET:-UNSET}"
            echo "=== PATH CHECK ==="
            echo "PATH_SET=$(test -n "$PATH" && echo YES || echo NO)"
            echo "=== COUNT ==="
            env | wc -l
        "#)
        .output()
        .expect("Failed to execute cuenv");
    
    assert!(output.status.success(), 
        "Command failed: {}", 
        String::from_utf8_lossy(&output.stderr));
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Positive assertions: CUE-defined variables should be present with correct values
    assert!(stdout.contains("APP_NAME=test-app"));
    assert!(stdout.contains("DATABASE_URL=postgres://localhost/testdb")); // CUE value, not parent
    assert!(stdout.contains("SECRET_KEY=test-secret-123"));
    assert!(stdout.contains("DEBUG=true")); // CUE value, not parent
    
    // Negative assertions: Parent environment should not leak
    assert!(stdout.contains("PARENT_SECRET=UNSET"));
    
    // PATH should still be available (needed for shell to work)
    assert!(stdout.contains("PATH_SET=YES"));
    
    // Environment should be minimal - roughly just our 4 vars plus PATH and a few system vars
    // This is a heuristic but helps detect environment pollution
    let env_lines: Vec<&str> = stdout.lines()
        .find(|line| line.contains("=== COUNT ==="))
        .and_then(|_| stdout.lines().last())
        .map(|line| line.trim())
        .unwrap_or("999")
        .split_whitespace()
        .collect();
    
    if let Ok(count) = env_lines[0].parse::<i32>() {
        assert!(count < 20, 
            "Environment has too many variables ({}), possible leakage", count);
        assert!(count >= 5, 
            "Environment has too few variables ({}), missing expected vars", count);
    }
    
    // Clean up
    std::env::remove_var("PARENT_SECRET");
    std::env::remove_var("DATABASE_URL");
    std::env::remove_var("DEBUG");
}