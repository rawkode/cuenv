//! Tests for task group execution strategies

use super::*;
use cuenv_config::{TaskConfig, TaskGroupMode, TaskNode};
use std::collections::HashMap;

fn create_test_task(dependencies: Option<Vec<String>>) -> TaskNode {
    TaskNode::Task(Box::new(TaskConfig {
        command: Some("echo test".to_string()),
        dependencies,
        ..Default::default()
    }))
}

fn create_test_group(mode: TaskGroupMode, tasks: HashMap<String, TaskNode>) -> TaskNode {
    TaskNode::Group {
        mode,
        tasks,
        description: None,
    }
}

#[test]
fn test_parallel_strategy_creates_barriers() {
    let strategy = ParallelStrategy;
    let mut tasks = HashMap::new();
    tasks.insert("task1".to_string(), create_test_task(None));
    tasks.insert("task2".to_string(), create_test_task(None));

    let result = strategy.process_group("test", &tasks, vec![]).unwrap();

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
    let mut tasks = HashMap::new();
    tasks.insert("task1".to_string(), create_test_task(None));
    tasks.insert("task2".to_string(), create_test_task(None));

    let result = strategy.process_group("test", &tasks, vec![]).unwrap();

    // Should have start barrier, 2 tasks, end barrier = 4 items
    assert_eq!(result.len(), 4);

    // Find the regular tasks (non-barrier)
    let regular_tasks: Vec<_> = result.iter().filter(|t| !t.is_barrier).collect();
    assert_eq!(regular_tasks.len(), 2);

    // Tasks should be in deterministic order (BTreeMap ordering)
    let task_names: Vec<String> = regular_tasks.iter().map(|t| t.name.clone()).collect();
    assert_eq!(task_names, vec!["task1".to_string(), "task2".to_string()]);
}

#[test]
fn test_workflow_strategy_respects_explicit_deps() {
    let strategy = WorkflowStrategy;
    let mut tasks = HashMap::new();
    tasks.insert("task1".to_string(), create_test_task(None));
    tasks.insert(
        "task2".to_string(),
        create_test_task(Some(vec!["task1".to_string()])),
    );

    let result = strategy.process_group("test", &tasks, vec![]).unwrap();

    // Find task2 and verify it depends on task1
    let task2 = result.iter().find(|t| t.name == "task2").unwrap();
    assert!(task2.dependencies.contains(&"test:task1".to_string()));
}

#[test]
fn test_group_strategy_preserves_tasks() {
    let strategy = GroupStrategy;
    let mut tasks = HashMap::new();
    tasks.insert("task1".to_string(), create_test_task(None));
    tasks.insert("task2".to_string(), create_test_task(None));

    let result = strategy.process_group("test", &tasks, vec![]).unwrap();

    // Group strategy should just preserve tasks without modification
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|t| !t.is_barrier));
}

#[test]
fn test_nested_groups_execution() {
    let strategy = ParallelStrategy;
    let mut tasks = HashMap::new();

    // Create a nested group
    let mut nested_tasks = HashMap::new();
    nested_tasks.insert("nested1".to_string(), create_test_task(None));
    nested_tasks.insert("nested2".to_string(), create_test_task(None));

    tasks.insert("simple".to_string(), create_test_task(None));
    tasks.insert(
        "group".to_string(),
        create_test_group(TaskGroupMode::Sequential, nested_tasks),
    );

    let result = strategy.process_group("test", &tasks, vec![]).unwrap();

    // Should handle nested groups
    assert!(result.len() > 2); // More than just the two top-level items due to nesting
}

#[test]
fn test_create_strategy_returns_correct_types() {
    // Test that create_strategy returns the expected strategy types
    let workflow = create_strategy(&TaskGroupMode::Workflow);
    let sequential = create_strategy(&TaskGroupMode::Sequential);
    let parallel = create_strategy(&TaskGroupMode::Parallel);
    let group = create_strategy(&TaskGroupMode::Group);

    // We can't easily test the concrete types due to trait objects,
    // but we can test that they process groups differently
    let mut tasks = HashMap::new();
    tasks.insert("task1".to_string(), create_test_task(None));

    let workflow_result = workflow.process_group("test", &tasks, vec![]).unwrap();
    let sequential_result = sequential.process_group("test", &tasks, vec![]).unwrap();
    let parallel_result = parallel.process_group("test", &tasks, vec![]).unwrap();
    let group_result = group.process_group("test", &tasks, vec![]).unwrap();

    // Sequential and parallel should add barriers, group shouldn't
    assert!(sequential_result.len() > group_result.len());
    assert!(parallel_result.len() > group_result.len());
    assert_eq!(workflow_result.len(), group_result.len()); // Workflow doesn't add barriers for single task
}

#[test]
fn test_sequential_ordering_deterministic() {
    let strategy = SequentialStrategy;
    let mut tasks = HashMap::new();

    // Insert in different order multiple times to test determinism
    for _ in 0..5 {
        tasks.clear();
        tasks.insert("zzz".to_string(), create_test_task(None));
        tasks.insert("aaa".to_string(), create_test_task(None));
        tasks.insert("mmm".to_string(), create_test_task(None));

        let result = strategy.process_group("test", &tasks, vec![]).unwrap();
        let task_names: Vec<String> = result
            .iter()
            .filter(|t| !t.is_barrier)
            .map(|t| t.name.clone())
            .collect();

        // Should always be in alphabetical order due to BTreeMap
        assert_eq!(
            task_names,
            vec!["aaa".to_string(), "mmm".to_string(), "zzz".to_string()]
        );
    }
}

#[test]
fn test_flattened_task_fields() {
    let strategy = GroupStrategy;
    let mut tasks = HashMap::new();
    tasks.insert(
        "task1".to_string(),
        create_test_task(Some(vec!["dep1".to_string()])),
    );

    let result = strategy
        .process_group("test", &tasks, vec!["parent".to_string()])
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
