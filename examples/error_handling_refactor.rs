// Example demonstrating the refactoring pattern from ? operator to explicit error handling

use cuenv::errors::{Error, Result};
use std::fs;
use std::path::Path;

// BEFORE: Using ? operator
#[allow(dead_code)]
fn read_config_before(path: &Path) -> Result<String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return Err(Error::from(e)),
    };
    let trimmed = content.trim().to_string();
    Ok(trimmed)
}

// AFTER: Using explicit match expressions
fn read_config_after(path: &Path) -> Result<String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => return Err(Error::file_system(path, "read", e)),
    };

    let trimmed = content.trim().to_string();
    Ok(trimmed)
}

// Example with multiple operations - BEFORE
#[allow(dead_code)]
fn process_env_file_before(path: &Path) -> Result<Vec<(String, String)>> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return Err(Error::from(e)),
    };
    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => return Err(Error::from(e)),
    };

    let obj = match parsed.as_object() {
        Some(o) => o,
        None => return Err(Error::configuration("Expected JSON object")),
    };

    let mut result = Vec::new();
    for (key, value) in obj {
        let val_str = match value.as_str() {
            Some(s) => s,
            None => {
                return Err(Error::configuration(format!(
                    "Expected string value for key: {}",
                    key
                )))
            }
        };
        result.push((key.clone(), val_str.to_string()));
    }

    Ok(result)
}

// Example with multiple operations - AFTER
fn process_env_file_after(path: &Path) -> Result<Vec<(String, String)>> {
    // Read file with explicit error handling
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => return Err(Error::file_system(path, "read", e)),
    };

    // Parse JSON with explicit error handling
    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(value) => value,
        Err(e) => return Err(Error::from(e)),
    };

    // Extract object with explicit error handling
    let obj = match parsed.as_object() {
        Some(obj) => obj,
        None => return Err(Error::configuration("expected JSON object at root level")),
    };

    let mut result = Vec::new();
    for (key, value) in obj {
        // Extract string value with explicit error handling
        let val_str = match value.as_str() {
            Some(s) => s,
            None => {
                return Err(Error::configuration(format!(
                    "expected string value for key: {}",
                    key
                )))
            }
        };
        result.push((key.clone(), val_str.to_string()));
    }

    Ok(result)
}

// Example with async operations - BEFORE
#[allow(dead_code)]
async fn async_operation_before() -> Result<String> {
    // let response = reqwest::get("https://api.example.com/data").await?;
    // let text = response.text().await?;
    // Ok(text)
    Ok("Example response".to_string())
}

// Example with async operations - AFTER
async fn async_operation_after() -> Result<String> {
    // Note: This is a hypothetical example since reqwest is not a dependency
    // It shows the pattern for async error handling

    // First async operation with explicit handling
    let response = match some_async_operation().await {
        Ok(resp) => resp,
        Err(e) => return Err(Error::configuration(format!("failed to fetch data: {}", e))),
    };

    // Second async operation with explicit handling
    let text = match another_async_operation(response).await {
        Ok(text) => text,
        Err(e) => {
            return Err(Error::configuration(format!(
                "failed to process response: {}",
                e
            )))
        }
    };

    Ok(text)
}

// Placeholder async functions for the example
async fn some_async_operation() -> std::result::Result<String, &'static str> {
    Ok("response".to_string())
}

async fn another_async_operation(data: String) -> std::result::Result<String, &'static str> {
    Ok(format!("processed: {}", data))
}

// Example using if-let pattern
fn using_if_let_pattern(optional_path: Option<&Path>) -> Result<String> {
    // Using if-let for Option handling
    if let Some(path) = optional_path {
        // Read file with match expression
        match fs::read_to_string(path) {
            Ok(content) => Ok(content),
            Err(e) => Err(Error::file_system(path, "read", e)),
        }
    } else {
        Err(Error::configuration("no path provided"))
    }
}

// Example showing early returns with match
fn early_return_pattern(paths: Vec<&Path>) -> Result<Vec<String>> {
    let mut results = Vec::new();

    for path in paths {
        // Early return on first error
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => return Err(Error::file_system(path, "read", e)),
        };

        results.push(content);
    }

    Ok(results)
}

fn main() {
    println!("This example demonstrates error handling refactoring patterns");
    println!("See the source code for before/after comparisons");
}
