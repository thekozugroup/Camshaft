use std::collections::{HashMap, HashSet, VecDeque};

use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::plan::load_plan;

pub fn run() -> Result<()> {
    let plan = load_plan()?;
    let project = &plan.project;

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    let task_ids: HashSet<&str> = project.activities.keys().map(|s| s.as_str()).collect();
    let task_count = project.activities.len();
    let dep_count = project.dependencies.len();

    // 1. Empty plan check
    if task_count == 0 {
        warnings.push("Plan has no activities".to_string());
    }

    // 2. Orphan check — tasks with no dependencies (neither predecessor nor successor)
    if task_count > 0 {
        let mut referenced: HashSet<&str> = HashSet::new();
        for dep in &project.dependencies {
            referenced.insert(&dep.predecessor_id);
            referenced.insert(&dep.successor_id);
        }
        for id in &task_ids {
            if !referenced.contains(id) {
                warnings.push(format!("Task '{}' has no dependencies (orphan)", id));
            }
        }
    }

    // 3. Missing reference check
    for dep in &project.dependencies {
        if !task_ids.contains(dep.predecessor_id.as_str()) {
            errors.push(format!(
                "Dependency references nonexistent task: '{}'",
                dep.predecessor_id
            ));
        }
        if !task_ids.contains(dep.successor_id.as_str()) {
            errors.push(format!(
                "Dependency references nonexistent task: '{}'",
                dep.successor_id
            ));
        }
    }

    // 4. Duplicate dependency check
    {
        let mut seen: HashSet<(&str, &str)> = HashSet::new();
        for dep in &project.dependencies {
            let pair = (dep.predecessor_id.as_str(), dep.successor_id.as_str());
            if !seen.insert(pair) {
                warnings.push(format!(
                    "Duplicate dependency: {} -> {}",
                    dep.predecessor_id, dep.successor_id
                ));
            }
        }
    }

    // 5. Cycle detection — Kahn's algorithm (topological sort)
    if !project.dependencies.is_empty() {
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

        for id in &task_ids {
            in_degree.entry(id).or_insert(0);
            adj.entry(id).or_default();
        }

        for dep in &project.dependencies {
            let pred = dep.predecessor_id.as_str();
            let succ = dep.successor_id.as_str();
            // Only process edges between existing tasks
            if task_ids.contains(pred) && task_ids.contains(succ) {
                adj.entry(pred).or_default().push(succ);
                *in_degree.entry(succ).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<&str> = VecDeque::new();
        for (&node, &deg) in &in_degree {
            if deg == 0 {
                queue.push_back(node);
            }
        }

        let mut visited = 0usize;
        while let Some(node) = queue.pop_front() {
            visited += 1;
            if let Some(neighbors) = adj.get(node) {
                for &neighbor in neighbors {
                    if let Some(deg) = in_degree.get_mut(neighbor) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
        }

        if visited < task_ids.len() {
            let cycle_nodes: Vec<&str> = in_degree
                .iter()
                .filter(|(_, &deg)| deg > 0)
                .map(|(&node, _)| node)
                .collect();
            errors.push(format!(
                "Cycle detected involving tasks: {}",
                cycle_nodes.join(", ")
            ));
        }
    }

    let valid = errors.is_empty();

    let output = json!({
        "valid": valid,
        "errors": errors,
        "warnings": warnings,
        "task_count": task_count,
        "dependency_count": dep_count
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    if valid {
        Ok(())
    } else {
        Err(CamshaftError::ValidationFailed(errors.join("; ")))
    }
}
