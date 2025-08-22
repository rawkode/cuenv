use cuenv_core::{Error, Result};
use std::collections::{HashMap, VecDeque};

/// Perform topological sort to determine execution levels
pub fn topological_sort(dependencies: &HashMap<String, Vec<String>>) -> Result<Vec<Vec<String>>> {
    let mut in_degree = HashMap::with_capacity(dependencies.len());
    let mut graph = HashMap::with_capacity(dependencies.len());

    // Initialize in-degree count and adjacency list
    for (task, deps) in dependencies {
        in_degree.entry(task.clone()).or_insert(0);
        graph.entry(task.clone()).or_insert_with(Vec::new);

        for dep in deps {
            *in_degree.entry(dep.clone()).or_insert(0) += 0; // Ensure dep is in map
            graph
                .entry(dep.clone())
                .or_insert_with(Vec::new)
                .push(task.clone());
            if let Some(degree) = in_degree.get_mut(task) {
                *degree += 1;
            } else {
                return Err(Error::configuration(format!(
                    "Task '{task}' not found in in-degree map"
                )));
            }
        }
    }

    let mut levels = Vec::with_capacity(dependencies.len() / 2); // Estimate
    let mut queue: VecDeque<String> = in_degree
        .iter()
        .filter(|(_, &degree)| degree == 0)
        .map(|(task, _)| task.clone())
        .collect();

    while !queue.is_empty() {
        let current_level: Vec<String> = queue.drain(..).collect();

        if current_level.is_empty() {
            break;
        }

        for task in &current_level {
            if let Some(dependents) = graph.get(task) {
                for dependent in dependents {
                    if let Some(degree) = in_degree.get_mut(dependent) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }
        }

        levels.push(current_level);
    }

    // Check for remaining tasks (would indicate circular dependencies)
    let processed_count: usize = levels.iter().map(|level| level.len()).sum();
    let total_tasks = in_degree.len(); // Use actual count of all tasks in graph
    if processed_count != total_tasks {
        return Err(Error::configuration(
            "Circular dependency detected in task graph".to_string(),
        ));
    }

    Ok(levels)
}
