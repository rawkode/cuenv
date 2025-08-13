mod behaviours;

use behaviours::world::TestWorld;
use cucumber::World as _;

#[tokio::main]
async fn main() {
    // Use the correct path relative to where the test runs
    // The test binary runs from the workspace root
    TestWorld::cucumber()
        .run_and_exit("tests/behaviours/features")
        .await;
}
