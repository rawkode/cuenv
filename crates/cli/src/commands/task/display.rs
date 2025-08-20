use crossterm::style::Stylize;
use cuenv_config::{TaskCollection, TaskConfig, TaskNode};
use indexmap::IndexMap;

/// Box drawing characters for tree visualization
const TREE_BRANCH: &str = "├── ";
const TREE_LAST: &str = "└── ";
const TREE_PIPE: &str = "│   ";
const TREE_EMPTY: &str = "    ";

/// Maximum depth for tree traversal
const MAX_DEPTH: usize = 10;

/// Display a single task
fn display_task(
    name: &str,
    config: &TaskConfig,
    verbose: bool,
    use_color: bool,
    _depth: usize,
    connector: &str,
) {
    let task_line = format_task_line(
        name,
        config.description.as_deref(),
        connector,
        verbose,
        use_color,
    );
    println!("{task_line}");
}

/// Count tasks recursively in a node
pub fn count_tasks(node: &TaskNode) -> usize {
    match node {
        TaskNode::Task(_) => 1,
        TaskNode::Group { tasks, .. } => match tasks {
            TaskCollection::Sequential(task_list) => task_list.iter().map(count_tasks).sum(),
            TaskCollection::Parallel(task_map) => task_map.values().map(count_tasks).sum(),
        },
    }
}

/// Display task nodes in a tree format
pub fn display_task_tree(nodes: &IndexMap<String, TaskNode>, verbose: bool, use_color: bool) {
    // IndexMap already preserves insertion order, no need to sort
    let sorted = nodes;

    // Simple, clean header
    println!();
    if use_color {
        println!("{}", "Tasks".bold());
    } else {
        println!("Tasks");
    }

    // Display all tasks in a unified format
    for (name, node) in sorted {
        match node {
            TaskNode::Task(_config) => {
                // Display single task with a simple bullet
                if use_color {
                    println!("  {} {}", "•".dark_grey(), name);
                } else {
                    println!("  • {name}");
                }
            }
            TaskNode::Group {
                description: _,
                tasks,
            } => {
                if verbose {
                    // Verbose mode: show tree structure
                    display_group_collection(
                        name, None, // Don't show descriptions in list view
                        tasks, verbose, use_color, 0, "",
                    );
                } else {
                    // Compact mode: single line per group
                    display_group_compact_collection(name, tasks, use_color);
                }
            }
        }
    }

    // Footer
    println!();
    if verbose {
        // Full footer with hints
        if use_color {
            println!("{}", "─".repeat(40).dark_grey());
            println!("Usage: cuenv task <name> [args...]");
            println!();
            println!(
                "{}",
                "Task groups can be executed directly or you can run specific subtasks."
                    .dark_grey()
            );
        } else {
            println!("{}", "-".repeat(40));
            println!("Usage: cuenv task <name> [args...]");
            println!();
            println!("Task groups can be executed directly or you can run specific subtasks.");
        }
    } else {
        // Minimal footer with icon key
        if use_color {
            println!("{}", "─".repeat(40).dark_grey());
            println!(
                "{} parallel  {} sequential  {} group  {} single",
                "⚡".cyan(),
                "⇢".yellow(),
                "⇉".green(),
                "•".dark_grey()
            );
            println!();
            println!("Run: cuenv task <name>  •  Use {} for details", "-v".cyan());
        } else {
            println!("{}", "-".repeat(40));
            println!("Run: cuenv task <name>  •  Use -v for details");
        }
    }
}

/// Display a group in compact format (single line)
fn display_group_compact_collection(name: &str, tasks: &TaskCollection, use_color: bool) {
    // Get list of subtask names
    let mut subtask_names: Vec<String> = match tasks {
        TaskCollection::Sequential(task_list) => task_list
            .iter()
            .enumerate()
            .map(|(i, _)| format!("task_{i}"))
            .collect(),
        TaskCollection::Parallel(task_map) => {
            task_map
                .keys()
                .map(|k| {
                    // Get just the last part of the name for cleaner display
                    if let Some(last_dot) = k.rfind('.') {
                        k[last_dot + 1..].to_string()
                    } else {
                        k.clone()
                    }
                })
                .collect()
        }
    };

    if matches!(tasks, TaskCollection::Parallel(_)) {
        subtask_names.sort();
    }

    // Create task list display
    let task_list = if subtask_names.len() > 4 {
        format!("{} …", subtask_names[..4].join(" "))
    } else {
        subtask_names.join(" ")
    };

    if use_color {
        // Distinct icons for each mode
        let (symbol, color) = match tasks {
            TaskCollection::Sequential(_) => ("⇢", name.yellow()),
            TaskCollection::Parallel(_) => ("⇉", name.green()),
        };

        println!(
            "  {} {} {}",
            symbol,
            color.bold(),
            format!("[{task_list}]").dark_grey()
        );
    } else {
        println!("  {name} [{task_list}]");
    }
}

/// Display a group and its contents (TaskCollection version)
#[allow(clippy::too_many_arguments)]
fn display_group_collection(
    name: &str,
    description: Option<&str>,
    tasks: &TaskCollection,
    verbose: bool,
    use_color: bool,
    depth: usize,
    prefix: &str,
) {
    let task_count = match tasks {
        TaskCollection::Sequential(task_list) => task_list.iter().map(count_tasks).sum::<usize>(),
        TaskCollection::Parallel(task_map) => task_map.values().map(count_tasks).sum::<usize>(),
    };

    let (mode_name, mode_color) = match tasks {
        TaskCollection::Sequential(_) => ("SEQ", "\x1b[94m"), // Blue
        TaskCollection::Parallel(_) => ("PAR", "\x1b[92m"),   // Green
    };

    let mode_badge = if use_color {
        format!("\x1b[1m{mode_color}[{mode_name}]\x1b[0m")
    } else {
        format!("[{mode_name}]")
    };

    // Format group line
    let indent = " ".repeat(depth * 4);
    let formatted_name = format!("{indent}{prefix}{name}");

    if use_color {
        print!("{}", formatted_name.cyan().bold());
        print!(" {}", mode_badge);
        println!(" ({task_count} tasks)");
    } else {
        println!("{formatted_name} {mode_badge} ({task_count} tasks)");
    }

    // Display description if present and verbose
    if verbose {
        if let Some(desc) = description {
            let desc_indent = " ".repeat(depth * 4 + 2);
            if use_color {
                println!("{desc_indent}{}", desc.dark_grey());
            } else {
                println!("{desc_indent}{desc}");
            }
        }
    }
}

/// Helper to format a task line consistently
fn format_task_line(
    name: &str,
    description: Option<&str>,
    connector: &str,
    verbose: bool,
    use_color: bool,
) -> String {
    if verbose && description.is_some() {
        if use_color {
            format!(
                "{}{} {}",
                connector,
                name,
                format!("– {}", description.unwrap()).dark_grey()
            )
        } else {
            format!("{}{} – {}", connector, name, description.unwrap())
        }
    } else {
        format!("{connector}{name}")
    }
}

/// Display children with tree structure (TaskCollection version)
fn display_tree_children_collection(
    collection: &TaskCollection,
    verbose: bool,
    use_color: bool,
    depth: usize,
) {
    // Convert TaskCollection to an IndexMap-like structure for display
    match collection {
        TaskCollection::Sequential(tasks) => {
            let total = tasks.len();
            let mut count = 0;

            for (index, node) in tasks.iter().enumerate() {
                count += 1;
                let is_last = count == total;
                let connector = if is_last { TREE_LAST } else { TREE_BRANCH };
                let child_prefix = if is_last { TREE_EMPTY } else { TREE_PIPE };
                let name = format!("task_{}", index);

                match node {
                    TaskNode::Task(config) => {
                        display_task(&name, config, verbose, use_color, depth, connector);
                    }
                    TaskNode::Group {
                        description,
                        tasks: subtasks,
                    } => {
                        display_group_collection(
                            &name,
                            description.as_deref(),
                            subtasks,
                            verbose,
                            use_color,
                            depth,
                            connector,
                        );

                        // Recursively display children
                        if depth < MAX_DEPTH {
                            print!("{}", " ".repeat(depth * 4 + 2));
                            println!("{child_prefix}");
                            display_tree_children_collection(
                                subtasks,
                                verbose,
                                use_color,
                                depth + 1,
                            );
                        }
                    }
                }
            }
        }
        TaskCollection::Parallel(task_map) => {
            // For parallel collections, use the existing display function
            display_tree_children(task_map, verbose, use_color, depth);
        }
    }
}

/// Display children with tree structure
fn display_tree_children(
    nodes: &IndexMap<String, TaskNode>,
    verbose: bool,
    use_color: bool,
    depth: usize,
) {
    // IndexMap preserves insertion order, no need to sort
    let total = nodes.len();
    let mut count = 0;

    for (name, node) in nodes {
        count += 1;
        let is_last = count == total;
        let connector = if is_last { TREE_LAST } else { TREE_BRANCH };
        let _child_prefix = if is_last { TREE_EMPTY } else { TREE_PIPE };

        match node {
            TaskNode::Task(config) => {
                let task_line = format_task_line(
                    name,
                    config.description.as_deref(),
                    connector,
                    verbose,
                    use_color,
                );
                println!("{task_line}");
            }
            TaskNode::Group { description, tasks } => {
                display_group_collection(
                    name,
                    description.as_deref(),
                    tasks,
                    verbose,
                    use_color,
                    depth + 1,
                    connector,
                );

                // Recursively display nested content with proper indentation
                if !tasks.is_empty() {
                    display_tree_children_collection(tasks, verbose, use_color, depth + 1);
                }
            }
        }
    }
}

/// Display a single group's contents when listing a specific group
pub fn display_group_contents(
    group_name: &str,
    description: Option<&str>,
    tasks: &TaskCollection,
    verbose: bool,
    use_color: bool,
) {
    let (mode_name, mode_color) = match tasks {
        TaskCollection::Sequential(_) => ("SEQUENTIAL", "\x1b[94m"), // Blue
        TaskCollection::Parallel(_) => ("PARALLEL", "\x1b[92m"),     // Green
    };

    let mode_badge = if use_color {
        format!("\x1b[1m{mode_color}[{mode_name}]\x1b[0m")
    } else {
        format!("[{mode_name}]")
    };

    // Header
    if use_color {
        println!("{} {}", group_name.bold().cyan(), mode_badge);
    } else {
        println!("{group_name} {mode_badge}");
    }

    if let Some(desc) = description {
        if use_color {
            println!("  {}", desc.dark_grey());
        } else {
            println!("  {desc}");
        }
    }

    println!();

    // Display tasks
    display_tree_children_collection(tasks, verbose, use_color, 0);

    // Action hint
    println!();
    let action_hint = match tasks {
        TaskCollection::Sequential(_) => {
            format!("Run 'cuenv task {group_name}' to execute all tasks sequentially")
        }
        TaskCollection::Parallel(_) => {
            format!("Run 'cuenv task {group_name}' to execute tasks based on dependencies")
        }
    };

    if use_color {
        println!("{}", action_hint.dark_grey());
    } else {
        println!("{action_hint}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuenv_config::{TaskConfig, TaskNode};

    fn create_test_task(description: Option<String>) -> TaskNode {
        TaskNode::Task(Box::new(TaskConfig {
            description,
            command: Some("echo test".to_string()),
            script: None,
            dependencies: None,
            working_dir: None,
            shell: None,
            inputs: None,
            outputs: None,
            security: None,
            cache: None,
            cache_key: None,
            cache_env: None,
            timeout: None,
        }))
    }

    fn create_test_group(tasks: TaskCollection, description: Option<String>) -> TaskNode {
        TaskNode::Group { description, tasks }
    }

    #[test]
    fn test_count_tasks_single_task() {
        let task = create_test_task(None);
        assert_eq!(count_tasks(&task), 1);
    }

    #[test]
    fn test_count_tasks_empty_group() {
        let group = create_test_group(TaskCollection::Parallel(IndexMap::new()), None);
        assert_eq!(count_tasks(&group), 0);
    }

    #[test]
    fn test_count_tasks_group_with_tasks() {
        let mut tasks = IndexMap::new();
        tasks.insert("task1".to_string(), create_test_task(None));
        tasks.insert("task2".to_string(), create_test_task(None));

        let group = create_test_group(TaskCollection::Parallel(tasks), None);
        assert_eq!(count_tasks(&group), 2);
    }

    #[test]
    fn test_count_tasks_nested_groups() {
        let mut inner_tasks = IndexMap::new();
        inner_tasks.insert("inner1".to_string(), create_test_task(None));
        inner_tasks.insert("inner2".to_string(), create_test_task(None));
        let inner_group = create_test_group(TaskCollection::Parallel(inner_tasks), None);

        let mut outer_tasks = IndexMap::new();
        outer_tasks.insert("task1".to_string(), create_test_task(None));
        outer_tasks.insert("group1".to_string(), inner_group);

        let outer_group = create_test_group(TaskCollection::Parallel(outer_tasks), None);
        assert_eq!(count_tasks(&outer_group), 3);
    }

    #[test]
    fn test_format_task_line_simple() {
        let result = format_task_line("test_task", None, "├── ", false, false);
        assert_eq!(result, "├── test_task");
    }

    #[test]
    fn test_format_task_line_with_description() {
        let result = format_task_line("test_task", Some("A test task"), "├── ", true, false);
        assert_eq!(result, "├── test_task – A test task");
    }

    #[test]
    fn test_format_task_line_with_color() {
        let result = format_task_line("test_task", Some("A test task"), "├── ", true, true);
        // Just verify it contains the expected parts (color codes make exact comparison difficult)
        assert!(result.contains("├── "));
        assert!(result.contains("test_task"));
        assert!(result.contains("A test task"));
    }

    #[test]
    fn test_format_task_line_non_verbose_with_description() {
        let result = format_task_line("test_task", Some("A test task"), "├── ", false, false);
        assert_eq!(result, "├── test_task");
    }

    #[test]
    fn test_format_task_line_unicode_characters() {
        let result = format_task_line("测试任务", Some("Unicode描述"), "├── ", true, false);
        assert_eq!(result, "├── 测试任务 – Unicode描述");
    }

    #[test]
    fn test_format_task_line_very_long_name() {
        let long_name = "a".repeat(100);
        let result = format_task_line(&long_name, None, "├── ", false, false);
        assert_eq!(result, format!("├── {long_name}"));
    }

    #[test]
    fn test_format_task_line_very_long_description() {
        let long_description = "This is a very long description that goes on and on and contains many words and details about what the task does and why it exists and how it should be executed and what the expected outcome should be".to_string();
        let result = format_task_line("task", Some(&long_description), "├── ", true, false);
        assert_eq!(result, format!("├── task – {long_description}"));
    }

    #[test]
    fn test_format_task_line_empty_description() {
        let result = format_task_line("test_task", Some(""), "├── ", true, false);
        assert_eq!(result, "├── test_task – ");
    }

    #[test]
    fn test_format_task_line_special_characters() {
        let result = format_task_line(
            "test-task_v2.0",
            Some("Task with @special#chars!"),
            "└── ",
            true,
            false,
        );
        assert_eq!(result, "└── test-task_v2.0 – Task with @special#chars!");
    }

    #[test]
    fn test_tree_constants() {
        assert_eq!(TREE_BRANCH, "├── ");
        assert_eq!(TREE_LAST, "└── ");
        assert_eq!(TREE_PIPE, "│   ");
        assert_eq!(TREE_EMPTY, "    ");
    }

    #[test]
    fn test_display_task_tree_empty() {
        let tasks = IndexMap::new();
        // This should not panic and should handle empty input gracefully
        display_task_tree(&tasks, false, false);
        display_task_tree(&tasks, true, false);
        display_task_tree(&tasks, false, true);
        display_task_tree(&tasks, true, true);
    }

    #[test]
    fn test_display_task_tree_single_task() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "simple_task".to_string(),
            create_test_task(Some("A simple task".to_string())),
        );

        // Test all combinations of verbose and color flags
        display_task_tree(&tasks, false, false);
        display_task_tree(&tasks, true, false);
        display_task_tree(&tasks, false, true);
        display_task_tree(&tasks, true, true);
    }

    #[test]
    fn test_display_task_tree_multiple_tasks() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "task_a".to_string(),
            create_test_task(Some("First task".to_string())),
        );
        tasks.insert(
            "task_b".to_string(),
            create_test_task(Some("Second task".to_string())),
        );
        tasks.insert("task_c".to_string(), create_test_task(None));

        display_task_tree(&tasks, false, false);
        display_task_tree(&tasks, true, true);
    }

    #[test]
    fn test_display_task_tree_with_groups() {
        let mut inner_tasks = IndexMap::new();
        inner_tasks.insert(
            "subtask1".to_string(),
            create_test_task(Some("Sub task 1".to_string())),
        );
        inner_tasks.insert(
            "subtask2".to_string(),
            create_test_task(Some("Sub task 2".to_string())),
        );

        let mut tasks = IndexMap::new();
        tasks.insert(
            "single_task".to_string(),
            create_test_task(Some("A standalone task".to_string())),
        );
        tasks.insert(
            "group1".to_string(),
            create_test_group(
                TaskCollection::Parallel(inner_tasks),
                Some("A parallel group".to_string()),
            ),
        );

        display_task_tree(&tasks, false, false);
        display_task_tree(&tasks, true, true);
    }

    #[test]
    fn test_display_group_contents_all_modes() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "task1".to_string(),
            create_test_task(Some("First task".to_string())),
        );
        tasks.insert(
            "task2".to_string(),
            create_test_task(Some("Second task".to_string())),
        );

        let collections = [
            TaskCollection::Sequential(vec![
                create_test_task(Some("Task 1".to_string())),
                create_test_task(Some("Task 2".to_string())),
            ]),
            TaskCollection::Parallel(tasks.clone()),
        ];

        for collection in &collections {
            display_group_contents(
                "test_group",
                Some("Test group description"),
                collection,
                false,
                false,
            );

            display_group_contents(
                "test_group",
                Some("Test group description"),
                collection,
                true,
                true,
            );
        }
    }

    #[test]
    fn test_display_group_contents_no_description() {
        let mut tasks = IndexMap::new();
        tasks.insert("task1".to_string(), create_test_task(None));

        display_group_contents(
            "test_group",
            None,
            &TaskCollection::Parallel(tasks),
            false,
            false,
        );
    }

    #[test]
    fn test_display_group_contents_empty_tasks() {
        let tasks = IndexMap::new();

        display_group_contents(
            "empty_group",
            Some("An empty group"),
            &TaskCollection::Parallel(tasks),
            true,
            true,
        );
    }

    #[test]
    fn test_display_group_contents_unicode_names() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "测试任务".to_string(),
            create_test_task(Some("Unicode任务描述".to_string())),
        );

        display_group_contents(
            "Unicode组",
            Some("Unicode组描述"),
            &TaskCollection::Parallel(tasks),
            true,
            true,
        );
    }

    #[test]
    fn test_display_group_compact_mode() {
        let mut tasks = IndexMap::new();
        for i in 1..=6 {
            tasks.insert(
                format!("task{i}"),
                create_test_task(Some(format!("Task {i} description"))),
            );
        }

        // This tests the "4 tasks shown, others as …" functionality
        let group = create_test_group(
            TaskCollection::Parallel(tasks),
            Some("A group with many tasks".to_string()),
        );

        let mut nodes = IndexMap::new();
        nodes.insert("big_group".to_string(), group);

        display_task_tree(&nodes, false, false);
        display_task_tree(&nodes, false, true);
    }

    #[test]
    fn test_display_with_dotted_task_names() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "namespace.task1".to_string(),
            create_test_task(Some("Namespaced task 1".to_string())),
        );
        tasks.insert(
            "namespace.task2".to_string(),
            create_test_task(Some("Namespaced task 2".to_string())),
        );
        tasks.insert(
            "other.namespace.task3".to_string(),
            create_test_task(Some("Deeply namespaced task".to_string())),
        );

        let group = create_test_group(
            TaskCollection::Parallel(tasks),
            Some("Group with namespaced tasks".to_string()),
        );

        let mut nodes = IndexMap::new();
        nodes.insert("namespaced_group".to_string(), group);

        display_task_tree(&nodes, false, false);
        display_task_tree(&nodes, true, true);
    }

    #[test]
    fn test_mixed_task_and_group_nodes() {
        let mut inner_tasks = IndexMap::new();
        inner_tasks.insert(
            "inner1".to_string(),
            create_test_task(Some("Inner task 1".to_string())),
        );
        inner_tasks.insert(
            "inner2".to_string(),
            create_test_task(Some("Inner task 2".to_string())),
        );

        let mut nodes = IndexMap::new();
        nodes.insert(
            "standalone_task".to_string(),
            create_test_task(Some("A standalone task".to_string())),
        );
        nodes.insert(
            "parallel_group".to_string(),
            create_test_group(
                TaskCollection::Parallel(inner_tasks.clone()),
                Some("A parallel group".to_string()),
            ),
        );
        nodes.insert(
            "sequential_group".to_string(),
            create_test_group(
                TaskCollection::Sequential(vec![
                    create_test_task(Some("Sequential task 1".to_string())),
                    create_test_task(Some("Sequential task 2".to_string())),
                ]),
                Some("A sequential group".to_string()),
            ),
        );
        nodes.insert("another_task".to_string(), create_test_task(None));

        display_task_tree(&nodes, false, false);
        display_task_tree(&nodes, true, true);
    }

    #[test]
    fn test_error_handling_edge_cases() {
        // Test with very large numbers of tasks
        let mut tasks = IndexMap::new();
        for i in 0..1000 {
            tasks.insert(
                format!("task_{i:04}"),
                create_test_task(Some(format!("Generated task {i}"))),
            );
        }

        let group = create_test_group(
            TaskCollection::Parallel(tasks),
            Some("Large group".to_string()),
        );

        let mut nodes = IndexMap::new();
        nodes.insert("large_group".to_string(), group);

        // Should handle large numbers without issues
        display_task_tree(&nodes, false, false);
    }
}
