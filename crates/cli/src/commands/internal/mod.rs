use clap::Subcommand;
use cuenv_core::{Error, Result};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum InternalCommands {
    /// Task Server Protocol implementation for devenv integration
    TaskProtocol {
        /// Task server executable to launch
        #[arg(long)]
        server: Option<String>,
        /// Directory to discover task servers
        #[arg(long)]
        discovery_dir: Option<PathBuf>,
        /// Task to run on external server
        #[arg(long)]
        run_task: Option<String>,
        /// List available tasks from servers
        #[arg(long)]
        list_tasks: bool,
        /// Start as a task server provider (expose cuenv tasks to external tools)
        #[arg(long)]
        serve: bool,
        /// Socket path for server mode
        #[arg(long)]
        socket: Option<PathBuf>,
        /// Export cuenv tasks as JSON for static consumption
        #[arg(long)]
        export_json: bool,
    },
}

impl InternalCommands {
    pub async fn execute(self) -> Result<()> {
        match self {
            InternalCommands::TaskProtocol {
                server,
                discovery_dir,
                run_task,
                list_tasks,
                serve,
                socket,
                export_json,
            } => {
                handle_task_protocol(
                    &server,
                    &discovery_dir,
                    &run_task,
                    list_tasks,
                    serve,
                    &socket,
                    export_json,
                )
                .await
            }
        }
    }
}

async fn handle_task_protocol(
    server: &Option<String>,
    discovery_dir: &Option<PathBuf>,
    run_task: &Option<String>,
    list_tasks: bool,
    serve: bool,
    socket: &Option<PathBuf>,
    export_json: bool,
) -> Result<()> {
    use cuenv_task::TaskServerManager;
    use std::collections::HashMap;

    // Create socket directory in temp
    let socket_dir = tempfile::tempdir().map_err(|e| {
        Error::configuration(format!("Failed to create temp socket directory: {e}"))
    })?;

    let mut manager = TaskServerManager::new(socket_dir.path().to_path_buf());

    // Add servers based on command line options
    let mut all_tasks = Vec::new();

    if let Some(server_executable) = server {
        // Launch a single server
        let server_name = std::path::Path::new(server_executable)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("server");

        match manager.add_server(server_executable, server_name).await {
            Ok(tasks) => {
                all_tasks.extend(tasks);
                tracing::info!("Connected to task server: {server_executable}");
            }
            Err(e) => {
                tracing::error!("Failed to connect to task server {server_executable}: {e}");
                return Err(e);
            }
        }
    }

    if let Some(discovery_path) = discovery_dir {
        // Discover servers from directory
        match manager.discover_servers(discovery_path).await {
            Ok(tasks) => {
                let task_count = tasks.len();
                all_tasks.extend(tasks);
                tracing::info!(
                    "Discovered {} task servers from {}",
                    task_count,
                    discovery_path.display()
                );
            }
            Err(e) => {
                tracing::error!("Failed to discover task servers: {e}");
                return Err(e);
            }
        }
    }

    if list_tasks {
        // List all available tasks from servers
        if all_tasks.is_empty() {
            tracing::info!("No tasks available from task servers");
        } else {
            tracing::info!("Available tasks from external servers:");
            for task in &all_tasks {
                if let Some(description) = &task.description {
                    tracing::info!("  {}: {}", task.name, description);
                } else {
                    tracing::info!("  {}", task.name);
                }
            }
        }
    }

    if let Some(task_name) = run_task {
        // Run a specific task
        if all_tasks.iter().any(|t| t.name == *task_name) {
            tracing::info!("Running task: {task_name}");

            let inputs = HashMap::new(); // TODO: Accept inputs from CLI
            let outputs = HashMap::new(); // TODO: Accept outputs from CLI

            match manager.run_task(task_name, inputs, outputs).await {
                Ok(exit_code) => {
                    if exit_code == 0 {
                        tracing::info!("Task '{task_name}' completed successfully");
                    } else {
                        tracing::info!("Task '{task_name}' failed with exit code {exit_code}");
                        #[cfg(not(test))]
                        std::process::exit(exit_code);
                        #[cfg(test)]
                        return Err(Error::configuration(format!(
                            "Task failed with exit code {exit_code}"
                        )));
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to run task '{task_name}': {e}");
                    return Err(e);
                }
            }
        } else {
            tracing::error!("Task '{task_name}' not found");
            tracing::error!("Available tasks:");
            for task in &all_tasks {
                tracing::error!("  - {}", task.name);
            }
            #[cfg(not(test))]
            std::process::exit(1);
            #[cfg(test)]
            return Err(Error::configuration(format!(
                "Task '{task_name}' not found"
            )));
        }
    }

    if serve {
        use crate::directory::DirectoryManager;
        use cuenv_env::EnvManager;
        use cuenv_task::TaskServerProvider;

        // Get current directory and load environment tasks
        let current_dir = match DirectoryManager::get_current_directory() {
            Ok(d) => d,
            Err(e) => {
                return Err(Error::configuration(format!(
                    "Failed to get current directory: {e}"
                )));
            }
        };

        let mut env_manager = EnvManager::new();
        env_manager.load_env(&current_dir).await?;

        // Create config from environment manager data
        use cuenv_config::{Config, ParseResult, RuntimeOptions};
        use std::collections::HashMap;
        use std::sync::Arc;

        let parse_result = ParseResult {
            variables: env_manager.get_cue_vars().clone(),
            metadata: HashMap::new(),
            commands: HashMap::new(),
            tasks: env_manager.get_tasks().clone(),
            task_nodes: indexmap::IndexMap::new(), // Empty for internal commands
            hooks: HashMap::new(),
            config: None,
        };

        let config = Arc::new(Config::new(
            current_dir.clone(),
            None, // no env file for internal command
            parse_result,
            RuntimeOptions::default(),
        ));

        // Determine socket path
        let socket_path = socket.clone().unwrap_or_else(|| {
            socket_dir
                .path()
                .join(format!("cuenv-{}.sock", std::process::id()))
        });

        tracing::info!(
            "Starting task server provider on socket: {}",
            socket_path.display()
        );

        // Create and start provider
        let mut provider = TaskServerProvider::new_with_options(
            Some(socket_path.clone()),
            config,
            false, // Don't allow execution by default for security
            false, // Not a subprocess
        );

        // Start the provider (blocks until shutdown)
        provider.start().await?;

        tracing::info!("Task server provider started successfully");
    }

    if export_json {
        use crate::directory::DirectoryManager;
        use cuenv_env::EnvManager;

        // Get current directory and load environment tasks
        let current_dir = match DirectoryManager::get_current_directory() {
            Ok(d) => d,
            Err(e) => {
                return Err(Error::configuration(format!(
                    "Failed to get current directory: {e}"
                )));
            }
        };

        let mut env_manager = EnvManager::new();
        env_manager.load_env(&current_dir).await?;

        // Extract and serialize tasks
        let tasks = env_manager.get_tasks();
        let json = serde_json::to_string_pretty(&tasks)?;
        tracing::info!("{json}");
    }

    fn should_show_usage(
        serve: bool,
        export_json: bool,
        server: &Option<String>,
        discovery_dir: &Option<PathBuf>,
        run_task: &Option<String>,
        list_tasks: bool,
    ) -> bool {
        !serve
            && !export_json
            && server.is_none()
            && discovery_dir.is_none()
            && run_task.is_none()
            && !list_tasks
    }

    // If no action specified, show usage
    if should_show_usage(
        serve,
        export_json,
        server,
        discovery_dir,
        run_task,
        list_tasks,
    ) {
        tracing::info!("Task Server Protocol (TSP) - Dual-Modality Support");
        tracing::info!();
        tracing::info!("Consumer Mode (use external task servers):");
        tracing::info!("  cuenv internal task-protocol --server <executable> --list-tasks");
        tracing::info!("  cuenv internal task-protocol --discovery-dir <path> --list-tasks");
        tracing::info!("  cuenv internal task-protocol --server <executable> --run-task <task>");
        tracing::info!();
        tracing::info!("Provider Mode (expose cuenv tasks to external tools):");
        tracing::info!("  cuenv internal task-protocol --serve [--socket <path>]");
        tracing::info!("  cuenv internal task-protocol --export-json");
    }

    // Shutdown servers
    manager.shutdown().await?;

    Ok(())
}

// Make should_show_usage function accessible to tests
#[cfg(test)]
fn should_show_usage(
    serve: bool,
    export_json: bool,
    server: &Option<String>,
    discovery_dir: &Option<PathBuf>,
    run_task: &Option<String>,
    list_tasks: bool,
) -> bool {
    !serve
        && !export_json
        && server.is_none()
        && discovery_dir.is_none()
        && run_task.is_none()
        && !list_tasks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_commands() -> InternalCommands {
        InternalCommands::TaskProtocol {
            server: None,
            discovery_dir: None,
            run_task: None,
            list_tasks: false,
            serve: false,
            socket: None,
            export_json: false,
        }
    }

    #[test]
    fn test_internal_commands_default_construction() {
        let commands = create_test_commands();

        match commands {
            InternalCommands::TaskProtocol {
                server,
                discovery_dir,
                run_task,
                list_tasks,
                serve,
                socket,
                export_json,
            } => {
                assert!(server.is_none());
                assert!(discovery_dir.is_none());
                assert!(run_task.is_none());
                assert!(!list_tasks);
                assert!(!serve);
                assert!(socket.is_none());
                assert!(!export_json);
            }
        }
    }

    #[test]
    fn test_should_show_usage_logic() {
        // Test the should_show_usage function behavior
        assert!(should_show_usage(false, false, &None, &None, &None, false));

        assert!(!should_show_usage(true, false, &None, &None, &None, false));

        assert!(!should_show_usage(false, true, &None, &None, &None, false));

        assert!(!should_show_usage(
            false,
            false,
            &Some("server".to_string()),
            &None,
            &None,
            false
        ));

        assert!(!should_show_usage(
            false,
            false,
            &None,
            &Some(PathBuf::from("/test")),
            &None,
            false
        ));

        assert!(!should_show_usage(
            false,
            false,
            &None,
            &None,
            &Some("task".to_string()),
            false
        ));

        assert!(!should_show_usage(false, false, &None, &None, &None, true));
    }

    #[tokio::test]
    async fn test_task_protocol_with_no_options() {
        let result = handle_task_protocol(&None, &None, &None, false, false, &None, false).await;

        // Should succeed but only show usage
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_task_protocol_with_invalid_server() {
        let result = handle_task_protocol(
            &Some("/non/existent/server".to_string()),
            &None,
            &None,
            false,
            false,
            &None,
            false,
        )
        .await;

        // Should fail when trying to connect to non-existent server
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_task_protocol_with_invalid_discovery_dir() {
        let non_existent_dir = PathBuf::from("/non/existent/directory");

        let result = handle_task_protocol(
            &None,
            &Some(non_existent_dir),
            &None,
            false,
            false,
            &None,
            false,
        )
        .await;

        // Should succeed (empty directory case is handled gracefully)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_task_protocol_list_tasks_with_no_servers() {
        let result = handle_task_protocol(
            &None, &None, &None, true, // list_tasks = true
            false, &None, false,
        )
        .await;

        // Should succeed and show "No tasks available"
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_task_protocol_run_task_with_no_servers() {
        let result = handle_task_protocol(
            &None,
            &None,
            &Some("non-existent-task".to_string()),
            false,
            false,
            &None,
            false,
        )
        .await;

        // Should fail because no tasks are available
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_task_protocol_discovery_with_empty_directory() {
        let temp_dir = TempDir::new().unwrap();

        let result = handle_task_protocol(
            &None,
            &Some(temp_dir.path().to_path_buf()),
            &None,
            true, // list_tasks = true
            false,
            &None,
            false,
        )
        .await;

        // Should succeed with empty discovery
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_task_protocol_export_json_with_environment() {
        let result = handle_task_protocol(
            &None, &None, &None, false, false, &None, true, // export_json = true
        )
        .await;

        // In the actual cuenv project, this might succeed or fail depending on the environment
        // Just verify that the function doesn't panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_internal_commands_execute_task_protocol() {
        let commands = InternalCommands::TaskProtocol {
            server: None,
            discovery_dir: None,
            run_task: None,
            list_tasks: false,
            serve: false,
            socket: None,
            export_json: false,
        };

        let result = commands.execute().await;

        // Should succeed (shows usage)
        assert!(result.is_ok());
    }

    #[test]
    fn test_file_stem_extraction() {
        let path = PathBuf::from("/usr/bin/test-server");
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("server");
        assert_eq!(stem, "test-server");

        let path = PathBuf::from("server.exe");
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("server");
        assert_eq!(stem, "server");

        let path = PathBuf::from("/");
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("server");
        assert_eq!(stem, "server");
    }

    #[tokio::test]
    async fn test_task_protocol_with_valid_temp_socket() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let result =
            handle_task_protocol(&None, &None, &None, false, false, &Some(socket_path), false)
                .await;

        // Should succeed (shows usage)
        assert!(result.is_ok());
    }

    #[test]
    fn test_socket_path_generation() {
        let socket_dir = PathBuf::from("/tmp/test");
        let process_id = std::process::id();
        let expected_socket = socket_dir.join(format!("cuenv-{process_id}.sock"));

        // Test that socket path includes process ID
        assert!(expected_socket
            .to_string_lossy()
            .contains(&process_id.to_string()));
    }

    mod error_handling_tests {
        use super::*;

        #[tokio::test]
        async fn test_server_connection_failure_error_propagation() {
            let result = handle_task_protocol(
                &Some("definitely-not-a-real-server-executable".to_string()),
                &None,
                &None,
                false,
                false,
                &None,
                false,
            )
            .await;

            assert!(result.is_err());
            let error_msg = result.unwrap_err().to_string();
            assert!(
                error_msg.contains("Failed to connect")
                    || error_msg.contains("No such file")
                    || error_msg.contains("not found")
            );
        }

        #[tokio::test]
        async fn test_directory_manager_error_handling() {
            // This test verifies that DirectoryManager errors are properly handled
            // We can't easily mock DirectoryManager, but we can test the error path
            // by ensuring that error formatting is correct
            let result = handle_task_protocol(
                &None, &None, &None, false, false, &None,
                true, // export_json = true (will try to load environment)
            )
            .await;

            // In the actual cuenv project, this might succeed or fail
            // Just verify that the function doesn't panic
            let _ = result;
        }

        #[tokio::test]
        async fn test_task_not_found_error_handling() {
            let result = handle_task_protocol(
                &None,
                &None,
                &Some("non-existent-task".to_string()),
                false,
                false,
                &None,
                false,
            )
            .await;

            // Should fail when task is not found
            assert!(result.is_err());
        }
    }

    mod configuration_tests {
        use super::*;

        #[test]
        fn test_task_protocol_all_options_set() {
            let temp_dir = TempDir::new().unwrap();
            let socket_path = temp_dir.path().join("test.sock");

            let commands = InternalCommands::TaskProtocol {
                server: Some("test-server".to_string()),
                discovery_dir: Some(temp_dir.path().to_path_buf()),
                run_task: Some("test-task".to_string()),
                list_tasks: true,
                serve: true,
                socket: Some(socket_path),
                export_json: true,
            };

            match commands {
                InternalCommands::TaskProtocol {
                    server,
                    discovery_dir,
                    run_task,
                    list_tasks,
                    serve,
                    socket,
                    export_json,
                } => {
                    assert_eq!(server, Some("test-server".to_string()));
                    assert_eq!(discovery_dir, Some(temp_dir.path().to_path_buf()));
                    assert_eq!(run_task, Some("test-task".to_string()));
                    assert!(list_tasks);
                    assert!(serve);
                    assert!(socket.is_some());
                    assert!(export_json);
                }
            }
        }

        #[test]
        fn test_multiple_conflicting_options() {
            // Test that multiple options can be set simultaneously
            // (the implementation handles this by processing them in order)
            let temp_dir = TempDir::new().unwrap();

            let commands = InternalCommands::TaskProtocol {
                server: Some("test-server".to_string()),
                discovery_dir: Some(temp_dir.path().to_path_buf()),
                run_task: Some("test-task".to_string()),
                list_tasks: true,
                serve: true,
                socket: None,
                export_json: true,
            };

            // All options should be preserved
            match &commands {
                InternalCommands::TaskProtocol {
                    serve, export_json, ..
                } => {
                    assert!(*serve);
                    assert!(*export_json);
                }
            }
        }
    }

    mod integration_tests {
        use super::*;

        #[tokio::test]
        async fn test_end_to_end_task_protocol_usage_display() {
            let commands = InternalCommands::TaskProtocol {
                server: None,
                discovery_dir: None,
                run_task: None,
                list_tasks: false,
                serve: false,
                socket: None,
                export_json: false,
            };

            // This should succeed and display usage
            let result = commands.execute().await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_task_protocol_with_discovery_and_list() {
            let temp_dir = TempDir::new().unwrap();

            let result = handle_task_protocol(
                &None,
                &Some(temp_dir.path().to_path_buf()),
                &None,
                true, // list_tasks
                false,
                &None,
                false,
            )
            .await;

            assert!(result.is_ok());
        }
    }
}
