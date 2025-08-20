mod behaviours;

use behaviours::world::TestWorld;
use cucumber::World as _;

#[tokio::test]
async fn test_environment_lifecycle() {
    TestWorld::cucumber()
        .features(&["./tests/behaviours/features/environment_lifecycle.feature"])
        .scenario_regex("Environment unloads when leaving directory")
        .run()
        .await;
}

#[tokio::test]
async fn test_exec_waits_for_preload_hooks() {
    TestWorld::cucumber()
        .features(&["./tests/behaviours/features/hook_execution.feature"])
        .scenario_regex("Exec command waits for preload hooks to complete")
        .run()
        .await;
}

#[tokio::test]
async fn test_security_tasks() {
    TestWorld::cucumber()
        .features(&["./tests/behaviours/features/security_tasks.feature"])
        .run()
        .await;
}

#[tokio::test]
async fn test_default_background_loading_with_preload_hooks() {
    TestWorld::cucumber()
        .features(&["./tests/behaviours/features/environment_lifecycle.feature"])
        .scenario_regex("Default background loading with preload hooks")
        .run()
        .await;
}

#[tokio::test]
async fn test_preload_hooks_wait_for_exec() {
    TestWorld::cucumber()
        .features(&["./tests/behaviours/features/environment_lifecycle.feature"])
        .scenario_regex("Preload hooks wait when using cuenv exec")
        .run()
        .await;
}

#[tokio::test]
async fn test_preload_hooks_wait_for_task() {
    TestWorld::cucumber()
        .features(&["./tests/behaviours/features/environment_lifecycle.feature"])
        .scenario_regex("Preload hooks wait when using cuenv task")
        .run()
        .await;
}

#[tokio::test]
async fn test_slow_preload_hooks_timing() {
    TestWorld::cucumber()
        .features(&["./tests/behaviours/features/environment_lifecycle.feature"])
        .scenario_regex("Environment variables available only after slow preload hooks finish")
        .run()
        .await;
}
