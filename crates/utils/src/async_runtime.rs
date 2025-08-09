use cuenv_core::{Error, Result};
use std::future::Future;
use tokio::runtime::{Builder, Runtime};

/// Async runtime manager that avoids creating runtime in async contexts
pub struct AsyncRuntime {
    runtime: Option<Runtime>,
}

impl AsyncRuntime {
    /// Create a new async runtime manager
    #[must_use]
    pub fn new() -> Self {
        Self { runtime: None }
    }

    /// Get or create a runtime
    fn get_or_create_runtime(&mut self) -> Result<&Runtime> {
        if self.runtime.is_none() {
            let runtime = match Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(e) => {
                    return Err(Error::configuration(format!(
                        "failed to create tokio runtime: {e}"
                    )));
                }
            };
            self.runtime = Some(runtime);
        }

        self.runtime.as_ref().ok_or_else(|| {
            Error::configuration("runtime unexpectedly missing after initialization")
        })
    }

    /// Execute an async function, creating a runtime if needed
    pub fn block_on<F, T>(&mut self, future: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        let runtime = self.get_or_create_runtime()?;

        runtime.block_on(future)
    }

    /// Check if we're already in an async context
    #[must_use]
    pub fn is_in_async_context() -> bool {
        tokio::runtime::Handle::try_current().is_ok()
    }

    /// Execute an async function, handling both sync and async contexts
    pub async fn execute<F, T>(future: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        // If we're already in an async context, just await the future
        future.await
    }
}

impl Default for AsyncRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to run async code from sync context safely
pub fn run_async<F, T>(future: F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    if AsyncRuntime::is_in_async_context() {
        // We're already in async context, can't block_on
        return Err(Error::configuration(
            "cannot use block_on from within an async runtime",
        ));
    }

    let mut runtime = AsyncRuntime::new();
    runtime.block_on(future)
}

/// Extension trait for making functions async-ready
pub trait AsyncReady {
    type Output;

    /// Convert to async-compatible version
    fn to_async(self) -> impl Future<Output = Self::Output>;
}

#[cfg(test)]
pub mod test_utils {
    use super::*;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;

    /// Global test runtime to reduce TLS slot usage
    static TEST_RUNTIME: Lazy<Mutex<Option<Runtime>>> = Lazy::new(|| Mutex::new(None));

    /// Get or create a shared test runtime to avoid TLS exhaustion
    pub fn get_test_runtime() -> std::sync::MutexGuard<'static, Option<Runtime>> {
        TEST_RUNTIME.lock().unwrap()
    }

    /// Execute async code using the shared test runtime
    pub fn run_test_async<F, T>(future: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        if AsyncRuntime::is_in_async_context() {
            return Err(Error::configuration(
                "cannot use run_test_async from within an async runtime",
            ));
        }

        let mut guard = get_test_runtime();
        if guard.is_none() {
            match Builder::new_current_thread().enable_all().build() {
                Ok(rt) => *guard = Some(rt),
                Err(e) => {
                    return Err(Error::configuration(format!(
                        "failed to create test runtime: {e}"
                    )));
                }
            }
        }

        if let Some(runtime) = guard.as_ref() {
            runtime.block_on(future)
        } else {
            Err(Error::configuration("test runtime not available"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "TLS exhaustion in CI - use nextest profile to run"]
    fn test_sync_context_execution() -> Result<()> {
        let mut runtime = AsyncRuntime::new();

        let result = runtime.block_on(async { Ok::<i32, Error>(42) });

        assert!(result.is_ok());
        assert_eq!(result?, 42);

        Ok(())
    }

    #[tokio::test]
    async fn test_async_context_detection() -> Result<()> {
        assert!(AsyncRuntime::is_in_async_context());

        let result = AsyncRuntime::execute(async { Ok::<i32, Error>(42) }).await;

        assert!(result.is_ok());
        assert_eq!(result?, 42);

        Ok(())
    }

    #[test]
    #[ignore = "TLS exhaustion in CI - use nextest profile to run"]
    fn test_run_async_helper() -> Result<()> {
        let result = run_async(async { Ok::<String, Error>("hello".to_string()) });

        assert!(result.is_ok());
        assert_eq!(result?, "hello");

        Ok(())
    }

    #[tokio::test]
    async fn test_nested_async_fails() {
        // This should fail because we're already in an async context
        let result = run_async(async { Ok::<String, Error>("should fail".to_string()) });

        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("async runtime"));
        }
    }
}
