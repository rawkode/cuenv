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
