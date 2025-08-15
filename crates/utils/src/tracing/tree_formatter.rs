use super::task_span::{TaskSpan, TaskState};
use crossterm::terminal;
use std::collections::HashMap;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Tree formatting configuration
pub struct TreeFormatterConfig {
    /// Use Unicode characters for tree drawing
    pub use_unicode: bool,
    /// Use colors in output
    pub use_colors: bool,
    /// Maximum width for progress bars
    pub progress_bar_width: usize,
    /// Indentation per tree level
    pub indent_width: usize,
}

impl Default for TreeFormatterConfig {
    fn default() -> Self {
        Self {
            use_unicode: supports_unicode(),
            use_colors: supports_colors(),
            progress_bar_width: 20,
            indent_width: 2,
        }
    }
}

/// Tree drawing characters
pub struct TreeChars {
    /// Vertical line
    pub vertical: &'static str,
    /// Horizontal line
    pub horizontal: &'static str,
    /// Tree branch (├─)
    pub branch: &'static str,
    /// Last branch (└─)
    pub last_branch: &'static str,
    /// Expanded node (▼)
    pub expanded: &'static str,
    /// Collapsed node (▶)
    pub collapsed: &'static str,
}

impl TreeChars {
    /// Unicode tree characters
    pub fn unicode() -> Self {
        Self {
            vertical: "│",
            horizontal: "─",
            branch: "├─",
            last_branch: "└─",
            expanded: "▼",
            collapsed: "▶",
        }
    }

    /// ASCII fallback characters
    pub fn ascii() -> Self {
        Self {
            vertical: "|",
            horizontal: "-",
            branch: "|-",
            last_branch: "`-",
            expanded: "v",
            collapsed: ">",
        }
    }
}

/// Formats task execution as a tree view
pub struct TreeFormatter {
    config: TreeFormatterConfig,
    chars: TreeChars,
}

impl TreeFormatter {
    /// Create a new tree formatter with default configuration
    pub fn new() -> Self {
        let config = TreeFormatterConfig::default();
        let chars = if config.use_unicode {
            TreeChars::unicode()
        } else {
            TreeChars::ascii()
        };

        Self { config, chars }
    }

    /// Create a new tree formatter with custom configuration
    pub fn with_config(config: TreeFormatterConfig) -> Self {
        let chars = if config.use_unicode {
            TreeChars::unicode()
        } else {
            TreeChars::ascii()
        };

        Self { config, chars }
    }

    /// Format the entire task tree
    pub fn format_tree(&self, tasks: &HashMap<u64, TaskSpan>) -> String {
        let mut output = String::new();

        // Find root tasks (tasks without parents)
        let root_tasks: Vec<_> = tasks
            .iter()
            .filter(|(_, task)| task.parent_id.is_none())
            .collect();

        // Sort root tasks by name for consistent output
        let mut sorted_roots = root_tasks;
        sorted_roots.sort_by(|(_, a), (_, b)| a.name.cmp(&b.name));

        for (i, (task_id, _)) in sorted_roots.iter().enumerate() {
            let is_last = i == sorted_roots.len() - 1;
            self.format_task_recursive(&mut output, tasks, **task_id, String::new(), is_last);
        }

        output
    }

    /// Recursively format a task and its children
    fn format_task_recursive(
        &self,
        output: &mut String,
        tasks: &HashMap<u64, TaskSpan>,
        task_id: u64,
        prefix: String,
        is_last: bool,
    ) {
        if let Some(task) = tasks.get(&task_id) {
            // Format the current task
            let task_line = self.format_task_line(task, &prefix, is_last);
            output.push_str(&task_line);
            output.push('\n');

            // Format children
            if !task.children.is_empty() {
                let child_prefix = if is_last {
                    format!("{prefix}  ")
                } else {
                    format!("{}{} ", prefix, self.chars.vertical)
                };

                let mut sorted_children = task.children.clone();
                sorted_children.sort_by(|a, b| {
                    let empty_string = String::new();
                    let name_a = tasks.get(a).map(|t| &t.name).unwrap_or(&empty_string);
                    let name_b = tasks.get(b).map(|t| &t.name).unwrap_or(&empty_string);
                    name_a.cmp(name_b)
                });

                for (i, child_id) in sorted_children.iter().enumerate() {
                    let is_last_child = i == sorted_children.len() - 1;
                    self.format_task_recursive(
                        output,
                        tasks,
                        *child_id,
                        child_prefix.clone(),
                        is_last_child,
                    );
                }
            }
        }
    }

    /// Format a single task line
    fn format_task_line(&self, task: &TaskSpan, prefix: &str, is_last: bool) -> String {
        let mut line = String::new();

        // Add prefix and tree characters
        line.push_str(prefix);
        if is_last {
            line.push_str(self.chars.last_branch);
        } else {
            line.push_str(self.chars.branch);
        }
        line.push(' ');

        // Add task state symbol
        let symbol = if self.config.use_unicode {
            task.state.symbol()
        } else {
            task.state.symbol_ascii()
        };

        if self.config.use_colors {
            let colored_symbol = match task.state {
                TaskState::Waiting => format!("\x1b[90m{symbol}\x1b[0m"), // Gray
                TaskState::Running { .. } => format!("\x1b[33m{symbol}\x1b[0m"), // Yellow
                TaskState::Completed { .. } => format!("\x1b[32m{symbol}\x1b[0m"), // Green
                TaskState::Failed { .. } => format!("\x1b[31m{symbol}\x1b[0m"), // Red
            };
            line.push_str(&colored_symbol);
        } else {
            line.push_str(symbol);
        }
        line.push(' ');

        // Add task name
        if self.config.use_colors {
            match task.state {
                TaskState::Failed { .. } => {
                    line.push_str(&format!("\x1b[31m{}\x1b[0m", task.name)); // Red
                }
                TaskState::Completed { .. } => {
                    line.push_str(&format!("\x1b[32m{}\x1b[0m", task.name)); // Green
                }
                _ => {
                    line.push_str(&task.name);
                }
            }
        } else {
            line.push_str(&task.name);
        }

        // Add progress bar if task is running and has progress
        if let TaskState::Running { .. } = task.state {
            if task.progress.is_some() {
                line.push(' ');
                let progress_bar = if self.config.use_unicode {
                    task.progress_bar(self.config.progress_bar_width)
                } else {
                    task.progress_bar_ascii(self.config.progress_bar_width)
                };
                line.push_str(&progress_bar);
            }
        }

        // Add duration for completed/failed tasks
        if task.state.is_terminal() || task.state.is_running() {
            let duration_str = task.duration_string();
            if !duration_str.is_empty() {
                line.push_str(" [");
                if self.config.use_colors {
                    line.push_str(&format!("\x1b[90m{duration_str}\x1b[0m")); // Gray
                } else {
                    line.push_str(&duration_str);
                }
                line.push(']');
            }
        }

        // Add target information if available
        if let Some(target) = &task.target {
            line.push(' ');
            if self.config.use_colors {
                line.push_str(&format!("\x1b[90m({target})\x1b[0m")); // Gray
            } else {
                line.push_str(&format!("({target})"));
            }
        }

        line
    }

    /// Format a summary line showing overall progress
    pub fn format_summary(&self, tasks: &HashMap<u64, TaskSpan>) -> String {
        let total_tasks = tasks.len();
        let completed_tasks = tasks.values().filter(|t| t.state.is_terminal()).count();
        let running_tasks = tasks.values().filter(|t| t.state.is_running()).count();
        let failed_tasks = tasks
            .values()
            .filter(|t| matches!(t.state, TaskState::Failed { .. }))
            .count();

        let mut summary = String::new();

        if self.config.use_unicode {
            summary.push_str("▼ Pipeline ");
        } else {
            summary.push_str("v Pipeline ");
        }

        summary.push_str(&format!("[{completed_tasks}/{total_tasks} complete"));

        if running_tasks > 0 {
            summary.push_str(&format!(", {running_tasks} running"));
        }

        if failed_tasks > 0 {
            if self.config.use_colors {
                summary.push_str(&format!(", \x1b[31m{failed_tasks} failed\x1b[0m"));
            } else {
                summary.push_str(&format!(", {failed_tasks} failed"));
            }
        }

        summary.push(']');
        summary
    }

    /// Get the maximum line width for proper terminal handling
    pub fn get_terminal_width(&self) -> usize {
        terminal::size().map(|(w, _)| w as usize).unwrap_or(80)
    }

    /// Truncate a line to fit terminal width
    pub fn truncate_line(&self, line: &str, max_width: usize) -> String {
        let width = UnicodeWidthStr::width(line);
        if width <= max_width {
            line.to_string()
        } else {
            // Truncate and add ellipsis
            let mut truncated = String::new();
            let mut current_width = 0;

            for ch in line.chars() {
                let ch_width = ch.width_cjk().unwrap_or(0);
                if current_width + ch_width + 3 > max_width {
                    // Reserve space for "..."
                    break;
                }
                truncated.push(ch);
                current_width += ch_width;
            }

            if self.config.use_unicode {
                truncated.push('…');
            } else {
                truncated.push_str("...");
            }

            truncated
        }
    }
}

impl Default for TreeFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if the terminal supports Unicode
fn supports_unicode() -> bool {
    // Check common environment variables that indicate Unicode support
    std::env::var("TERM")
        .map(|term| !term.contains("xterm-color") && !term.contains("screen-256color-bce"))
        .unwrap_or(true)
        && std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LC_CTYPE"))
            .or_else(|_| std::env::var("LANG"))
            .map(|locale| locale.contains("UTF-8") || locale.contains("utf8"))
            .unwrap_or(true)
}

/// Check if the terminal supports colors
fn supports_colors() -> bool {
    // Check if we're in a TTY and common color environment variables
    std::io::IsTerminal::is_terminal(&std::io::stderr())
        && std::env::var("NO_COLOR").is_err()
        && std::env::var("TERM")
            .map(|term| term != "dumb")
            .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Instant;

    fn create_sample_task(name: &str, state: TaskState) -> TaskSpan {
        TaskSpan {
            name: name.to_string(),
            state,
            progress: None,
            parent_id: None,
            children: Vec::new(),
            target: None,
            metadata: HashMap::new(),
        }
    }

    fn create_task_with_children(name: &str, state: TaskState, children: Vec<u64>) -> TaskSpan {
        TaskSpan {
            name: name.to_string(),
            state,
            progress: None,
            parent_id: None,
            children,
            target: None,
            metadata: HashMap::new(),
        }
    }

    fn create_child_task(name: &str, state: TaskState, parent_id: u64) -> TaskSpan {
        TaskSpan {
            name: name.to_string(),
            state,
            progress: None,
            parent_id: Some(parent_id),
            children: Vec::new(),
            target: None,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_tree_formatter_config_default() {
        let config = TreeFormatterConfig::default();
        assert_eq!(config.progress_bar_width, 20);
        assert_eq!(config.indent_width, 2);
    }

    #[test]
    fn test_tree_chars_unicode() {
        let chars = TreeChars::unicode();
        assert_eq!(chars.vertical, "│");
        assert_eq!(chars.horizontal, "─");
        assert_eq!(chars.branch, "├─");
        assert_eq!(chars.last_branch, "└─");
        assert_eq!(chars.expanded, "▼");
        assert_eq!(chars.collapsed, "▶");
    }

    #[test]
    fn test_tree_chars_ascii() {
        let chars = TreeChars::ascii();
        assert_eq!(chars.vertical, "|");
        assert_eq!(chars.horizontal, "-");
        assert_eq!(chars.branch, "|-");
        assert_eq!(chars.last_branch, "`-");
        assert_eq!(chars.expanded, "v");
        assert_eq!(chars.collapsed, ">");
    }

    #[test]
    fn test_tree_formatter_new() {
        let formatter = TreeFormatter::new();
        // Default should have default config
        let default_config = TreeFormatterConfig::default();
        assert_eq!(
            formatter.config.progress_bar_width,
            default_config.progress_bar_width
        );
        assert_eq!(formatter.config.indent_width, default_config.indent_width);
    }

    #[test]
    fn test_tree_formatter_with_config() {
        let config = TreeFormatterConfig {
            use_unicode: false,
            use_colors: false,
            progress_bar_width: 30,
            indent_width: 4,
        };
        let formatter = TreeFormatter::with_config(config);
        assert_eq!(formatter.config.progress_bar_width, 30);
        assert_eq!(formatter.config.indent_width, 4);
        assert!(!formatter.config.use_unicode);
        assert!(!formatter.config.use_colors);
    }

    #[test]
    fn test_format_empty_tree() {
        let formatter = TreeFormatter::new();
        let tasks = HashMap::new();
        let output = formatter.format_tree(&tasks);
        assert_eq!(output, "");
    }

    #[test]
    fn test_format_single_task() {
        let formatter = TreeFormatter::new();
        let mut tasks = HashMap::new();

        let task = create_sample_task("test_task", TaskState::Waiting);
        tasks.insert(1, task);

        let output = formatter.format_tree(&tasks);
        assert!(output.contains("test_task"));
        assert!(output.contains("└─")); // Last branch for single task
    }

    #[test]
    fn test_format_multiple_root_tasks() {
        let formatter = TreeFormatter::new();
        let mut tasks = HashMap::new();

        // Tasks are sorted by name, so "build" comes before "test"
        tasks.insert(1, create_sample_task("build", TaskState::Waiting));
        tasks.insert(2, create_sample_task("test", TaskState::Waiting));

        let output = formatter.format_tree(&tasks);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("build"));
        assert!(lines[1].contains("test"));
        // First task should use branch, last should use last_branch
        assert!(lines[0].contains("├─"));
        assert!(lines[1].contains("└─"));
    }

    #[test]
    fn test_format_nested_tasks() {
        let formatter = TreeFormatter::new();
        let mut tasks = HashMap::new();

        // Parent task with children
        let parent = create_task_with_children(
            "parent",
            TaskState::Running {
                started_at: Instant::now(),
            },
            vec![2, 3],
        );
        tasks.insert(1, parent);

        // Child tasks
        tasks.insert(
            2,
            create_child_task(
                "child1",
                TaskState::Completed {
                    started_at: Instant::now(),
                    completed_at: Instant::now(),
                },
                1,
            ),
        );
        tasks.insert(
            3,
            create_child_task(
                "child2",
                TaskState::Failed {
                    started_at: Instant::now(),
                    failed_at: Instant::now(),
                    error: "test error".to_string(),
                },
                1,
            ),
        );

        let output = formatter.format_tree(&tasks);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 3);

        // Parent should be first
        assert!(lines[0].contains("parent"));
        assert!(lines[0].contains("└─")); // Last root task

        // Children should be indented
        assert!(lines[1].contains("child1"));
        assert!(lines[2].contains("child2"));
        assert!(lines[1].contains("├─")); // First child
        assert!(lines[2].contains("└─")); // Last child
    }

    #[test]
    fn test_format_task_line_with_different_states() {
        let formatter = TreeFormatter::with_config(TreeFormatterConfig {
            use_unicode: true,
            use_colors: false,
            progress_bar_width: 20,
            indent_width: 2,
        });

        // Test waiting state
        let waiting_task = create_sample_task("waiting", TaskState::Waiting);
        let line = formatter.format_task_line(&waiting_task, "", true);
        assert!(line.contains("◯")); // Unicode waiting symbol
        assert!(line.contains("waiting"));

        // Test running state
        let running_task = create_sample_task(
            "running",
            TaskState::Running {
                started_at: Instant::now(),
            },
        );
        let line = formatter.format_task_line(&running_task, "", true);
        assert!(line.contains("⟳")); // Unicode running symbol

        // Test completed state
        let completed_task = create_sample_task(
            "completed",
            TaskState::Completed {
                started_at: Instant::now(),
                completed_at: Instant::now(),
            },
        );
        let line = formatter.format_task_line(&completed_task, "", true);
        assert!(line.contains("✓")); // Unicode completed symbol

        // Test failed state
        let failed_task = create_sample_task(
            "failed",
            TaskState::Failed {
                started_at: Instant::now(),
                failed_at: Instant::now(),
                error: "test error".to_string(),
            },
        );
        let line = formatter.format_task_line(&failed_task, "", true);
        assert!(line.contains("✗")); // Unicode failed symbol
    }

    #[test]
    fn test_format_task_line_ascii_fallback() {
        let formatter = TreeFormatter::with_config(TreeFormatterConfig {
            use_unicode: false,
            use_colors: false,
            progress_bar_width: 20,
            indent_width: 2,
        });

        // Test with ASCII symbols
        let waiting_task = create_sample_task("waiting", TaskState::Waiting);
        let line = formatter.format_task_line(&waiting_task, "", true);
        assert!(line.contains("o")); // ASCII waiting symbol
        assert!(line.contains("`-")); // ASCII last branch
    }

    #[test]
    fn test_format_task_line_with_progress() {
        let formatter = TreeFormatter::with_config(TreeFormatterConfig {
            use_unicode: true,
            use_colors: false,
            progress_bar_width: 10,
            indent_width: 2,
        });

        let mut task = create_sample_task(
            "with_progress",
            TaskState::Running {
                started_at: Instant::now(),
            },
        );
        task.progress = Some(50);

        let line = formatter.format_task_line(&task, "", true);
        assert!(line.contains("with_progress"));
        assert!(line.contains("█")); // Progress bar should be present
        assert!(line.contains("░")); // Empty part of progress bar
    }

    #[test]
    fn test_format_task_line_with_target() {
        let formatter = TreeFormatter::with_config(TreeFormatterConfig {
            use_unicode: true,
            use_colors: false,
            progress_bar_width: 20,
            indent_width: 2,
        });

        let mut task = create_sample_task("with_target", TaskState::Waiting);
        task.target = Some("src/main.rs".to_string());

        let line = formatter.format_task_line(&task, "", true);
        assert!(line.contains("with_target"));
        assert!(line.contains("(src/main.rs)"));
    }

    #[test]
    fn test_format_summary() {
        let formatter = TreeFormatter::new();
        let mut tasks = HashMap::new();

        tasks.insert(
            1,
            create_sample_task(
                "completed",
                TaskState::Completed {
                    started_at: Instant::now(),
                    completed_at: Instant::now(),
                },
            ),
        );
        tasks.insert(
            2,
            create_sample_task(
                "running",
                TaskState::Running {
                    started_at: Instant::now(),
                },
            ),
        );
        tasks.insert(
            3,
            create_sample_task(
                "failed",
                TaskState::Failed {
                    started_at: Instant::now(),
                    failed_at: Instant::now(),
                    error: "test error".to_string(),
                },
            ),
        );
        tasks.insert(4, create_sample_task("waiting", TaskState::Waiting));

        let summary = formatter.format_summary(&tasks);
        assert!(summary.contains("Pipeline"));
        // Check for completed tasks - TaskState::Completed is terminal
        assert!(summary.contains("complete"));
        assert!(summary.contains("1 running"));
        assert!(summary.contains("1 failed"));
    }

    #[test]
    fn test_format_summary_no_colors() {
        let formatter = TreeFormatter::with_config(TreeFormatterConfig {
            use_unicode: false,
            use_colors: false,
            progress_bar_width: 20,
            indent_width: 2,
        });

        let mut tasks = HashMap::new();
        tasks.insert(
            1,
            create_sample_task(
                "failed",
                TaskState::Failed {
                    started_at: Instant::now(),
                    failed_at: Instant::now(),
                    error: "test error".to_string(),
                },
            ),
        );

        let summary = formatter.format_summary(&tasks);
        assert!(summary.contains("v Pipeline")); // ASCII fallback
        assert!(summary.contains("1 failed"));
        // Should not contain ANSI color codes when colors are disabled
        assert!(!summary.contains("\x1b["));
    }

    #[test]
    fn test_get_terminal_width() {
        let formatter = TreeFormatter::new();
        let width = formatter.get_terminal_width();
        // Should return a reasonable default if terminal size cannot be determined
        assert!(width >= 80);
    }

    #[test]
    fn test_truncate_line_no_truncation_needed() {
        let formatter = TreeFormatter::new();
        let line = "short line";
        let truncated = formatter.truncate_line(line, 100);
        assert_eq!(truncated, line);
    }

    #[test]
    fn test_truncate_line_with_truncation() {
        let formatter = TreeFormatter::with_config(TreeFormatterConfig {
            use_unicode: true,
            use_colors: false,
            progress_bar_width: 20,
            indent_width: 2,
        });

        let long_line = "this is a very long line that definitely needs truncation";
        let truncated = formatter.truncate_line(long_line, 20);
        assert!(truncated.len() <= 20);
        assert!(truncated.ends_with('…')); // Unicode ellipsis
    }

    #[test]
    fn test_truncate_line_with_truncation_ascii() {
        let formatter = TreeFormatter::with_config(TreeFormatterConfig {
            use_unicode: false,
            use_colors: false,
            progress_bar_width: 20,
            indent_width: 2,
        });

        let long_line = "this is a very long line that definitely needs truncation";
        let truncated = formatter.truncate_line(long_line, 20);
        assert!(truncated.len() <= 20);
        assert!(truncated.ends_with("...")); // ASCII ellipsis
    }

    #[test]
    fn test_truncate_line_with_unicode_characters() {
        let formatter = TreeFormatter::new();
        let unicode_line = "这是一个包含中文字符的测试行";
        let truncated = formatter.truncate_line(unicode_line, 10);
        // Should handle Unicode width correctly
        assert!(truncated.len() <= unicode_line.len());
    }

    #[test]
    fn test_deep_nesting_performance() {
        let formatter = TreeFormatter::new();
        let mut tasks: HashMap<u64, TaskSpan> = HashMap::new();

        // Create a deeply nested structure
        let mut current_parent = None;
        for i in 0..100 {
            let mut task = create_sample_task(&format!("task_{i}"), TaskState::Waiting);
            task.parent_id = current_parent;

            // Add child to parent
            if let Some(parent_id) = current_parent {
                if let Some(parent) = tasks.get_mut(&parent_id) {
                    parent.children.push(i);
                }
            }

            tasks.insert(i, task);
            current_parent = Some(i);
        }

        // This should complete without hanging or excessive memory usage
        let output = formatter.format_tree(&tasks);
        assert!(!output.is_empty());
        assert_eq!(output.lines().count(), 100);
    }

    #[test]
    fn test_concurrent_task_formatting() {
        use std::sync::Arc;
        use std::thread;

        let formatter = Arc::new(TreeFormatter::new());
        let mut tasks = HashMap::new();

        // Create tasks with different states
        for i in 0..10 {
            let state = match i % 4 {
                0 => TaskState::Waiting,
                1 => TaskState::Running {
                    started_at: Instant::now(),
                },
                2 => TaskState::Completed {
                    started_at: Instant::now(),
                    completed_at: Instant::now(),
                },
                _ => TaskState::Failed {
                    started_at: Instant::now(),
                    failed_at: Instant::now(),
                    error: "test".to_string(),
                },
            };
            tasks.insert(i as u64, create_sample_task(&format!("task_{i}"), state));
        }

        let tasks = Arc::new(tasks);
        let mut handles = vec![];

        // Spawn multiple threads to format the tree concurrently
        for _ in 0..5 {
            let formatter_clone = Arc::clone(&formatter);
            let tasks_clone = Arc::clone(&tasks);

            let handle = thread::spawn(move || {
                let output = formatter_clone.format_tree(&tasks_clone);
                assert!(!output.is_empty());
                output
            });
            handles.push(handle);
        }

        // All threads should complete successfully
        for handle in handles {
            let result = handle.join();
            assert!(result.is_ok());
            let output = result.unwrap();
            assert!(output.contains("task_"));
        }
    }

    #[test]
    fn test_malformed_task_data() {
        let formatter = TreeFormatter::new();
        let mut tasks = HashMap::new();

        // Task with missing children
        let mut parent = create_sample_task("parent", TaskState::Waiting);
        parent.children = vec![999]; // Non-existent child
        tasks.insert(1, parent);

        // Should handle gracefully without panicking
        let output = formatter.format_tree(&tasks);
        assert!(output.contains("parent"));
    }

    #[test]
    fn test_supports_unicode_function() {
        // Test the supports_unicode function with different environment setups
        // Note: This test doesn't modify actual env vars to avoid side effects
        let supports_unicode_result = supports_unicode();
        // Just verify it returns a boolean without panicking by calling the function
        let _ = supports_unicode_result;
    }

    #[test]
    fn test_supports_colors_function() {
        // Test the supports_colors function
        let supports_colors_result = supports_colors();
        // Just verify it returns a boolean without panicking by calling the function
        let _ = supports_colors_result;
    }

    #[test]
    fn test_tree_formatter_default_trait() {
        let formatter1 = TreeFormatter::default();
        let formatter2 = TreeFormatter::new();

        // Default should behave the same as new()
        assert_eq!(
            formatter1.config.progress_bar_width,
            formatter2.config.progress_bar_width
        );
        assert_eq!(
            formatter1.config.indent_width,
            formatter2.config.indent_width
        );
    }

    #[test]
    fn test_format_tree_with_colors_enabled() {
        let formatter = TreeFormatter::with_config(TreeFormatterConfig {
            use_unicode: true,
            use_colors: true,
            progress_bar_width: 20,
            indent_width: 2,
        });

        let mut tasks = HashMap::new();
        tasks.insert(
            1,
            create_sample_task(
                "completed",
                TaskState::Completed {
                    started_at: Instant::now(),
                    completed_at: Instant::now(),
                },
            ),
        );
        tasks.insert(
            2,
            create_sample_task(
                "failed",
                TaskState::Failed {
                    started_at: Instant::now(),
                    failed_at: Instant::now(),
                    error: "test error".to_string(),
                },
            ),
        );

        let output = formatter.format_tree(&tasks);
        // Should contain ANSI color codes when colors are enabled
        assert!(output.contains("\x1b[32m")); // Green for completed
        assert!(output.contains("\x1b[31m")); // Red for failed
        assert!(output.contains("\x1b[0m")); // Reset code
    }

    #[test]
    fn test_format_tree_with_emoji_and_unicode() {
        let formatter = TreeFormatter::new();
        let mut tasks = HashMap::new();

        // Task names with Unicode and emojis
        tasks.insert(
            1,
            create_sample_task(
                "🚀 deploy",
                TaskState::Running {
                    started_at: Instant::now(),
                },
            ),
        );
        tasks.insert(
            2,
            create_sample_task(
                "📦 build",
                TaskState::Completed {
                    started_at: Instant::now(),
                    completed_at: Instant::now(),
                },
            ),
        );
        tasks.insert(3, create_sample_task("测试 test", TaskState::Waiting));

        let output = formatter.format_tree(&tasks);
        assert!(output.contains("🚀 deploy"));
        assert!(output.contains("📦 build"));
        assert!(output.contains("测试 test"));

        // Should handle Unicode width calculations correctly
        assert!(!output.is_empty());
    }
}
