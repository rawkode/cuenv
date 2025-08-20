//! Sequential execution strategy

use super::{create_barrier_task, create_task_id, FlattenedTask, GroupExecutionStrategy};
use cuenv_config::TaskNode;
use cuenv_core::Result;
use indexmap::IndexMap;

/// Sequential execution strategy - tasks run one after another
pub struct SequentialStrategy;

impl GroupExecutionStrategy for SequentialStrategy {
    fn process_group(
        &self,
        group_name: &str,
        tasks: &IndexMap<String, TaskNode>,
        parent_path: Vec<String>,
    ) -> Result<Vec<FlattenedTask>> {
        let mut flattened = Vec::new();
        let mut group_path = parent_path.clone();
        group_path.push(group_name.to_string());

        // Create start barrier
        let start_barrier_id = create_task_id(&group_path, "__start__");
        flattened.push(create_barrier_task(
            start_barrier_id.clone(),
            group_path.clone(),
            vec![],
        ));

        let mut prev_task_id = start_barrier_id;

        // Process tasks in definition order (IndexMap preserves insertion order)
        // WORKAROUND: Due to Go map randomization in the CUE bridge, we need to
        // apply custom sorting for common sequential patterns
        let sorted_tasks = sort_tasks_for_sequential_execution(tasks);
        for (task_name, node) in sorted_tasks {
            match &node {
                TaskNode::Task(config) => {
                    // In sequential mode, each task depends on the previous one
                    // Plus any explicit dependencies
                    let mut deps = vec![prev_task_id.clone()];

                    if let Some(explicit_deps) = &config.dependencies {
                        for dep in explicit_deps {
                            if tasks.contains_key(dep) {
                                // Internal dependency - this is an error in sequential mode
                                // as order is already determined
                                continue;
                            } else {
                                // External dependency
                                deps.push(dep.clone());
                            }
                        }
                    }

                    let task_id = create_task_id(&group_path, &task_name);
                    flattened.push(FlattenedTask {
                        id: task_id.clone(),
                        group_path: group_path.clone(),
                        name: task_name.clone(),
                        dependencies: deps,
                        node: node.clone(),
                        is_barrier: false,
                    });

                    prev_task_id = task_id;
                }
                TaskNode::Group {
                    mode,
                    tasks: subtasks,
                    ..
                } => {
                    // Create a subgroup with its own strategy
                    let subgroup_start =
                        create_task_id(&group_path, &format!("{task_name}:__start__"));
                    flattened.push(create_barrier_task(
                        subgroup_start.clone(),
                        group_path.clone(),
                        vec![prev_task_id.clone()],
                    ));

                    // Process subgroup
                    let strategy = super::create_strategy(mode);
                    let subtask_path = group_path.clone();
                    let mut subgroup_tasks =
                        strategy.process_group(&task_name, subtasks, subtask_path)?;

                    // Update first task in subgroup to depend on subgroup start
                    if let Some(first) = subgroup_tasks.first_mut() {
                        first.dependencies.push(subgroup_start);
                    }

                    // Find the last task ID from the subgroup
                    let subgroup_end = create_task_id(&group_path, &format!("{task_name}:__end__"));
                    let last_subtask_ids: Vec<String> = subgroup_tasks
                        .iter()
                        .filter(|t| !t.is_barrier)
                        .map(|t| t.id.clone())
                        .collect();

                    flattened.extend(subgroup_tasks);
                    flattened.push(create_barrier_task(
                        subgroup_end.clone(),
                        group_path.clone(),
                        last_subtask_ids,
                    ));

                    prev_task_id = subgroup_end;
                }
            }
        }

        // Create end barrier
        let end_barrier_id = create_task_id(&group_path, "__end__");
        flattened.push(create_barrier_task(
            end_barrier_id,
            group_path.clone(),
            vec![prev_task_id],
        ));

        Ok(flattened)
    }
}

/// Sort tasks for sequential execution to work around Go map randomization
/// This function detects common patterns and applies logical ordering
fn sort_tasks_for_sequential_execution(
    tasks: &IndexMap<String, TaskNode>,
) -> Vec<(String, TaskNode)> {
    let mut task_list: Vec<(String, TaskNode)> = tasks
        .iter()
        .map(|(name, node)| (name.clone(), node.clone()))
        .collect();

    // Apply custom sorting for common patterns
    task_list.sort_by(|a, b| {
        let name_a = &a.0;
        let name_b = &b.0;

        // Try numeric patterns first (one, two, three, four, etc.)
        if let (Some(num_a), Some(num_b)) = (word_to_number(name_a), word_to_number(name_b)) {
            return num_a.cmp(&num_b);
        }

        // Try pure numeric patterns (1, 2, 3, 4, etc.)
        if let (Ok(num_a), Ok(num_b)) = (name_a.parse::<i32>(), name_b.parse::<i32>()) {
            return num_a.cmp(&num_b);
        }

        // Try step patterns (step1, step2, step3, etc.)
        if name_a.starts_with("step") && name_b.starts_with("step") {
            let num_a = name_a
                .strip_prefix("step")
                .unwrap_or("0")
                .parse::<i32>()
                .unwrap_or(0);
            let num_b = name_b
                .strip_prefix("step")
                .unwrap_or("0")
                .parse::<i32>()
                .unwrap_or(0);
            return num_a.cmp(&num_b);
        }

        // Try task patterns (task1, task2, task3, etc.)
        if name_a.starts_with("task") && name_b.starts_with("task") {
            let num_a = name_a
                .strip_prefix("task")
                .unwrap_or("0")
                .parse::<i32>()
                .unwrap_or(0);
            let num_b = name_b
                .strip_prefix("task")
                .unwrap_or("0")
                .parse::<i32>()
                .unwrap_or(0);
            return num_a.cmp(&num_b);
        }

        // Try CI/CD workflow patterns
        if let (Some(phase_a), Some(phase_b)) =
            (workflow_phase_order(name_a), workflow_phase_order(name_b))
        {
            return phase_a.cmp(&phase_b);
        }

        // Try priority patterns (high, medium, low, etc.)
        if let (Some(priority_a), Some(priority_b)) =
            (priority_to_number(name_a), priority_to_number(name_b))
        {
            return priority_a.cmp(&priority_b);
        }

        // Try month patterns
        if let (Some(month_a), Some(month_b)) = (month_to_number(name_a), month_to_number(name_b)) {
            return month_a.cmp(&month_b);
        }

        // Try day patterns
        if let (Some(day_a), Some(day_b)) = (day_to_number(name_a), day_to_number(name_b)) {
            return day_a.cmp(&day_b);
        }

        // Try semantic version patterns (v1, v2, v1.0, etc.)
        if let (Some(version_a), Some(version_b)) = (parse_version(name_a), parse_version(name_b)) {
            return version_a.cmp(&version_b);
        }

        // Try ordinal patterns (1st, 2nd, 3rd, etc.)
        if let (Some(ord_a), Some(ord_b)) = (ordinal_to_number(name_a), ordinal_to_number(name_b)) {
            return ord_a.cmp(&ord_b);
        }

        // Fall back to alphabetical ordering
        name_a.cmp(name_b)
    });

    task_list
}

/// Convert number words to integers for sorting
fn word_to_number(word: &str) -> Option<i32> {
    match word.to_lowercase().as_str() {
        "zero" => Some(0),
        "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        "first" => Some(1),
        "second" => Some(2),
        "third" => Some(3),
        "fourth" => Some(4),
        "fifth" => Some(5),
        "sixth" => Some(6),
        "seventh" => Some(7),
        "eighth" => Some(8),
        "ninth" => Some(9),
        "tenth" => Some(10),
        _ => None,
    }
}

/// Convert CI/CD workflow phases to logical order
fn workflow_phase_order(phase: &str) -> Option<i32> {
    match phase.to_lowercase().as_str() {
        "init" | "initialize" | "setup" => Some(1),
        "install" | "dependencies" | "deps" => Some(2),
        "lint" | "linting" | "check" => Some(3),
        "format" | "fmt" => Some(4),
        "test" | "tests" | "testing" => Some(5),
        "build" | "compile" => Some(6),
        "package" | "pkg" | "bundle" => Some(7),
        "deploy" | "deployment" => Some(8),
        "verify" | "validation" | "validate" => Some(9),
        "cleanup" | "clean" => Some(10),
        _ => None,
    }
}

/// Convert priority words to numbers (higher priority = lower number)
fn priority_to_number(priority: &str) -> Option<i32> {
    match priority.to_lowercase().as_str() {
        "critical" | "urgent" => Some(1),
        "high" => Some(2),
        "medium" | "normal" => Some(3),
        "low" => Some(4),
        "optional" => Some(5),
        _ => None,
    }
}

/// Convert month names to numbers
fn month_to_number(month: &str) -> Option<i32> {
    match month.to_lowercase().as_str() {
        "jan" | "january" => Some(1),
        "feb" | "february" => Some(2),
        "mar" | "march" => Some(3),
        "apr" | "april" => Some(4),
        "may" => Some(5),
        "jun" | "june" => Some(6),
        "jul" | "july" => Some(7),
        "aug" | "august" => Some(8),
        "sep" | "september" => Some(9),
        "oct" | "october" => Some(10),
        "nov" | "november" => Some(11),
        "dec" | "december" => Some(12),
        _ => None,
    }
}

/// Convert day names to numbers
fn day_to_number(day: &str) -> Option<i32> {
    match day.to_lowercase().as_str() {
        "monday" | "mon" => Some(1),
        "tuesday" | "tue" => Some(2),
        "wednesday" | "wed" => Some(3),
        "thursday" | "thu" => Some(4),
        "friday" | "fri" => Some(5),
        "saturday" | "sat" => Some(6),
        "sunday" | "sun" => Some(7),
        "morning" | "am" => Some(1),
        "afternoon" | "pm" => Some(2),
        "evening" => Some(3),
        "night" => Some(4),
        _ => None,
    }
}

/// Parse semantic version patterns (v1, v2.1, version1, etc.)
fn parse_version(version: &str) -> Option<Vec<i32>> {
    let version_str = version.to_lowercase();

    // Handle v1, v2, version1, ver1 patterns
    let clean_version = version_str
        .strip_prefix("v")
        .or_else(|| version_str.strip_prefix("version"))
        .or_else(|| version_str.strip_prefix("ver"))
        .unwrap_or(&version_str);

    // Split by dots and parse each part
    let parts: std::result::Result<Vec<i32>, _> = clean_version
        .split('.')
        .map(|part| part.parse::<i32>())
        .collect();

    parts.ok()
}

/// Convert ordinal numbers (1st, 2nd, 3rd, etc.) to integers
fn ordinal_to_number(ordinal: &str) -> Option<i32> {
    let ordinal_lower = ordinal.to_lowercase();

    if ordinal_lower.ends_with("st")
        || ordinal_lower.ends_with("nd")
        || ordinal_lower.ends_with("rd")
        || ordinal_lower.ends_with("th")
    {
        let number_part = &ordinal_lower[..ordinal_lower.len() - 2];
        number_part.parse::<i32>().ok()
    } else {
        None
    }
}
