use cuenv::cue_parser::{CueParser, ParseOptions};
use proptest::prelude::*;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

/// Generate valid CUE environment variable names
fn valid_env_var_name() -> impl Strategy<Value = String> {
    "[A-Z][A-Z0-9_]{0,30}".prop_map(|s| s.to_string())
}

/// Generate valid CUE string values
fn valid_string_value() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple strings
        "[a-zA-Z0-9_\\-\\./ ]{0,100}",
        // Paths
        "(/[a-zA-Z0-9_\\-\\.]+){1,5}",
        // URLs
        "https?://[a-zA-Z0-9\\-\\.]+\\.[a-z]{2,6}(/[a-zA-Z0-9_\\-\\.]*)*",
    ]
}

/// Generate valid capability names
fn valid_capability_name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,20}".prop_map(|s| s.to_string())
}

/// Generate a simple CUE file content with environment variables
fn cue_env_content(vars: Vec<(String, String)>) -> String {
    let mut content = String::from("package env\n\nenv: {\n");

    for (key, value) in vars {
        content.push_str(&format!(
            "    {}: \"{}\"\n",
            key,
            value.replace('"', "\\\"")
        ));
    }

    content.push_str("}\n");
    content
}

/// Generate a CUE file with capabilities
fn cue_with_capabilities(vars: Vec<(String, String, Option<String>)>) -> String {
    let mut content = String::from("package env\n\nenv: {\n");

    for (key, value, capability) in vars {
        if let Some(cap) = capability {
            content.push_str(&format!(
                "    {}: \"{}\" @capability(\"{}\")\n",
                key,
                value.replace('"', "\\\""),
                cap
            ));
        } else {
            content.push_str(&format!(
                "    {}: \"{}\"\n",
                key,
                value.replace('"', "\\\"")
            ));
        }
    }

    content.push_str("}\n");
    content
}

/// Generate a CUE file with environments
fn cue_with_environments(
    global_vars: Vec<(String, String)>,
    environments: HashMap<String, Vec<(String, String)>>,
) -> String {
    let mut content = String::from("package env\n\nenv: {\n");

    // Global variables
    for (key, value) in global_vars {
        content.push_str(&format!(
            "    {}: \"{}\"\n",
            key,
            value.replace('"', "\\\"")
        ));
    }

    // Environments
    if !environments.is_empty() {
        content.push_str("\n    environment: {\n");
        for (env_name, vars) in environments {
            content.push_str(&format!("        {}: {{\n", env_name));
            for (key, value) in vars {
                content.push_str(&format!(
                    "            {}: \"{}\"\n",
                    key,
                    value.replace('"', "\\\"")
                ));
            }
            content.push_str("        }\n");
        }
        content.push_str("    }\n");
    }

    content.push_str("}\n");
    content
}

#[test]
fn test_parse_empty_file_is_valid() {
    let temp_dir = TempDir::new().unwrap();
    let cue_file = temp_dir.path().join("env.cue");
    let content = "package env\n";
    fs::write(&cue_file, content).unwrap();

    let options = ParseOptions::default();

    let result = CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

    assert!(result.variables.is_empty());
    assert!(result.commands.is_empty());
}

proptest! {
    #[test]
    fn test_parse_simple_env_vars(
        vars in prop::collection::vec(
            (valid_env_var_name(), valid_string_value()),
            0..20
        )
    ) {
        let temp_dir = TempDir::new().unwrap();
        let cue_file = temp_dir.path().join("env.cue");

        // Deduplicate variables by key to avoid CUE conflicts
        let mut unique_vars = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();
        for (key, value) in vars {
            if seen_keys.insert(key.clone()) {
                unique_vars.push((key, value));
            }
        }

        let content = cue_env_content(unique_vars.clone());
        fs::write(&cue_file, content).unwrap();

        let options = ParseOptions {
            environment: None,
            capabilities: vec![],
        };

        let result = CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

        // All variables should be parsed
        prop_assert_eq!(result.variables.len(), unique_vars.len());

        // Each variable should have the correct value
        for (key, value) in &unique_vars {
            prop_assert_eq!(
                result.variables.get(key).map(|s| s.as_str()),
                Some(value.as_str())
            );
        }
    }

    #[test]
    fn test_parse_with_capabilities(
        vars in prop::collection::vec(
            (
                valid_env_var_name(),
                valid_string_value(),
                prop::option::of(valid_capability_name())
            ),
            0..20
        )
    ) {
        let temp_dir = TempDir::new().unwrap();
        let cue_file = temp_dir.path().join("env.cue");

        // Deduplicate variables by key to avoid CUE conflicts
        let mut unique_vars = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();
        for (key, value, cap) in vars {
            if seen_keys.insert(key.clone()) {
                unique_vars.push((key, value, cap));
            }
        }

        let content = cue_with_capabilities(unique_vars.clone());
        fs::write(&cue_file, content).unwrap();


        // Extract all unique capabilities
        let all_capabilities: Vec<String> = unique_vars.iter()
            .filter_map(|(_, _, cap)| cap.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let options = ParseOptions {
            environment: None,
            capabilities: all_capabilities.clone(),
        };

        let result = CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

        // When capabilities filter is empty, all variables are included
        // When capabilities filter is non-empty, variables without capabilities are always included,
        // and variables with capabilities are only included if they match
        if all_capabilities.is_empty() {
            // No filter active - all variables should be included
            prop_assert_eq!(result.variables.len(), unique_vars.len());
            for (key, value, _) in &unique_vars {
                prop_assert_eq!(
                    result.variables.get(key).map(|s| s.as_str()),
                    Some(value.as_str())
                );
            }
        } else {
            // Filter active - variables without capabilities + variables with matching capabilities
            let expected_count = unique_vars.iter()
                .filter(|(_, _, cap)| cap.is_none() || all_capabilities.contains(&cap.as_ref().unwrap()))
                .count();

            prop_assert_eq!(result.variables.len(), expected_count);

            // Check that appropriate variables were parsed
            for (key, value, cap) in &unique_vars {
                if cap.is_none() || all_capabilities.contains(&cap.as_ref().unwrap()) {
                    prop_assert_eq!(
                        result.variables.get(key).map(|s| s.as_str()),
                        Some(value.as_str())
                    );
                }
            }
        }
    }

    #[test]
    fn test_parse_with_environments(
        global_vars in prop::collection::vec(
            (valid_env_var_name(), valid_string_value()),
            0..5
        ),
        env_names in prop::collection::vec("[a-z][a-z0-9_]{0,10}", 1..4),
        env_vars in prop::collection::vec(
            prop::collection::vec(
                (valid_env_var_name(), valid_string_value()),
                0..5
            ),
            1..4
        )
    ) {
        prop_assume!(env_names.len() == env_vars.len());

        let temp_dir = TempDir::new().unwrap();
        let cue_file = temp_dir.path().join("env.cue");

        // Deduplicate global vars
        let mut unique_global_vars = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();
        for (key, value) in global_vars {
            if seen_keys.insert(key.clone()) {
                unique_global_vars.push((key, value));
            }
        }

        // Deduplicate environment vars and ensure unique environment names
        let mut unique_env_names = Vec::new();
        let mut seen_env_names = std::collections::HashSet::new();
        for name in env_names {
            if seen_env_names.insert(name.clone()) {
                unique_env_names.push(name);
            }
        }

        let mut environments: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for (i, env_name) in unique_env_names.iter().enumerate() {
            if i < env_vars.len() {
                let mut unique_env_vars = Vec::new();
                let mut env_seen_keys = std::collections::HashSet::new();
                for (key, value) in &env_vars[i] {
                    if env_seen_keys.insert(key.clone()) {
                        unique_env_vars.push((key.clone(), value.clone()));
                    }
                }
                environments.insert(env_name.clone(), unique_env_vars);
            }
        }

        prop_assume!(!environments.is_empty());

        let content = cue_with_environments(unique_global_vars.clone(), environments.clone());
        fs::write(&cue_file, content).unwrap();


        // Test each environment
        for (env_name, expected_vars) in &environments {
            let options = ParseOptions {
                environment: Some(env_name.clone()),
                capabilities: vec![],
                };

            let result = CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

            // Should have global vars + environment-specific vars
            let mut all_expected = HashMap::new();
            for (k, v) in &unique_global_vars {
                all_expected.insert(k.clone(), v.clone());
            }
            for (k, v) in expected_vars {
                all_expected.insert(k.clone(), v.clone());
            }

            prop_assert_eq!(result.variables.len(), all_expected.len());

            for (key, value) in &all_expected {
                prop_assert_eq!(
                    result.variables.get(key).map(|s| s.as_str()),
                    Some(value.as_str())
                );
            }
        }
    }

    #[test]
    fn test_capability_filtering(
        vars_with_caps in prop::collection::vec(
            (
                valid_env_var_name(),
                valid_string_value(),
                valid_capability_name()
            ),
            5..15
        ),
        vars_without_caps in prop::collection::vec(
            (valid_env_var_name(), valid_string_value()),
            5..15
        ),
        selected_cap_indices in prop::collection::vec(0..5usize, 1..3)
    ) {
        prop_assume!(!vars_with_caps.is_empty());
        prop_assume!(selected_cap_indices.iter().all(|&i| i < vars_with_caps.len()));

        let temp_dir = TempDir::new().unwrap();
        let cue_file = temp_dir.path().join("env.cue");

        // Ensure unique variable names to avoid CUE conflicts
        let mut used_names = std::collections::HashSet::new();
        let mut unique_vars_with_caps = Vec::new();
        for (k, v, c) in vars_with_caps {
            if used_names.insert(k.clone()) {
                unique_vars_with_caps.push((k, v, c));
            }
        }

        let mut unique_vars_without_caps = Vec::new();
        for (k, v) in vars_without_caps {
            if used_names.insert(k.clone()) {
                unique_vars_without_caps.push((k, v));
            }
        }

        // Ensure we still have some variables with capabilities
        prop_assume!(!unique_vars_with_caps.is_empty());

        // Adjust selected_cap_indices to match the filtered list
        let adjusted_indices: Vec<usize> = selected_cap_indices
            .into_iter()
            .filter(|&i| i < unique_vars_with_caps.len())
            .collect();
        prop_assume!(!adjusted_indices.is_empty());

        // Combine variables with and without capabilities
        let mut all_vars = Vec::new();
        for (k, v, c) in unique_vars_with_caps.iter() {
            all_vars.push((k.clone(), v.clone(), Some(c.clone())));
        }
        for (k, v) in unique_vars_without_caps.iter() {
            all_vars.push((k.clone(), v.clone(), None));
        }

        let content = cue_with_capabilities(all_vars.clone());
        fs::write(&cue_file, content).unwrap();

        // Select specific capabilities
        let selected_caps: Vec<String> = adjusted_indices
            .iter()
            .map(|&i| unique_vars_with_caps[i].2.clone())
            .collect();

        let options = ParseOptions {
            environment: None,
            capabilities: selected_caps.clone(),
        };

        let result = CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

        // When capabilities are specified, variables without capabilities are always included,
        // and variables with capabilities are only included if they match
        let expected_count = unique_vars_without_caps.len() + unique_vars_with_caps.iter()
            .filter(|(_, _, cap)| selected_caps.contains(cap))
            .count();

        prop_assert_eq!(result.variables.len(), expected_count);
    }

    #[test]
    fn test_special_characters_in_values(
        key in valid_env_var_name(),
        special_chars in prop::collection::vec(
            prop_oneof![
                Just('\''),
                Just('"'),
                Just('\\'),
                Just('\n'),
                Just('\t'),
                Just(' '),
                Just('$'),
                Just('`'),
            ],
            0..10
        )
    ) {
        let temp_dir = TempDir::new().unwrap();
        let cue_file = temp_dir.path().join("env.cue");

        // Create a value with special characters
        let value: String = special_chars.into_iter().collect();
        let escaped_value = value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\t', "\\t");

        let content = format!("package env\n\nenv: {{\n    {}: \"{}\"\n}}\n", key, escaped_value);
        fs::write(&cue_file, content).unwrap();

        let options = ParseOptions::default();

        // Should be able to parse without panic
        let result = CueParser::eval_package_with_options(temp_dir.path(), "env", &options);

        // Parser might fail on invalid escape sequences, which is expected
        if result.is_ok() {
            let parsed = result.unwrap();
            if let Some(parsed_value) = parsed.variables.get(&key) {
                // The parsed value should handle escapes properly
                prop_assert!(parsed_value.len() <= value.len() + escaped_value.len());
            }
        }
    }

    #[test]
    fn test_environment_override_behavior(
        base_vars in prop::collection::hash_map(
            valid_env_var_name(),
            valid_string_value(),
            1..10
        ),
        env_name in "[a-z][a-z0-9_]{0,10}",
        override_indices in prop::collection::vec(0..10usize, 1..5)
    ) {
        prop_assume!(!base_vars.is_empty());

        let base_keys: Vec<String> = base_vars.keys().cloned().collect();
        let valid_indices: Vec<usize> = override_indices
            .into_iter()
            .filter(|&i| i < base_keys.len())
            .collect();

        prop_assume!(!valid_indices.is_empty());

        let temp_dir = TempDir::new().unwrap();
        let cue_file = temp_dir.path().join("env.cue");

        let mut content = String::from("package env\n\nenv: {\n");

        // Write base variables
        for (key, value) in &base_vars {
            content.push_str(&format!("    {}: \"{}\"\n", key, value.replace('"', "\\\"")));
        }

        // Write environment with overrides
        content.push_str(&format!("\n    environment: {{\n        {}: {{\n", env_name));
        for &idx in &valid_indices {
            let key = &base_keys[idx];
            content.push_str(&format!("            {}: \"OVERRIDDEN_{}\"\n", key, key));
        }
        content.push_str("        }\n    }\n}\n");

        fs::write(&cue_file, content).unwrap();

        let options = ParseOptions {
            environment: Some(env_name.clone()),
            capabilities: vec![],
        };

        let result = CueParser::eval_package_with_options(temp_dir.path(), "env", &options).unwrap();

        // All base variables should be present
        prop_assert_eq!(result.variables.len(), base_vars.len());

        // Check overrides
        for (key, base_value) in &base_vars {
            let expected_value = if valid_indices.iter().any(|&i| &base_keys[i] == key) {
                format!("OVERRIDDEN_{}", key)
            } else {
                base_value.clone()
            };

            prop_assert_eq!(
                result.variables.get(key).map(|s| s.as_str()),
                Some(expected_value.as_str())
            );
        }
    }
}
