use crate::events::{TaskEvent, TaskRegistry, TaskState};
use chrono::Local;
use cuenv_task::executor::TaskExecutionPlan;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

struct TaskTreeContext<'a> {
    task_infos: &'a HashMap<String, crate::events::TaskInfo>,
    task_configs: &'a HashMap<String, cuenv_core::TaskDefinition>,
}
use std::fs::File;
use std::io::{self, Write};
use std::time::Instant;

/// Non-TTY fallback that outputs ASCII DAG and Chrome Trace JSON
pub struct FallbackRenderer {
    task_registry: TaskRegistry,
    start_time: Instant,
    output_path: Option<String>,
}

#[derive(Serialize, Deserialize)]
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
        output.push_str(&format!("Generated: {timestamp}\n"));
        output.push_str(&format!("Total tasks: {}\n", plan.tasks.len()));
        output.push_str(&format!("Execution levels: {}\n\n", plan.levels.len()));

        // Build dependency tree
        let tasks = self.task_registry.get_all_tasks().await;
        let root_tasks = self.find_root_tasks(&plan.tasks);

        output.push_str("Dependency Tree:\n");
        output.push_str("---------------\n");

        for root in &root_tasks {
            let context = TaskTreeContext {
                task_infos: &tasks,
                task_configs: &plan.tasks,
            };
            Self::render_task_tree(&mut output, root, &context, 0, "", true);
        }

        output.push_str("\n\nExecution Order:\n");
        output.push_str("---------------\n");

        for (level_idx, level_tasks) in plan.levels.iter().enumerate() {
            output.push_str(&format!("\nLevel {level_idx}: "));

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

    fn find_root_tasks(&self, tasks: &HashMap<String, cuenv_core::TaskDefinition>) -> Vec<String> {
        let mut roots = Vec::new();
        
        // Build set of all dependency names
        let mut all_dependencies = std::collections::HashSet::new();
        for task in tasks.values() {
            for dep in task.dependency_names() {
                all_dependencies.insert(dep.clone());
            }
        }

        for (task_name, task) in tasks {
            // A task is a root if it has no dependencies AND is not a dependency of any other task
            if task.dependency_names().is_empty() && !all_dependencies.contains(task_name) {
                roots.push(task_name.clone());
            }
        }

        roots.sort();
        roots
    }

    fn render_task_tree(
        output: &mut String,
        task_name: &str,
        context: &TaskTreeContext<'_>,
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

        let state = context
            .task_infos
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

        if let Some(config) = context.task_configs.get(task_name) {
            let deps = config.dependency_names();
            if !deps.is_empty() {
                let child_prefix = if depth == 0 {
                    "".to_string()
                } else if is_last {
                    format!("{prefix}    ")
                } else {
                    format!("{prefix}│   ")
                };

                let num_deps = deps.len();
                for (idx, dep) in deps.iter().enumerate() {
                    let is_last_dep = idx == num_deps - 1;
                    Self::render_task_tree(
                        output,
                        dep,
                        context,
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
            let dag_path = format!("{base_path}.dag.txt");
            let mut dag_file = File::create(&dag_path)?;
            dag_file.write_all(dag_content.as_bytes())?;
            println!("Task DAG written to: {dag_path}");

            // Write Chrome Trace JSON
            if let Ok(trace_content) = self.generate_chrome_trace().await {
                let trace_path = format!("{base_path}.trace.json");
                let mut trace_file = File::create(&trace_path)?;
                trace_file.write_all(trace_content.as_bytes())?;
                println!("Chrome Trace written to: {trace_path} (open in chrome://tracing)");
            }
        }

        Ok(())
    }

    /// Handle task events in non-TTY mode
    pub async fn handle_event(&self, event: TaskEvent) {
        match event {
            TaskEvent::Started { task_name, .. } => {
                println!("[START] {task_name}");
            }
            TaskEvent::Progress { task_name, message } => {
                println!("[PROGRESS] {task_name} - {message}");
            }
            TaskEvent::Log {
                task_name,
                stream,
                content,
            } => {
                let prefix = match stream {
                    crate::events::LogStream::Stdout => "[OUT]",
                    crate::events::LogStream::Stderr => "[ERR]",
                    crate::events::LogStream::System => "[SYS]",
                };
                for line in content.lines() {
                    println!("{prefix} {task_name} | {line}");
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
                println!("[CANCEL] {task_name}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{LogStream, TaskEvent, TaskRegistry, TaskState};
    use cuenv_core::{ResolvedDependency, TaskDefinition, TaskExecutionMode};
    use cuenv_task::executor::TaskExecutionPlan;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    fn create_test_task_registry() -> TaskRegistry {
        TaskRegistry::new()
    }

    async fn setup_test_tasks(registry: &TaskRegistry) {
        // Create a complex dependency chain for testing
        registry.register_task("root1".to_string(), vec![]).await;
        registry.register_task("root2".to_string(), vec![]).await;
        registry
            .register_task("child1".to_string(), vec!["root1".to_string()])
            .await;
        registry
            .register_task(
                "child2".to_string(),
                vec!["root1".to_string(), "root2".to_string()],
            )
            .await;
        registry
            .register_task(
                "grandchild".to_string(),
                vec!["child1".to_string(), "child2".to_string()],
            )
            .await;

        // Set up various task states
        registry
            .update_task_state("root1", TaskState::Completed)
            .await;
        registry
            .update_task_state("root2", TaskState::Running)
            .await;
        registry
            .update_task_state("child1", TaskState::Failed)
            .await;
        registry
            .update_task_state("child2", TaskState::Running)
            .await;
        registry
            .update_task_state("grandchild", TaskState::Queued)
            .await;

        // Add some exit codes
        registry.set_exit_code("root1", 0).await;
        registry.set_exit_code("child1", 1).await;

        // Add some logs
        registry
            .add_log(
                "root1",
                LogStream::Stdout,
                "root1 completed successfully".to_string(),
            )
            .await;
        registry
            .add_log(
                "child1",
                LogStream::Stderr,
                "child1 failed with error".to_string(),
            )
            .await;
        registry
            .add_log(
                "child2",
                LogStream::System,
                "child2 in progress".to_string(),
            )
            .await;
    }

    fn create_test_task_definitions() -> HashMap<String, TaskDefinition> {
        let mut tasks = HashMap::new();

        // Create mock task definitions that match our test registry
        tasks.insert(
            "root1".to_string(),
            create_mock_task_definition("root1", vec![]),
        );
        tasks.insert(
            "root2".to_string(),
            create_mock_task_definition("root2", vec![]),
        );
        tasks.insert(
            "child1".to_string(),
            create_mock_task_definition("child1", vec!["root1".to_string()]),
        );
        tasks.insert(
            "child2".to_string(),
            create_mock_task_definition("child2", vec!["root1".to_string(), "root2".to_string()]),
        );
        tasks.insert(
            "grandchild".to_string(),
            create_mock_task_definition(
                "grandchild",
                vec!["child1".to_string(), "child2".to_string()],
            ),
        );

        tasks
    }

    fn create_mock_task_definition(name: &str, deps: Vec<String>) -> TaskDefinition {
        let mut task = TaskDefinition::new(
            name.to_string(),
            TaskExecutionMode::Command {
                command: format!("echo 'Running {name}'"),
            },
            PathBuf::from("/tmp"),
        );
        task.dependencies = deps.into_iter().map(ResolvedDependency::new).collect();
        task
    }

    fn create_test_execution_plan() -> TaskExecutionPlan {
        let tasks = create_test_task_definitions();
        let levels = vec![
            vec!["root1".to_string(), "root2".to_string()],
            vec!["child1".to_string(), "child2".to_string()],
            vec!["grandchild".to_string()],
        ];

        TaskExecutionPlan { tasks, levels }
    }

    #[tokio::test]
    async fn test_fallback_renderer_initialization() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry, None);

        assert!(renderer.output_path.is_none());
        // start_time should be recent
        assert!(renderer.start_time.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_fallback_renderer_initialization_with_output_path() {
        let registry = create_test_task_registry();
        let output_path = "/tmp/test_output".to_string();
        let renderer = FallbackRenderer::new(registry, Some(output_path.clone()));

        assert_eq!(renderer.output_path, Some(output_path));
    }

    #[tokio::test]
    async fn test_find_root_tasks() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry, None);
        let tasks = create_test_task_definitions();

        let roots = renderer.find_root_tasks(&tasks);

        // root1 and root2 should be the only root tasks (no dependencies)
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&"root1".to_string()));
        assert!(roots.contains(&"root2".to_string()));

        // Should be sorted
        assert_eq!(roots, vec!["root1".to_string(), "root2".to_string()]);
    }

    #[tokio::test]
    async fn test_find_root_tasks_empty() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry, None);
        let tasks = HashMap::new();

        let roots = renderer.find_root_tasks(&tasks);
        assert!(roots.is_empty());
    }

    #[tokio::test]
    async fn test_find_root_tasks_all_have_dependencies() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry, None);

        let mut tasks = HashMap::new();
        tasks.insert(
            "task1".to_string(),
            create_mock_task_definition("task1", vec!["task2".to_string()]),
        );
        tasks.insert(
            "task2".to_string(),
            create_mock_task_definition("task2", vec!["task1".to_string()]),
        );

        let roots = renderer.find_root_tasks(&tasks);
        assert!(roots.is_empty());
    }

    #[tokio::test]
    async fn test_generate_ascii_dag() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry.clone(), None);

        setup_test_tasks(&registry).await;
        let plan = create_test_execution_plan();

        let ascii_output = renderer.generate_ascii_dag(&plan).await;

        // Verify structure
        assert!(ascii_output.contains("Task Execution Plan"));
        assert!(ascii_output.contains("Total tasks: 5"));
        assert!(ascii_output.contains("Execution levels: 3"));
        assert!(ascii_output.contains("Dependency Tree:"));
        assert!(ascii_output.contains("Execution Order:"));
        assert!(ascii_output.contains("Legend:"));

        // Verify task states are shown with icons
        assert!(ascii_output.contains("✓ root1")); // Completed
        assert!(ascii_output.contains("▣ root2")); // Running
        assert!(ascii_output.contains("✖ child1")); // Failed
        assert!(ascii_output.contains("▣ child2")); // Running
        assert!(ascii_output.contains("◌ grandchild")); // Queued

        // Verify legend is present
        assert!(ascii_output.contains("◌ Queued  ▣ Running  ■ Completed  ✖ Failed  ⊘ Cancelled"));
    }

    #[tokio::test]
    async fn test_generate_ascii_dag_empty_plan() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry, None);

        let plan = TaskExecutionPlan {
            tasks: HashMap::new(),
            levels: vec![],
        };

        let ascii_output = renderer.generate_ascii_dag(&plan).await;

        assert!(ascii_output.contains("Total tasks: 0"));
        assert!(ascii_output.contains("Execution levels: 0"));
    }

    #[tokio::test]
    async fn test_generate_chrome_trace() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry.clone(), None);

        setup_test_tasks(&registry).await;

        let trace_json = renderer.generate_chrome_trace().await.unwrap();

        // Parse the JSON to verify structure
        let events: Vec<ChromeTraceEvent> = serde_json::from_str(&trace_json).unwrap();

        // Should have events for tasks that have start times
        assert!(!events.is_empty());

        for event in &events {
            assert_eq!(event.cat, "task");
            assert!(event.ph == "X" || event.ph == "B"); // Complete or Begin event
            assert_eq!(event.pid, std::process::id());
            assert!(!event.name.is_empty());
            assert!(!event.tid.is_empty());
        }
    }

    #[tokio::test]
    async fn test_generate_chrome_trace_empty() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry, None);

        let trace_json = renderer.generate_chrome_trace().await.unwrap();
        let events: Vec<ChromeTraceEvent> = serde_json::from_str(&trace_json).unwrap();

        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_handle_event_started() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry, None);

        let event = TaskEvent::Started {
            task_name: "test_task".to_string(),
            timestamp: Instant::now(),
        };

        // This should not panic
        renderer.handle_event(event).await;
    }

    #[tokio::test]
    async fn test_handle_event_progress() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry, None);

        let event = TaskEvent::Progress {
            task_name: "test_task".to_string(),
            message: "Processing...".to_string(),
        };

        renderer.handle_event(event).await;
    }

    #[tokio::test]
    async fn test_handle_event_log() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry, None);

        let event = TaskEvent::Log {
            task_name: "test_task".to_string(),
            stream: LogStream::Stdout,
            content: "Multi-line\nlog\ncontent".to_string(),
        };

        renderer.handle_event(event).await;
    }

    #[tokio::test]
    async fn test_handle_event_completed() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry, None);

        let event = TaskEvent::Completed {
            task_name: "test_task".to_string(),
            exit_code: 0,
            duration_ms: 1500,
        };

        renderer.handle_event(event).await;
    }

    #[tokio::test]
    async fn test_handle_event_failed() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry, None);

        let event = TaskEvent::Failed {
            task_name: "test_task".to_string(),
            error: "Process failed".to_string(),
            duration_ms: 2000,
        };

        renderer.handle_event(event).await;
    }

    #[tokio::test]
    async fn test_handle_event_cancelled() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry, None);

        let event = TaskEvent::Cancelled {
            task_name: "test_task".to_string(),
        };

        renderer.handle_event(event).await;
    }

    #[tokio::test]
    async fn test_write_output_files_no_path() {
        let registry = create_test_task_registry();
        let renderer = FallbackRenderer::new(registry, None);
        let plan = create_test_execution_plan();

        // Should not create any files when output_path is None
        let result = renderer.write_output_files(&plan).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_write_output_files_with_path() {
        let registry = create_test_task_registry();
        setup_test_tasks(&registry).await;

        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir
            .path()
            .join("test_output")
            .to_string_lossy()
            .to_string();

        let renderer = FallbackRenderer::new(registry, Some(output_path.clone()));
        let plan = create_test_execution_plan();

        let result = renderer.write_output_files(&plan).await;
        assert!(result.is_ok());

        // Verify files were created
        let dag_path = format!("{output_path}.dag.txt");
        let trace_path = format!("{output_path}.trace.json");

        assert!(std::path::Path::new(&dag_path).exists());
        assert!(std::path::Path::new(&trace_path).exists());

        // Verify content
        let dag_content = std::fs::read_to_string(&dag_path).unwrap();
        assert!(dag_content.contains("Task Execution Plan"));

        let trace_content = std::fs::read_to_string(&trace_path).unwrap();
        let _: Vec<ChromeTraceEvent> = serde_json::from_str(&trace_content).unwrap();
        // Should parse as valid JSON
    }

    #[tokio::test]
    async fn test_render_task_tree_structure() {
        let registry = create_test_task_registry();
        let _renderer = FallbackRenderer::new(registry.clone(), None);
        setup_test_tasks(&registry).await;

        let tasks = registry.get_all_tasks().await;
        let task_configs = create_test_task_definitions();
        let context = TaskTreeContext {
            task_infos: &tasks,
            task_configs: &task_configs,
        };

        let mut output = String::new();
        FallbackRenderer::render_task_tree(&mut output, "child1", &context, 0, "", true);

        // Should render child1 and its dependencies
        assert!(output.contains("✖ child1")); // child1 failed
        assert!(output.contains("└─ ✓ root1")); // root1 is a dependency of child1
    }

    #[tokio::test]
    async fn test_render_task_tree_depth() {
        let registry = create_test_task_registry();
        let _renderer = FallbackRenderer::new(registry.clone(), None);
        setup_test_tasks(&registry).await;

        let tasks = registry.get_all_tasks().await;
        let task_configs = create_test_task_definitions();
        let context = TaskTreeContext {
            task_infos: &tasks,
            task_configs: &task_configs,
        };

        let mut output = String::new();
        FallbackRenderer::render_task_tree(&mut output, "child1", &context, 1, "│   ", false);

        // Should have proper indentation for depth 1
        assert!(output.contains("├─ ✖ child1"));
    }

    #[tokio::test]
    async fn test_chrome_trace_event_structure() {
        let registry = create_test_task_registry();
        let mut renderer = FallbackRenderer::new(registry.clone(), None);

        // Set a known start time for deterministic testing
        renderer.start_time = Instant::now() - Duration::from_secs(10);

        // Create a task with complete timing information
        registry
            .register_task("timed_task".to_string(), vec!["dep".to_string()])
            .await;
        registry
            .update_task_state("timed_task", TaskState::Running)
            .await;
        registry.set_exit_code("timed_task", 42).await;

        let trace_json = renderer.generate_chrome_trace().await.unwrap();
        let events: Vec<ChromeTraceEvent> = serde_json::from_str(&trace_json).unwrap();

        if let Some(event) = events.first() {
            assert_eq!(event.name, "timed_task");
            assert_eq!(event.cat, "task");
            assert!(event.ph == "X" || event.ph == "B");
            assert_eq!(event.pid, std::process::id());
            assert_eq!(event.tid, "timed_task");

            // Verify args structure
            assert!(event.args.contains_key("state"));
            assert!(event.args.contains_key("exit_code"));
            assert!(event.args.contains_key("dependencies"));
        }
    }
}
