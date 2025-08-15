use crossterm::style::Stylize;
use cuenv_config::{TaskGroupMode, TaskNode};
use std::collections::{BTreeMap, HashMap};

/// Box drawing characters for tree visualization
const TREE_BRANCH: &str = "├── ";
const TREE_LAST: &str = "└── ";
const TREE_PIPE: &str = "│   ";
const TREE_EMPTY: &str = "    ";

/// Format the execution mode as a colored badge
pub fn format_mode_badge(mode: &TaskGroupMode) -> String {
    match mode {
        TaskGroupMode::Workflow => "[WORKFLOW]".cyan().to_string(),
        TaskGroupMode::Sequential => "[SEQUENTIAL]".yellow().to_string(),
        TaskGroupMode::Parallel => "[PARALLEL]".green().to_string(),
        TaskGroupMode::Group => "[GROUP]".dark_grey().to_string(),
    }
}

/// Count tasks recursively in a node
pub fn count_tasks(node: &TaskNode) -> usize {
    match node {
        TaskNode::Task(_) => 1,
        TaskNode::Group { tasks, .. } => tasks.values().map(count_tasks).sum(),
    }
}

/// Display task nodes in a tree format
pub fn display_task_tree(nodes: &HashMap<String, TaskNode>, verbose: bool, use_color: bool) {
    // Sort for consistent display
    let sorted: BTreeMap<_, _> = nodes.iter().collect();

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
                mode,
                tasks,
            } => {
                if verbose {
                    // Verbose mode: show tree structure
                    display_group(
                        name, None, // Don't show descriptions in list view
                        mode, tasks, verbose, use_color, 0, "",
                    );
                } else {
                    // Compact mode: single line per group
                    display_group_compact(name, mode, tasks, use_color);
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
                "{} workflow  {} sequential  {} parallel  {} group  {} single",
                "⚡".cyan(),
                "⇢".yellow(),
                "⇉".green(),
                "◉".white(),
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
fn display_group_compact(
    name: &str,
    mode: &TaskGroupMode,
    tasks: &HashMap<String, TaskNode>,
    use_color: bool,
) {
    // Get list of subtask names (just the last part after dots)
    let mut subtask_names: Vec<String> = tasks
        .keys()
        .map(|k| {
            // Get just the last part of the name for cleaner display
            if let Some(last_dot) = k.rfind('.') {
                k[last_dot + 1..].to_string()
            } else {
                k.clone()
            }
        })
        .collect();
    subtask_names.sort();

    // Create task list display
    let task_list = if subtask_names.len() > 4 {
        format!("{} …", subtask_names[..4].join(" "))
    } else {
        subtask_names.join(" ")
    };

    if use_color {
        // Distinct icons for each mode
        let (symbol, color) = match mode {
            TaskGroupMode::Workflow => ("⚡", name.cyan()),
            TaskGroupMode::Sequential => ("⇢", name.yellow()),
            TaskGroupMode::Parallel => ("⇉", name.green()),
            TaskGroupMode::Group => ("◉", name.white()),
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

/// Display a group and its contents
#[allow(clippy::too_many_arguments)]
fn display_group(
    name: &str,
    description: Option<&str>,
    mode: &TaskGroupMode,
    tasks: &HashMap<String, TaskNode>,
    verbose: bool,
    use_color: bool,
    depth: usize,
    prefix: &str,
) {
    let task_count = tasks.values().map(count_tasks).sum::<usize>();
    let mode_badge = if use_color {
        format_mode_badge(mode)
    } else {
        format!(
            "[{}]",
            match mode {
                TaskGroupMode::Workflow => "WORKFLOW",
                TaskGroupMode::Sequential => "SEQUENTIAL",
                TaskGroupMode::Parallel => "PARALLEL",
                TaskGroupMode::Group => "GROUP",
            }
        )
    };

    // Display group header
    let group_line = if use_color {
        format!(
            "{}{} {} {}",
            prefix,
            name.bold().cyan(),
            mode_badge,
            format!("({task_count} tasks)").dark_grey()
        )
    } else {
        format!("{prefix}{name} {mode_badge} ({task_count} tasks)")
    };
    println!("{group_line}");

    // Display description if verbose
    if verbose {
        if let Some(desc) = description {
            let desc_line = if use_color {
                format!("{}  {}", prefix, desc.dark_grey())
            } else {
                format!("{prefix}  {desc}")
            };
            println!("{desc_line}");
        }
    }

    // Display child items with tree structure
    display_tree_children(tasks, verbose, use_color, depth);
}

/// Helper function to display tree children with a specific prefix
fn display_tree_children_with_prefix(
    tasks: &HashMap<String, TaskNode>,
    verbose: bool,
    use_color: bool,
    _depth: usize,
    prefix: &str,
) {
    let sorted: BTreeMap<_, _> = tasks.iter().collect();
    let total = sorted.len();
    let mut count = 0;

    for (name, node) in sorted {
        count += 1;
        let is_last = count == total;
        let connector = if is_last {
            format!("{prefix}{TREE_LAST}")
        } else {
            format!("{prefix}{TREE_BRANCH}")
        };

        match node {
            TaskNode::Task(config) => {
                let task_line = format_task_line(
                    name,
                    config.description.as_deref(),
                    &connector,
                    verbose,
                    use_color,
                );
                println!("{task_line}");
            }
            TaskNode::Group { .. } => {
                // For nested groups, just show a placeholder for now
                println!("{connector}{name} [...]");
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

/// Display children with tree structure
fn display_tree_children(
    nodes: &HashMap<String, TaskNode>,
    verbose: bool,
    use_color: bool,
    depth: usize,
) {
    let sorted: BTreeMap<_, _> = nodes.iter().collect();
    let total = sorted.len();
    let mut count = 0;

    for (name, node) in sorted {
        count += 1;
        let is_last = count == total;
        let connector = if is_last { TREE_LAST } else { TREE_BRANCH };
        let child_prefix = if is_last { TREE_EMPTY } else { TREE_PIPE };

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
            TaskNode::Group {
                description,
                mode,
                tasks,
            } => {
                display_group(
                    name,
                    description.as_deref(),
                    mode,
                    tasks,
                    verbose,
                    use_color,
                    depth + 1,
                    connector,
                );

                // Recursively display nested content with proper indentation
                if !tasks.is_empty() {
                    display_tree_children_with_prefix(
                        tasks,
                        verbose,
                        use_color,
                        depth + 1,
                        child_prefix,
                    );
                }
            }
        }
    }
}

/// Display a single group's contents when listing a specific group
pub fn display_group_contents(
    group_name: &str,
    description: Option<&str>,
    mode: &TaskGroupMode,
    tasks: &HashMap<String, TaskNode>,
    verbose: bool,
    use_color: bool,
) {
    let mode_badge = if use_color {
        format_mode_badge(mode)
    } else {
        format!(
            "[{}]",
            match mode {
                TaskGroupMode::Workflow => "WORKFLOW",
                TaskGroupMode::Sequential => "SEQUENTIAL",
                TaskGroupMode::Parallel => "PARALLEL",
                TaskGroupMode::Group => "GROUP",
            }
        )
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
    display_tree_children(tasks, verbose, use_color, 0);

    // Action hint
    println!();
    let action_hint = match mode {
        TaskGroupMode::Group => {
            format!("Run 'cuenv task {group_name} <task>' to execute a specific task")
        }
        TaskGroupMode::Parallel => {
            format!("Run 'cuenv task {group_name}' to execute all tasks in parallel")
        }
        TaskGroupMode::Sequential => {
            format!("Run 'cuenv task {group_name}' to execute all tasks sequentially")
        }
        TaskGroupMode::Workflow => {
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
    use cuenv_config::{TaskConfig, TaskGroupMode, TaskNode};
    use std::collections::HashMap;

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

    fn create_test_group(
        mode: TaskGroupMode,
        description: Option<String>,
        tasks: HashMap<String, TaskNode>,
    ) -> TaskNode {
        TaskNode::Group {
            description,
            mode,
            tasks,
        }
    }

    #[test]
    fn test_format_mode_badge() {
        assert_eq!(
            format_mode_badge(&TaskGroupMode::Workflow),
            "[WORKFLOW]".cyan().to_string()
        );
        assert_eq!(
            format_mode_badge(&TaskGroupMode::Sequential),
            "[SEQUENTIAL]".yellow().to_string()
        );
        assert_eq!(
            format_mode_badge(&TaskGroupMode::Parallel),
            "[PARALLEL]".green().to_string()
        );
        assert_eq!(
            format_mode_badge(&TaskGroupMode::Group),
            "[GROUP]".dark_grey().to_string()
        );
    }

    #[test]
    fn test_count_tasks_single_task() {
        let task = create_test_task(None);
        assert_eq!(count_tasks(&task), 1);
    }

    #[test]
    fn test_count_tasks_empty_group() {
        let group = create_test_group(TaskGroupMode::Group, None, HashMap::new());
        assert_eq!(count_tasks(&group), 0);
    }

    #[test]
    fn test_count_tasks_group_with_tasks() {
        let mut tasks = HashMap::new();
        tasks.insert("task1".to_string(), create_test_task(None));
        tasks.insert("task2".to_string(), create_test_task(None));

        let group = create_test_group(TaskGroupMode::Group, None, tasks);
        assert_eq!(count_tasks(&group), 2);
    }

    #[test]
    fn test_count_tasks_nested_groups() {
        let mut inner_tasks = HashMap::new();
        inner_tasks.insert("inner1".to_string(), create_test_task(None));
        inner_tasks.insert("inner2".to_string(), create_test_task(None));
        let inner_group = create_test_group(TaskGroupMode::Group, None, inner_tasks);

        let mut outer_tasks = HashMap::new();
        outer_tasks.insert("task1".to_string(), create_test_task(None));
        outer_tasks.insert("group1".to_string(), inner_group);

        let outer_group = create_test_group(TaskGroupMode::Group, None, outer_tasks);
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
        let tasks = HashMap::new();
        // This should not panic and should handle empty input gracefully
        display_task_tree(&tasks, false, false);
        display_task_tree(&tasks, true, false);
        display_task_tree(&tasks, false, true);
        display_task_tree(&tasks, true, true);
    }

    #[test]
    fn test_display_task_tree_single_task() {
        let mut tasks = HashMap::new();
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
        let mut tasks = HashMap::new();
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
        let mut inner_tasks = HashMap::new();
        inner_tasks.insert(
            "subtask1".to_string(),
            create_test_task(Some("Sub task 1".to_string())),
        );
        inner_tasks.insert(
            "subtask2".to_string(),
            create_test_task(Some("Sub task 2".to_string())),
        );

        let mut tasks = HashMap::new();
        tasks.insert(
            "single_task".to_string(),
            create_test_task(Some("A standalone task".to_string())),
        );
        tasks.insert(
            "group1".to_string(),
            create_test_group(
                TaskGroupMode::Parallel,
                Some("A parallel group".to_string()),
                inner_tasks,
            ),
        );

        display_task_tree(&tasks, false, false);
        display_task_tree(&tasks, true, true);
    }

    #[test]
    fn test_display_group_contents_all_modes() {
        let mut tasks = HashMap::new();
        tasks.insert(
            "task1".to_string(),
            create_test_task(Some("First task".to_string())),
        );
        tasks.insert(
            "task2".to_string(),
            create_test_task(Some("Second task".to_string())),
        );

        let modes = [
            TaskGroupMode::Workflow,
            TaskGroupMode::Sequential,
            TaskGroupMode::Parallel,
            TaskGroupMode::Group,
        ];

        for mode in &modes {
            display_group_contents(
                "test_group",
                Some("Test group description"),
                mode,
                &tasks,
                false,
                false,
            );

            display_group_contents(
                "test_group",
                Some("Test group description"),
                mode,
                &tasks,
                true,
                true,
            );
        }
    }

    #[test]
    fn test_display_group_contents_no_description() {
        let mut tasks = HashMap::new();
        tasks.insert("task1".to_string(), create_test_task(None));

        display_group_contents(
            "test_group",
            None,
            &TaskGroupMode::Group,
            &tasks,
            false,
            false,
        );
    }

    #[test]
    fn test_display_group_contents_empty_tasks() {
        let tasks = HashMap::new();

        display_group_contents(
            "empty_group",
            Some("An empty group"),
            &TaskGroupMode::Group,
            &tasks,
            true,
            true,
        );
    }

    #[test]
    fn test_display_group_contents_unicode_names() {
        let mut tasks = HashMap::new();
        tasks.insert(
            "测试任务".to_string(),
            create_test_task(Some("Unicode任务描述".to_string())),
        );

        display_group_contents(
            "Unicode组",
            Some("Unicode组描述"),
            &TaskGroupMode::Parallel,
            &tasks,
            true,
            true,
        );
    }

    #[test]
    fn test_display_group_compact_mode() {
        let mut tasks = HashMap::new();
        for i in 1..=6 {
            tasks.insert(
                format!("task{i}"),
                create_test_task(Some(format!("Task {i} description"))),
            );
        }

        // This tests the "4 tasks shown, others as …" functionality
        let group = create_test_group(
            TaskGroupMode::Parallel,
            Some("A group with many tasks".to_string()),
            tasks,
        );

        let mut nodes = HashMap::new();
        nodes.insert("big_group".to_string(), group);

        display_task_tree(&nodes, false, false);
        display_task_tree(&nodes, false, true);
    }

    #[test]
    fn test_display_with_dotted_task_names() {
        let mut tasks = HashMap::new();
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
            TaskGroupMode::Group,
            Some("Group with namespaced tasks".to_string()),
            tasks,
        );

        let mut nodes = HashMap::new();
        nodes.insert("namespaced_group".to_string(), group);

        display_task_tree(&nodes, false, false);
        display_task_tree(&nodes, true, true);
    }

    #[test]
    fn test_mixed_task_and_group_nodes() {
        let mut inner_tasks = HashMap::new();
        inner_tasks.insert(
            "inner1".to_string(),
            create_test_task(Some("Inner task 1".to_string())),
        );
        inner_tasks.insert(
            "inner2".to_string(),
            create_test_task(Some("Inner task 2".to_string())),
        );

        let mut nodes = HashMap::new();
        nodes.insert(
            "standalone_task".to_string(),
            create_test_task(Some("A standalone task".to_string())),
        );
        nodes.insert(
            "workflow_group".to_string(),
            create_test_group(
                TaskGroupMode::Workflow,
                Some("A workflow group".to_string()),
                inner_tasks.clone(),
            ),
        );
        nodes.insert(
            "sequential_group".to_string(),
            create_test_group(
                TaskGroupMode::Sequential,
                Some("A sequential group".to_string()),
                inner_tasks,
            ),
        );
        nodes.insert("another_task".to_string(), create_test_task(None));

        display_task_tree(&nodes, false, false);
        display_task_tree(&nodes, true, true);
    }

    #[test]
    fn test_error_handling_edge_cases() {
        // Test with very large numbers of tasks
        let mut tasks = HashMap::new();
        for i in 0..1000 {
            tasks.insert(
                format!("task_{i:04}"),
                create_test_task(Some(format!("Generated task {i}"))),
            );
        }

        let group = create_test_group(
            TaskGroupMode::Parallel,
            Some("Large group".to_string()),
            tasks,
        );

        let mut nodes = HashMap::new();
        nodes.insert("large_group".to_string(), group);

        // Should handle large numbers without issues
        display_task_tree(&nodes, false, false);
    }
}
