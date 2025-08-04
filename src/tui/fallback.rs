use crate::task_executor::TaskExecutionPlan;
use crate::tui::events::{TaskEvent, TaskRegistry, TaskState};
use chrono::Local;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Write};
use std::time::Instant;

/// Non-TTY fallback that outputs ASCII DAG and Chrome Trace JSON
pub struct FallbackRenderer {
    task_registry: TaskRegistry,
    start_time: Instant,
    output_path: Option<String>,
}

#[derive(Serialize)]
struct ChromeTraceEvent {
    name: String,
    cat: String,
    ph: String,
    ts: u64,
    dur: Option<u64>,
    pid: u32,
    tid: String,
    args: HashMap<String, serde_json::Value>,
}

impl FallbackRenderer {
    pub fn new(task_registry: TaskRegistry, output_path: Option<String>) -> Self {
        Self {
            task_registry,
            start_time: Instant::now(),
            output_path,
        }
    }

    /// Generate ASCII representation of the DAG
    pub async fn generate_ascii_dag(&self, plan: &TaskExecutionPlan) -> String {
        let mut output = String::new();
        output.push_str("Task Execution Plan\n");
        output.push_str("==================\n\n");

        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        output.push_str(&format!("Generated: {}\n", timestamp));
        output.push_str(&format!("Total tasks: {}\n", plan.tasks.len()));
        output.push_str(&format!("Execution levels: {}\n\n", plan.levels.len()));

        // Build dependency tree
        let tasks = self.task_registry.get_all_tasks().await;
        let root_tasks = self.find_root_tasks(&plan.tasks);

        output.push_str("Dependency Tree:\n");
        output.push_str("---------------\n");

        for root in &root_tasks {
            Self::render_task_tree(&mut output, root, &tasks, &plan.tasks, 0, "", true);
        }

        output.push_str("\n\nExecution Order:\n");
        output.push_str("---------------\n");

        for (level_idx, level_tasks) in plan.levels.iter().enumerate() {
            output.push_str(&format!("\nLevel {}: ", level_idx));

            let task_names: Vec<String> = level_tasks
                .iter()
                .map(|t| {
                    let state = tasks
                        .get(t)
                        .map(|info| &info.state)
                        .unwrap_or(&TaskState::Queued);
                    format!("{} {}", state.icon(), t)
                })
                .collect();

            output.push_str(&task_names.join(", "));
        }

        output.push_str("\n\nLegend:\n");
        output.push_str("------\n");
        output.push_str("◌ Queued  ▣ Running  ■ Completed  ✖ Failed  ⊘ Cancelled\n");

        output
    }

    fn find_root_tasks(
        &self,
        tasks: &HashMap<String, crate::cue_parser::TaskConfig>,
    ) -> Vec<String> {
        let mut roots = Vec::new();
        let all_deps: std::collections::HashSet<String> = tasks
            .values()
            .flat_map(|task| task.dependencies.clone().unwrap_or_default().into_iter())
            .collect();

        for task_name in tasks.keys() {
            if !all_deps.contains(task_name) {
                roots.push(task_name.clone());
            }
        }

        roots.sort();
        roots
    }

    #[allow(clippy::too_many_arguments)]
    fn render_task_tree(
        output: &mut String,
        task_name: &str,
        task_infos: &HashMap<String, crate::tui::events::TaskInfo>,
        task_configs: &HashMap<String, crate::cue_parser::TaskConfig>,
        depth: usize,
        prefix: &str,
        is_last: bool,
    ) {
        let connector = if depth == 0 {
            ""
        } else if is_last {
            "└─ "
        } else {
            "├─ "
        };

        let state = task_infos
            .get(task_name)
            .map(|info| &info.state)
            .unwrap_or(&TaskState::Queued);

        output.push_str(&format!(
            "{}{}{} {}\n",
            prefix,
            connector,
            state.icon(),
            task_name
        ));

        if let Some(config) = task_configs.get(task_name) {
            if let Some(deps) = &config.dependencies {
                let child_prefix = if depth == 0 {
                    "".to_string()
                } else if is_last {
                    format!("{}    ", prefix)
                } else {
                    format!("{}│   ", prefix)
                };

                let num_deps = deps.len();
                for (idx, dep) in deps.iter().enumerate() {
                    let is_last_dep = idx == num_deps - 1;
                    Self::render_task_tree(
                        output,
                        dep,
                        task_infos,
                        task_configs,
                        depth + 1,
                        &child_prefix,
                        is_last_dep,
                    );
                }
            }
        }
    }

    /// Generate Chrome Trace format JSON for visualization
    pub async fn generate_chrome_trace(&self) -> Result<String, serde_json::Error> {
        let mut events = Vec::new();
        let tasks = self.task_registry.get_all_tasks().await;

        for (task_name, task_info) in tasks {
            if let Some(start_time) = task_info.start_time {
                let start_us = start_time.duration_since(self.start_time).as_micros() as u64;

                let mut args = HashMap::new();
                args.insert(
                    "state".to_string(),
                    serde_json::Value::String(format!("{:?}", task_info.state)),
                );

                if let Some(exit_code) = task_info.exit_code {
                    args.insert(
                        "exit_code".to_string(),
                        serde_json::Value::Number(exit_code.into()),
                    );
                }

                if !task_info.dependencies.is_empty() {
                    args.insert(
                        "dependencies".to_string(),
                        serde_json::Value::Array(
                            task_info
                                .dependencies
                                .iter()
                                .map(|d| serde_json::Value::String(d.clone()))
                                .collect(),
                        ),
                    );
                }

                let duration = task_info.duration().map(|d| d.as_micros() as u64);

                events.push(ChromeTraceEvent {
                    name: task_name.clone(),
                    cat: "task".to_string(),
                    ph: if duration.is_some() {
                        "X".to_string()
                    } else {
                        "B".to_string()
                    },
                    ts: start_us,
                    dur: duration,
                    pid: std::process::id(),
                    tid: task_name,
                    args,
                });
            }
        }

        serde_json::to_string_pretty(&events)
    }

    /// Write output files for non-TTY environments
    pub async fn write_output_files(&self, plan: &TaskExecutionPlan) -> io::Result<()> {
        // Only write files if output path is configured
        if let Some(base_path) = &self.output_path {
            // Write ASCII DAG
            let dag_content = self.generate_ascii_dag(plan).await;
            let dag_path = format!("{}.dag.txt", base_path);
            let mut dag_file = File::create(&dag_path)?;
            dag_file.write_all(dag_content.as_bytes())?;
            println!("Task DAG written to: {}", dag_path);

            // Write Chrome Trace JSON
            if let Ok(trace_content) = self.generate_chrome_trace().await {
                let trace_path = format!("{}.trace.json", base_path);
                let mut trace_file = File::create(&trace_path)?;
                trace_file.write_all(trace_content.as_bytes())?;
                println!(
                    "Chrome Trace written to: {} (open in chrome://tracing)",
                    trace_path
                );
            }
        }

        Ok(())
    }

    /// Handle task events in non-TTY mode
    pub async fn handle_event(&self, event: TaskEvent) {
        match event {
            TaskEvent::Started { task_name, .. } => {
                println!("[START] {}", task_name);
            }
            TaskEvent::Progress { task_name, message } => {
                println!("[PROGRESS] {} - {}", task_name, message);
            }
            TaskEvent::Log {
                task_name,
                stream,
                content,
            } => {
                let prefix = match stream {
                    crate::tui::events::LogStream::Stdout => "[OUT]",
                    crate::tui::events::LogStream::Stderr => "[ERR]",
                    crate::tui::events::LogStream::System => "[SYS]",
                };
                for line in content.lines() {
                    println!("{} {} | {}", prefix, task_name, line);
                }
            }
            TaskEvent::Completed {
                task_name,
                exit_code,
                duration_ms,
            } => {
                println!(
                    "[DONE] {} - exit: {} - duration: {:.2}s",
                    task_name,
                    exit_code,
                    duration_ms as f64 / 1000.0
                );
            }
            TaskEvent::Failed {
                task_name,
                error,
                duration_ms,
            } => {
                println!(
                    "[FAIL] {} - error: {} - duration: {:.2}s",
                    task_name,
                    error,
                    duration_ms as f64 / 1000.0
                );
            }
            TaskEvent::Cancelled { task_name } => {
                println!("[CANCEL] {}", task_name);
            }
        }
    }
}
