use std::collections::{HashMap, VecDeque};

use workaholic::domain::work::WorkTask;

/// Compute a valid topological execution order for a list of `WorkTask`s.
///
/// Returns the task indices in the order they can be started (dependencies
/// first). Tasks with no dependencies appear at the front.
///
/// Returns an error if:
/// - A task references an unknown dependency.
/// - The dependency graph contains a cycle.
pub fn topological_sort(tasks: &[WorkTask]) -> Result<Vec<usize>, String> {
    let n = tasks.len();

    // Map task name → index.
    let name_to_idx: HashMap<&str, usize> =
        tasks.iter().enumerate().map(|(i, t)| (t.name.as_str(), i)).collect();

    // in_degree[i] = number of dependencies not yet satisfied for task i.
    let mut in_degree = vec![0usize; n];
    // adj[i] = tasks that have task i as a dependency (i must run first).
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];

    for (i, task) in tasks.iter().enumerate() {
        for dep_name in &task.depends_on {
            match name_to_idx.get(dep_name.as_str()) {
                Some(&j) => {
                    adj[j].push(i);
                    in_degree[i] += 1;
                }
                None => {
                    return Err(format!(
                        "task '{}' depends on unknown task '{dep_name}'",
                        task.name
                    ));
                }
            }
        }
    }

    // Kahn's algorithm.
    let mut queue: VecDeque<usize> =
        (0..n).filter(|&i| in_degree[i] == 0).collect();

    let mut order = Vec::with_capacity(n);
    while let Some(i) = queue.pop_front() {
        order.push(i);
        for &j in &adj[i] {
            in_degree[j] -= 1;
            if in_degree[j] == 0 {
                queue.push_back(j);
            }
        }
    }

    if order.len() != n {
        return Err("circular dependency detected in workflow task graph".to_string());
    }

    Ok(order)
}

/// Return the set of task indices that are ready to start given the set of
/// already-completed indices.
///
/// A task is ready when all its `depends_on` entries appear in `completed`.
pub fn ready_tasks(tasks: &[WorkTask], completed: &[usize]) -> Vec<usize> {
    let completed_names: std::collections::HashSet<&str> =
        completed.iter().map(|&i| tasks[i].name.as_str()).collect();

    tasks
        .iter()
        .enumerate()
        .filter(|(i, t)| {
            !completed.contains(i) && t.depends_on.iter().all(|d| completed_names.contains(d.as_str()))
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use workaholic::domain::work::WorkTask;

    fn task(name: &str, depends_on: &[&str]) -> WorkTask {
        WorkTask {
            name: name.into(),
            task_ref: name.into(),
            depends_on: depends_on.iter().map(|s| s.to_string()).collect(),
            inputs: serde_json::Value::Null,
            condition: None,
            retry_count: None,
            timeout_seconds: None,
            execution_profile: None,
        }
    }

    #[test]
    fn linear_chain() {
        let tasks = vec![
            task("a", &[]),
            task("b", &["a"]),
            task("c", &["b"]),
        ];
        let order = topological_sort(&tasks).unwrap();
        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn independent_tasks() {
        let tasks = vec![task("a", &[]), task("b", &[]), task("c", &[])];
        let order = topological_sort(&tasks).unwrap();
        // All three have no deps; any order is valid.
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn diamond_dag() {
        // a → b, a → c, b → d, c → d
        let tasks = vec![
            task("a", &[]),
            task("b", &["a"]),
            task("c", &["a"]),
            task("d", &["b", "c"]),
        ];
        let order = topological_sort(&tasks).unwrap();
        assert_eq!(order.len(), 4);
        let a_pos = order.iter().position(|&i| i == 0).unwrap();
        let d_pos = order.iter().position(|&i| i == 3).unwrap();
        assert!(a_pos < d_pos, "a must come before d");
    }

    #[test]
    fn cycle_detected() {
        let tasks = vec![
            task("a", &["b"]),
            task("b", &["a"]),
        ];
        assert!(topological_sort(&tasks).is_err());
    }

    #[test]
    fn unknown_dependency() {
        let tasks = vec![task("a", &["nonexistent"])];
        let err = topological_sort(&tasks).unwrap_err();
        assert!(err.contains("unknown task"));
    }

    #[test]
    fn ready_tasks_progression() {
        let tasks = vec![
            task("a", &[]),
            task("b", &["a"]),
            task("c", &["a"]),
            task("d", &["b", "c"]),
        ];
        // Initially: a is ready
        let ready = ready_tasks(&tasks, &[]);
        assert_eq!(ready, vec![0]);

        // After a completes: b and c are ready
        let mut ready = ready_tasks(&tasks, &[0]);
        ready.sort();
        assert_eq!(ready, vec![1, 2]);

        // After a, b, c: d is ready
        let ready = ready_tasks(&tasks, &[0, 1, 2]);
        assert_eq!(ready, vec![3]);
    }
}
