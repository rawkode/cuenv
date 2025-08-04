use crate::tracing::task_span::{TaskSpan, TaskState};
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
                    format!("{}  ", prefix)
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
                TaskState::Waiting => format!("\x1b[90m{}\x1b[0m", symbol), // Gray
                TaskState::Running { .. } => format!("\x1b[33m{}\x1b[0m", symbol), // Yellow
                TaskState::Completed { .. } => format!("\x1b[32m{}\x1b[0m", symbol), // Green
                TaskState::Failed { .. } => format!("\x1b[31m{}\x1b[0m", symbol), // Red
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
                    line.push_str(&format!("\x1b[90m{}\x1b[0m", duration_str)); // Gray
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
                line.push_str(&format!("\x1b[90m({})\x1b[0m", target)); // Gray
            } else {
                line.push_str(&format!("({})", target));
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

        summary.push_str(&format!("[{}/{} complete", completed_tasks, total_tasks));

        if running_tasks > 0 {
            summary.push_str(&format!(", {} running", running_tasks));
        }

        if failed_tasks > 0 {
            if self.config.use_colors {
                summary.push_str(&format!(", \x1b[31m{} failed\x1b[0m", failed_tasks));
            } else {
                summary.push_str(&format!(", {} failed", failed_tasks));
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
