//! Tests for task group execution strategies

use super::*;
use cuenv_config::{TaskCollection, TaskConfig, TaskNode};
use indexmap::IndexMap;

// Import the strategy structs directly since they're not all re-exported
use super::parallel::ParallelStrategy;

fn create_test_task(dependencies: Option<Vec<String>>) -> TaskNode {
    TaskNode::Task(Box::new(TaskConfig {
        command: Some("echo test".to_string()),
        dependencies,
        ..Default::default()
    }))
}

fn create_test_group(tasks: TaskCollection) -> TaskNode {
    TaskNode::Group {
        tasks,
        description: None,
    }
}

#[test]
fn test_parallel_strategy_creates_barriers() {
    let strategy = ParallelStrategy;
    let mut tasks = IndexMap::new();
    tasks.insert("task1".to_string(), create_test_task(None));
    tasks.insert("task2".to_string(), create_test_task(None));

    let collection = TaskCollection::Parallel(tasks);
    let result = strategy.process_group("test", &collection, vec![]).unwrap();

    // Should have start barrier, 2 tasks, end barrier = 4 items
    assert_eq!(result.len(), 4);

    // Find barriers
    let barriers: Vec<_> = result.iter().filter(|t| t.is_barrier).collect();
    assert_eq!(barriers.len(), 2); // start and end barriers

    // All regular tasks should depend on start barrier
    let start_barrier_id = "test:__start__";
    for task in result.iter().filter(|t| !t.is_barrier) {
        assert!(task.dependencies.contains(&start_barrier_id.to_string()));
    }
}

#[test]
fn test_sequential_strategy_chains_dependencies() {
    let strategy = SequentialStrategy;
    let mut tasks = IndexMap::new();
    tasks.insert("task1".to_string(), create_test_task(None));
    tasks.insert("task2".to_string(), create_test_task(None));

    let task_vec: Vec<TaskNode> = tasks.values().cloned().collect();
    let collection = TaskCollection::Sequential(task_vec);
    let result = strategy.process_group("test", &collection, vec![]).unwrap();

    // Should have start barrier, 2 tasks, end barrier = 4 items
    assert_eq!(result.len(), 4);

    // Find the regular tasks (non-barrier)
    let regular_tasks: Vec<_> = result.iter().filter(|t| !t.is_barrier).collect();
    assert_eq!(regular_tasks.len(), 2);

    // Tasks should have auto-generated names for sequential execution
    let task_names: Vec<String> = regular_tasks.iter().map(|t| t.name.clone()).collect();
    assert_eq!(task_names, vec!["task_0".to_string(), "task_1".to_string()]);
}

#[test]
fn test_group_strategy_preserves_tasks() {
    let strategy = GroupStrategy;
    let mut tasks = IndexMap::new();
    tasks.insert("task1".to_string(), create_test_task(None));
    tasks.insert("task2".to_string(), create_test_task(None));

    let collection = TaskCollection::Parallel(tasks);
    let result = strategy.process_group("test", &collection, vec![]).unwrap();

    // Group strategy should just preserve tasks without modification
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|t| !t.is_barrier));
}

#[test]
fn test_nested_groups_execution() {
    let strategy = ParallelStrategy;
    let mut tasks = IndexMap::new();

    // Create a nested group
    let mut nested_tasks = IndexMap::new();
    nested_tasks.insert("nested1".to_string(), create_test_task(None));
    nested_tasks.insert("nested2".to_string(), create_test_task(None));

    tasks.insert("simple".to_string(), create_test_task(None));
    tasks.insert(
        "group".to_string(),
        create_test_group(TaskCollection::Parallel(nested_tasks)),
    );

    let collection = TaskCollection::Parallel(tasks);
    let result = strategy.process_group("test", &collection, vec![]).unwrap();

    // Should handle nested groups
    assert!(result.len() > 2); // More than just the two top-level items due to nesting
}

#[test]
fn test_strategy_differences() {
    // Test that different strategies process groups differently
    let sequential = SequentialStrategy;
    let group = GroupStrategy;

    // We can test that they process groups differently
    let mut tasks = IndexMap::new();
    tasks.insert("task1".to_string(), create_test_task(None));

    // Convert IndexMap to Vec for Sequential variant
    let task_vec: Vec<TaskNode> = tasks.values().cloned().collect();
    let sequential_collection = TaskCollection::Sequential(task_vec);
    let group_collection = TaskCollection::Parallel(tasks.clone());

    let sequential_result = sequential
        .process_group("test", &sequential_collection, vec![])
        .unwrap();
    let group_result = group
        .process_group("test", &group_collection, vec![])
        .unwrap();

    // Sequential should add barriers, group shouldn't
    assert!(sequential_result.len() > group_result.len());
}

#[test]
fn test_parallel_ordering_deterministic() {
    let strategy = ParallelStrategy;
    let mut tasks = IndexMap::new();

    // Insert in different order multiple times to test determinism
    for _ in 0..5 {
        tasks.clear();
        tasks.insert("zzz".to_string(), create_test_task(None));
        tasks.insert("aaa".to_string(), create_test_task(None));
        tasks.insert("mmm".to_string(), create_test_task(None));

        let collection = TaskCollection::Parallel(tasks.clone());
        let result = strategy.process_group("test", &collection, vec![]).unwrap();
        let task_names: Vec<String> = result
            .iter()
            .filter(|t| !t.is_barrier)
            .map(|t| t.name.clone())
            .collect();

        // Should preserve IndexMap insertion order for parallel tasks
        assert_eq!(
            task_names,
            vec!["zzz".to_string(), "aaa".to_string(), "mmm".to_string()]
        );
    }
}

#[test]
fn test_sequential_positional_ordering() {
    let strategy = SequentialStrategy;

    // Create tasks in a specific order to test positional behavior
    let task_vec = vec![
        create_test_task(None), // Will become task_0
        create_test_task(None), // Will become task_1
        create_test_task(None), // Will become task_2
        create_test_task(None), // Will become task_3
    ];

    let collection = TaskCollection::Sequential(task_vec);
    let result = strategy.process_group("test", &collection, vec![]).unwrap();
    let task_names: Vec<String> = result
        .iter()
        .filter(|t| !t.is_barrier)
        .map(|t| t.name.clone())
        .collect();

    // Should be in positional order with auto-generated names
    assert_eq!(
        task_names,
        vec![
            "task_0".to_string(),
            "task_1".to_string(),
            "task_2".to_string(),
            "task_3".to_string()
        ]
    );
}

#[test]
fn test_parallel_vs_sequential_naming_behavior() {
    // Test that parallel tasks preserve original names while sequential tasks get auto-generated names
    let parallel_strategy = ParallelStrategy;
    let sequential_strategy = SequentialStrategy;

    // Create tasks with specific names
    let mut parallel_tasks = IndexMap::new();
    parallel_tasks.insert("alpha".to_string(), create_test_task(None));
    parallel_tasks.insert("beta".to_string(), create_test_task(None));

    // For parallel execution
    let parallel_collection = TaskCollection::Parallel(parallel_tasks.clone());
    let parallel_result = parallel_strategy
        .process_group("test", &parallel_collection, vec![])
        .unwrap();
    let parallel_names: Vec<String> = parallel_result
        .iter()
        .filter(|t| !t.is_barrier)
        .map(|t| t.name.clone())
        .collect();

    // For sequential execution - convert to Vec
    let task_vec: Vec<TaskNode> = parallel_tasks.values().cloned().collect();
    let sequential_collection = TaskCollection::Sequential(task_vec);
    let sequential_result = sequential_strategy
        .process_group("test", &sequential_collection, vec![])
        .unwrap();
    let sequential_names: Vec<String> = sequential_result
        .iter()
        .filter(|t| !t.is_barrier)
        .map(|t| t.name.clone())
        .collect();

    // Parallel should preserve original names
    assert_eq!(
        parallel_names,
        vec!["alpha".to_string(), "beta".to_string()]
    );

    // Sequential should have auto-generated names
    assert_eq!(
        sequential_names,
        vec!["task_0".to_string(), "task_1".to_string()]
    );
}

#[test]
fn test_flattened_task_fields() {
    let strategy = GroupStrategy;
    let mut tasks = IndexMap::new();
    tasks.insert(
        "task1".to_string(),
        create_test_task(Some(vec!["dep1".to_string()])),
    );

    let collection = TaskCollection::Parallel(tasks);
    let result = strategy
        .process_group("test", &collection, vec!["parent".to_string()])
        .unwrap();
    let task = &result[0];

    // Test accessing the fields to ensure they are used
    assert_eq!(task.id, "parent.test:task1");
    assert_eq!(task.group_path, vec!["parent", "test"]);
    assert_eq!(task.name, "task1");
    assert!(!task.dependencies.is_empty());
    assert!(!task.is_barrier);

    // Ensure we can access the node
    match &task.node {
        TaskNode::Task(_) => {} // Expected
        TaskNode::Group { .. } => panic!("Expected task node, got group"),
    }

    // Test barrier task
    let barrier = super::create_barrier_task(
        "barrier_id".to_string(),
        vec!["group".to_string()],
        vec!["dep".to_string()],
    );
    assert_eq!(barrier.id, "barrier_id");
    assert_eq!(barrier.group_path, vec!["group"]);
    assert_eq!(barrier.name, "__barrier__");
    assert!(!barrier.dependencies.is_empty());
    assert!(barrier.is_barrier);
}
