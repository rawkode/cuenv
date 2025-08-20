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

        // Write CUE content with tasks using new array structure
        let cue_content = r#"package cuenv

tasks: {
    ordered_sequence: {
        description: "Test sequential task ordering"
        tasks: [
            {
                command: "echo 'step 1'"
                description: "First step"
            },
            {
                command: "echo 'step 2'"  
                description: "Second step"
            },
            {
                command: "echo 'step 3'"
                description: "Third step"
            },
            {
                command: "echo 'step 4'"
                description: "Fourth step"
            }
        ]
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
                    // Verify it's a sequential collection
                    match tasks {
                        crate::TaskCollection::Sequential(task_list) => {
                            // Sequential tasks should preserve array order
                            assert_eq!(
                                task_list.len(),
                                4,
                                "Should have 4 tasks in sequential order"
                            );

                            // Verify each task has the expected command
                            for (index, task) in task_list.iter().enumerate() {
                                match task {
                                    crate::TaskNode::Task(config) => {
                                        let expected_command = format!("echo 'step {}'", index + 1);
                                        assert_eq!(
                                            config.command.as_ref().unwrap(),
                                            &expected_command
                                        );
                                        let expected_description = format!(
                                            "{} step",
                                            match index {
                                                0 => "First",
                                                1 => "Second",
                                                2 => "Third",
                                                3 => "Fourth",
                                                _ => panic!("Unexpected task index: {index}"),
                                            }
                                        );
                                        assert_eq!(
                                            config.description.as_ref().unwrap(),
                                            &expected_description
                                        );
                                    }
                                    _ => panic!("All items in sequential array should be tasks"),
                                }
                            }

                            println!(
                                "✓ Sequential task ordering test passed: {} tasks in array order",
                                task_list.len()
                            );
                        }
                        crate::TaskCollection::Parallel(_) => {
                            panic!(
                                "ordered_sequence should be a Sequential collection, not Parallel"
                            )
                        }
                    }
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
        tasks: [
            { command: "echo zebra" },
            { command: "echo alpha" }, 
            { command: "echo beta" }
        ]
    }
    
    second_group: {
        tasks: [
            { command: "echo omega" },
            { command: "echo gamma" },
            { command: "echo delta" }
        ]
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

        // Test first_group ordering (should preserve array order)
        if let Some(crate::TaskNode::Group { tasks, .. }) = result.task_nodes.get("first_group") {
            match tasks {
                crate::TaskCollection::Sequential(task_list) => {
                    assert_eq!(task_list.len(), 3, "first_group should have 3 tasks");

                    // Verify order is preserved: zebra, alpha, beta
                    let commands: Vec<_> = task_list
                        .iter()
                        .map(|task| match task {
                            crate::TaskNode::Task(config) => {
                                config.command.as_ref().unwrap().as_str()
                            }
                            _ => panic!("Should be a task"),
                        })
                        .collect();

                    assert_eq!(commands, vec!["echo zebra", "echo alpha", "echo beta"]);
                }
                _ => panic!("first_group should be Sequential"),
            }
        }

        // Test second_group ordering (should preserve array order)
        if let Some(crate::TaskNode::Group { tasks, .. }) = result.task_nodes.get("second_group") {
            match tasks {
                crate::TaskCollection::Sequential(task_list) => {
                    assert_eq!(task_list.len(), 3, "second_group should have 3 tasks");

                    // Verify order is preserved: omega, gamma, delta
                    let commands: Vec<_> = task_list
                        .iter()
                        .map(|task| match task {
                            crate::TaskNode::Task(config) => {
                                config.command.as_ref().unwrap().as_str()
                            }
                            _ => panic!("Should be a task"),
                        })
                        .collect();

                    assert_eq!(commands, vec!["echo omega", "echo gamma", "echo delta"]);
                }
                _ => panic!("second_group should be Sequential"),
            }
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
        tasks: [
            { command: "echo x" },
            { command: "echo a" },
            { command: "echo z" },
            { command: "echo m" }
        ]
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
        let mut all_commands = Vec::new();

        for i in 0..5 {
            let result = CueParser::eval_package_with_options(
                temp_dir.path(),
                DEFAULT_PACKAGE_NAME,
                &parse_options,
            )
            .unwrap_or_else(|_| panic!("Failed to parse CUE on iteration {i}"));

            if let Some(crate::TaskNode::Group { tasks, .. }) =
                result.task_nodes.get("consistency_test")
            {
                match tasks {
                    crate::TaskCollection::Sequential(task_list) => {
                        let commands: Vec<String> = task_list
                            .iter()
                            .map(|task| match task {
                                crate::TaskNode::Task(config) => {
                                    config.command.as_ref().unwrap().clone()
                                }
                                _ => panic!("Should be a task"),
                            })
                            .collect();
                        all_commands.push(commands);
                    }
                    _ => panic!("consistency_test should be Sequential"),
                }
            }
        }

        std::env::set_current_dir(original_dir).unwrap();

        // All orderings should be identical (array order preserved)
        let expected = vec![
            "echo x".to_string(),
            "echo a".to_string(),
            "echo z".to_string(),
            "echo m".to_string(),
        ];
        for (i, commands) in all_commands.iter().enumerate() {
            assert_eq!(
                commands,
                &expected,
                "Parse #{} had different ordering: {:?}, expected: {:?}",
                i + 1,
                commands,
                expected
            );
        }

        println!(
            "✓ Consistency test passed across {} parses",
            all_commands.len()
        );
    }
}
