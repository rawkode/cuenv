//! Test for CUE field ordering preservation
//!
//! This test ensures that when we parse CUE configurations, the field order
//! is preserved as defined in the CUE file, not randomized by Go maps.

#[cfg(test)]
mod tests {
    use super::super::{CueParser, ParseOptions};
    use cuenv_core::constants::DEFAULT_PACKAGE_NAME;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_sequential_task_field_ordering_preserved() {
        // Create a temporary directory for our test
        let temp_dir = TempDir::new().unwrap();
        let cue_file = temp_dir.path().join("env.cue");

        // Write CUE content with tasks in a specific order
        let cue_content = r#"package cuenv

tasks: {
    ordered_sequence: {
        description: "Test sequential task ordering"
        mode: "sequential"
        
        // These should execute in this exact order: alpha, beta, gamma, delta
        alpha: {
            command: "echo"
            args: ["step 1"]
        }
        beta: {
            command: "echo"
            args: ["step 2"]
        }
        gamma: {
            command: "echo"
            args: ["step 3"]
        }
        delta: {
            command: "echo"
            args: ["step 4"]
        }
    }
}
"#;

        fs::write(&cue_file, cue_content).unwrap();

        // Save current directory and change to temp dir for CUE evaluation
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let parse_options = ParseOptions {
            environment: None,
            capabilities: Vec::new(),
        };

        // Parse the CUE file
        let result = CueParser::eval_package_with_options(
            temp_dir.path(),
            DEFAULT_PACKAGE_NAME,
            &parse_options,
        )
        .expect("Failed to parse CUE");

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        // Verify we have the ordered_sequence group
        assert!(
            result.task_nodes.contains_key("ordered_sequence"),
            "Should contain ordered_sequence task group"
        );

        // Get the ordered_sequence group and verify it's a Group
        if let Some(task_node) = result.task_nodes.get("ordered_sequence") {
            match task_node {
                crate::TaskNode::Group { tasks, .. } => {
                    // Convert IndexMap keys to Vec to check ordering
                    let task_names: Vec<&String> = tasks.keys().collect();

                    // This is the critical test: verify field order matches CUE definition
                    assert_eq!(
                        task_names,
                        vec!["alpha", "beta", "gamma", "delta"],
                        "Task field order should match CUE definition order, got: {:?}",
                        task_names
                    );

                    println!("✓ Field ordering test passed: {:?}", task_names);
                }
                _ => panic!("ordered_sequence should be a Group node"),
            }
        }
    }

    #[test]
    fn test_multiple_groups_preserve_individual_ordering() {
        let temp_dir = TempDir::new().unwrap();
        let cue_file = temp_dir.path().join("env.cue");

        // Test with multiple groups, each with their own ordering
        let cue_content = r#"package cuenv

tasks: {
    first_group: {
        mode: "sequential"
        zebra: { command: "echo zebra" }
        alpha: { command: "echo alpha" }
        beta: { command: "echo beta" }
    }
    
    second_group: {
        mode: "sequential"
        omega: { command: "echo omega" }
        gamma: { command: "echo gamma" }
        delta: { command: "echo delta" }
    }
}
"#;

        fs::write(&cue_file, cue_content).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let parse_options = ParseOptions {
            environment: None,
            capabilities: Vec::new(),
        };

        let result = CueParser::eval_package_with_options(
            temp_dir.path(),
            DEFAULT_PACKAGE_NAME,
            &parse_options,
        )
        .expect("Failed to parse CUE");

        std::env::set_current_dir(original_dir).unwrap();

        // Test first_group ordering
        if let Some(crate::TaskNode::Group { tasks, .. }) = result.task_nodes.get("first_group") {
            let task_names: Vec<&String> = tasks.keys().collect();
            assert_eq!(
                task_names,
                vec!["zebra", "alpha", "beta"],
                "first_group field order incorrect: {:?}",
                task_names
            );
        }

        // Test second_group ordering
        if let Some(crate::TaskNode::Group { tasks, .. }) = result.task_nodes.get("second_group") {
            let task_names: Vec<&String> = tasks.keys().collect();
            assert_eq!(
                task_names,
                vec!["omega", "gamma", "delta"],
                "second_group field order incorrect: {:?}",
                task_names
            );
        }

        println!("✓ Multiple group ordering test passed");
    }

    #[test]
    fn test_consistent_ordering_across_multiple_parses() {
        let temp_dir = TempDir::new().unwrap();
        let cue_file = temp_dir.path().join("env.cue");

        let cue_content = r#"package cuenv

tasks: {
    consistency_test: {
        mode: "sequential"
        task_x: { command: "echo x" }
        task_a: { command: "echo a" }
        task_z: { command: "echo z" }
        task_m: { command: "echo m" }
    }
}
"#;

        fs::write(&cue_file, cue_content).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let parse_options = ParseOptions {
            environment: None,
            capabilities: Vec::new(),
        };

        // Parse the same content multiple times
        let mut all_orderings = Vec::new();

        for i in 0..5 {
            let result = CueParser::eval_package_with_options(
                temp_dir.path(),
                DEFAULT_PACKAGE_NAME,
                &parse_options,
            )
            .expect(&format!("Failed to parse CUE on iteration {}", i));

            if let Some(crate::TaskNode::Group { tasks, .. }) =
                result.task_nodes.get("consistency_test")
            {
                let task_names: Vec<String> = tasks.keys().cloned().collect();
                all_orderings.push(task_names);
            }
        }

        std::env::set_current_dir(original_dir).unwrap();

        // All orderings should be identical
        let expected = vec![
            "task_x".to_string(),
            "task_a".to_string(),
            "task_z".to_string(),
            "task_m".to_string(),
        ];
        for (i, ordering) in all_orderings.iter().enumerate() {
            assert_eq!(
                ordering,
                &expected,
                "Parse #{} had different ordering: {:?}, expected: {:?}",
                i + 1,
                ordering,
                expected
            );
        }

        println!(
            "✓ Consistency test passed across {} parses",
            all_orderings.len()
        );
    }
}
