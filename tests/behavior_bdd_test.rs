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
