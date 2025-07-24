use crate::errors::{Error, Result};
use crate::types::{CommandArguments, EnvironmentVariables};
use async_trait::async_trait;
#[cfg(test)]
use std::collections::HashMap;
use std::process::Output;

/// Trait for executing external commands
/// This abstraction allows for testing without mocking by providing
/// different implementations for production and test environments
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    /// Execute a command with the given arguments
    /// Returns the output of the command
    async fn execute(&self, cmd: &str, args: &CommandArguments) -> Result<Output>;

    /// Execute a command with environment variables
    async fn execute_with_env(
        &self,
        cmd: &str,
        args: &CommandArguments,
        env: EnvironmentVariables,
    ) -> Result<Output>;
}

/// Production implementation that executes real commands
pub struct SystemCommandExecutor;

#[async_trait]
impl CommandExecutor for SystemCommandExecutor {
    async fn execute(&self, cmd: &str, args: &CommandArguments) -> Result<Output> {
        match std::process::Command::new(cmd)
            .args(args.as_slice())
            .output()
        {
            Ok(output) => Ok(output),
            Err(e) => Err(Error::command_execution(
                cmd,
                args.clone().into_inner(),
                format!("failed to execute command: {e}"),
                None,
            )),
        }
    }

    async fn execute_with_env(
        &self,
        cmd: &str,
        args: &CommandArguments,
        env: EnvironmentVariables,
    ) -> Result<Output> {
        match std::process::Command::new(cmd)
            .args(args.as_slice())
            .envs(env.into_inner())
            .output()
        {
            Ok(output) => Ok(output),
            Err(e) => Err(Error::command_execution(
                cmd,
                args.clone().into_inner(),
                format!("failed to execute command with environment: {e}"),
                None,
            )),
        }
    }
}

/// Test implementation that simulates command execution
/// This provides deterministic behavior for testing
#[cfg(test)]
pub struct TestCommandExecutor {
    responses: std::sync::Arc<std::sync::Mutex<HashMap<String, TestResponse>>>,
}

#[cfg(test)]
#[derive(Clone)]
pub struct TestResponse {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status_code: i32,
}

#[cfg(test)]
impl TestCommandExecutor {
    pub fn new() -> Self {
        Self {
            responses: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn add_response(&self, cmd: &str, args: &[String], response: TestResponse) {
        let key = format!("{} {}", cmd, args.join(" "));
        let mut responses = self.responses.lock().unwrap();
        responses.insert(key, response);
    }

    pub fn add_simple_response(&self, cmd: &str, args: &[String], stdout: &str) {
        self.add_response(
            cmd,
            args,
            TestResponse {
                stdout: stdout.as_bytes().to_vec(),
                stderr: Vec::new(),
                status_code: 0,
            },
        );
    }

    pub fn add_error_response(&self, cmd: &str, args: &[String], stderr: &str) {
        self.add_response(
            cmd,
            args,
            TestResponse {
                stdout: Vec::new(),
                stderr: stderr.as_bytes().to_vec(),
                status_code: 1,
            },
        );
    }
}

#[cfg(test)]
#[async_trait]
impl CommandExecutor for TestCommandExecutor {
    async fn execute(&self, cmd: &str, args: &CommandArguments) -> Result<Output> {
        let key = format!("{} {}", cmd, args.as_slice().join(" "));
        let responses = self.responses.lock().unwrap();

        match responses.get(&key) {
            Some(response) => Ok(Output {
                status: exit_status::from_raw(response.status_code),
                stdout: response.stdout.clone(),
                stderr: response.stderr.clone(),
            }),
            None => Err(Error::configuration(format!(
                "no test response configured for command: {}",
                key
            ))),
        }
    }

    async fn execute_with_env(
        &self,
        cmd: &str,
        args: &CommandArguments,
        _env: EnvironmentVariables,
    ) -> Result<Output> {
        // For testing, we ignore env vars and just use the base execute
        self.execute(cmd, args).await
    }
}

/// Factory for creating command executors
pub struct CommandExecutorFactory;

impl CommandExecutorFactory {
    /// Create a production command executor
    pub fn system() -> Box<dyn CommandExecutor> {
        Box::new(SystemCommandExecutor)
    }

    /// Create a test command executor
    #[cfg(test)]
    pub fn test() -> TestCommandExecutor {
        TestCommandExecutor::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_test_executor_simple_response() {
        let executor = CommandExecutorFactory::test();
        executor.add_simple_response("echo", &["hello".to_string()], "hello\n");

        let args = CommandArguments::from_vec(vec!["hello".to_string()]);
        let output = executor.execute("echo", &args).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout), "hello\n");
        assert!(output.status.success());
    }

    #[tokio::test]
    async fn test_test_executor_error_response() {
        let executor = CommandExecutorFactory::test();
        executor.add_error_response("false", &[], "command failed");

        let args = CommandArguments::new();
        let output = executor.execute("false", &args).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stderr), "command failed");
        assert!(!output.status.success());
    }

    #[tokio::test]
    async fn test_test_executor_missing_response() {
        let executor = CommandExecutorFactory::test();

        let args = CommandArguments::from_vec(vec!["cmd".to_string()]);
        let result = executor.execute("unknown", &args).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("no test response configured"));
    }
}

// Platform-specific module for creating ExitStatus
#[cfg(test)]
mod exit_status {
    #[cfg(unix)]
    pub fn from_raw(code: i32) -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code)
    }

    #[cfg(windows)]
    pub fn from_raw(code: i32) -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code as u32)
    }
}
