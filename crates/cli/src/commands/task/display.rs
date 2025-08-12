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
    let total = sorted.len();

    // Header
    if use_color {
        println!("{}", "Available Tasks:".bold());
        println!("{}", "═".repeat(50).dark_grey());
        println!();
    } else {
        println!("Available Tasks:");
        println!("{}", "=".repeat(50));
        println!();
    }

    // Display each top-level item
    let mut count = 0;
    let mut standalone_tasks = Vec::new();

    for (name, node) in sorted {
        count += 1;
        let is_last = count == total;

        match node {
            TaskNode::Task(config) => {
                // Collect standalone tasks to display at the end
                standalone_tasks.push((name.clone(), config.description.clone()));
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
                    0,
                    "",
                );

                // Add spacing between groups
                if !is_last {
                    println!();
                }
            }
        }
    }

    // Display standalone tasks if any
    if !standalone_tasks.is_empty() {
        if use_color {
            println!("{}", "─".repeat(50).dark_grey());
            println!("{}", "Standalone Tasks:".dark_grey());
        } else {
            println!("{}", "-".repeat(50));
            println!("Standalone Tasks:");
        }

        for (name, desc) in standalone_tasks {
            if verbose {
                if let Some(d) = desc {
                    println!("  • {} {}", name, format!("({})", d).dark_grey());
                } else {
                    println!("  • {}", name);
                }
            } else {
                println!("  • {}", name);
            }
        }
    }

    // Footer with usage hints
    println!();
    if use_color {
        println!("{}", "═".repeat(50).dark_grey());
        println!("{}", "Hints:".bold());
        println!(
            "  {} = Select a specific task to run",
            "[GROUP]".dark_grey()
        );
        println!(
            "  {} = Execute all tasks",
            "[PARALLEL/SEQUENTIAL/WORKFLOW]".green()
        );
        println!();
        println!(
            "{}: cuenv task <name> {}",
            "Usage".dark_grey(),
            "[args...]".dark_grey()
        );
    } else {
        println!("{}", "=".repeat(50));
        println!("Hints:");
        println!("  [GROUP] = Select a specific task to run");
        println!("  [PARALLEL/SEQUENTIAL/WORKFLOW] = Execute all tasks");
        println!();
        println!("Usage: cuenv task <name> [args...]");
    }
}

/// Display a group and its contents
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
            format!("({} tasks)", task_count).dark_grey()
        )
    } else {
        format!("{}{} {} ({} tasks)", prefix, name, mode_badge, task_count)
    };
    println!("{}", group_line);

    // Display description if verbose
    if verbose {
        if let Some(desc) = description {
            let desc_line = if use_color {
                format!("{}  {}", prefix, desc.dark_grey())
            } else {
                format!("{}  {}", prefix, desc)
            };
            println!("{}", desc_line);
        }
    }

    // Display child items with tree structure
    display_tree_children(tasks, verbose, use_color, depth);
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
                let task_line = if verbose && config.description.is_some() {
                    if use_color {
                        format!(
                            "{}{} {}",
                            connector,
                            name,
                            format!("– {}", config.description.as_ref().unwrap()).dark_grey()
                        )
                    } else {
                        format!(
                            "{}{} – {}",
                            connector,
                            name,
                            config.description.as_ref().unwrap()
                        )
                    }
                } else {
                    format!("{}{}", connector, name)
                };
                println!("{}", task_line);
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

                // Add continuation lines for nested content
                if !tasks.is_empty() {
                    for (inner_name, inner_node) in tasks.iter() {
                        let sorted_inner: BTreeMap<_, _> = tasks.iter().collect();
                        let is_inner_last = sorted_inner.keys().last() == Some(&inner_name);
                        let inner_connector = if is_inner_last {
                            format!("{}{}", child_prefix, TREE_LAST)
                        } else {
                            format!("{}{}", child_prefix, TREE_BRANCH)
                        };

                        match inner_node {
                            TaskNode::Task(config) => {
                                let task_line = if verbose && config.description.is_some() {
                                    if use_color {
                                        format!(
                                            "{}{} {}",
                                            inner_connector,
                                            inner_name,
                                            format!("– {}", config.description.as_ref().unwrap())
                                                .dark_grey()
                                        )
                                    } else {
                                        format!(
                                            "{}{} – {}",
                                            inner_connector,
                                            inner_name,
                                            config.description.as_ref().unwrap()
                                        )
                                    }
                                } else {
                                    format!("{}{}", inner_connector, inner_name)
                                };
                                println!("{}", task_line);
                            }
                            TaskNode::Group { .. } => {
                                // For nested groups, we'd need to recurse further
                                // For now, just show the name
                                println!("{}{} [...]", inner_connector, inner_name);
                            }
                        }
                    }
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
        println!("{} {}", group_name, mode_badge);
    }

    if let Some(desc) = description {
        if use_color {
            println!("  {}", desc.dark_grey());
        } else {
            println!("  {}", desc);
        }
    }

    println!();

    // Display tasks
    display_tree_children(tasks, verbose, use_color, 0);

    // Action hint
    println!();
    let action_hint = match mode {
        TaskGroupMode::Group => {
            format!(
                "Run 'cuenv task {} <task>' to execute a specific task",
                group_name
            )
        }
        TaskGroupMode::Parallel => {
            format!(
                "Run 'cuenv task {}' to execute all tasks in parallel",
                group_name
            )
        }
        TaskGroupMode::Sequential => {
            format!(
                "Run 'cuenv task {}' to execute all tasks sequentially",
                group_name
            )
        }
        TaskGroupMode::Workflow => {
            format!(
                "Run 'cuenv task {}' to execute tasks based on dependencies",
                group_name
            )
        }
    };

    if use_color {
        println!("{}", action_hint.dark_grey());
    } else {
        println!("{}", action_hint);
    }
}
