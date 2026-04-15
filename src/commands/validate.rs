use std::collections::{HashMap, HashSet, VecDeque};

use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::plan::load_plan;

/// A single validation issue with severity, message, and the tasks it affects.
struct Issue {
    severity: &'static str, // "error" | "warning" | "info"
    message: String,
    affected_tasks: Vec<String>,
}

pub fn run() -> Result<()> {
    let plan = load_plan()?;
    let project = &plan.project;

    let mut issues: Vec<Issue> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    let task_ids: HashSet<&str> = project.activities.keys().map(|s| s.as_str()).collect();
    let task_count = project.activities.len();
    let dep_count = project.dependencies.len();

    // 1. Empty plan check
    if task_count == 0 {
        let m = "Plan has no activities".to_string();
        warnings.push(m.clone());
        issues.push(Issue {
            severity: "warning",
            message: m,
            affected_tasks: vec![],
        });
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
                let m = format!("Task '{}' has no dependencies (orphan)", id);
                warnings.push(m.clone());
                issues.push(Issue {
                    severity: "warning",
                    message: m,
                    affected_tasks: vec![id.to_string()],
                });
            }
        }
    }

    // 3. Missing reference check
    for dep in &project.dependencies {
        if !task_ids.contains(dep.predecessor_id.as_str()) {
            let m = format!(
                "Dependency references nonexistent task: '{}'",
                dep.predecessor_id
            );
            errors.push(m.clone());
            issues.push(Issue {
                severity: "error",
                message: m,
                affected_tasks: vec![dep.predecessor_id.clone()],
            });
        }
        if !task_ids.contains(dep.successor_id.as_str()) {
            let m = format!(
                "Dependency references nonexistent task: '{}'",
                dep.successor_id
            );
            errors.push(m.clone());
            issues.push(Issue {
                severity: "error",
                message: m,
                affected_tasks: vec![dep.successor_id.clone()],
            });
        }
    }

    // 4. Duplicate dependency check
    {
        let mut seen: HashSet<(&str, &str)> = HashSet::new();
        for dep in &project.dependencies {
            let pair = (dep.predecessor_id.as_str(), dep.successor_id.as_str());
            if !seen.insert(pair) {
                let m = format!(
                    "Duplicate dependency: {} -> {}",
                    dep.predecessor_id, dep.successor_id
                );
                warnings.push(m.clone());
                issues.push(Issue {
                    severity: "warning",
                    message: m,
                    affected_tasks: vec![
                        dep.predecessor_id.clone(),
                        dep.successor_id.clone(),
                    ],
                });
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
            // Fall-back: the full SCC reachable set (over-reports).
            let scc_nodes: Vec<String> = in_degree
                .iter()
                .filter(|(_, &deg)| deg > 0)
                .map(|(&node, _)| node.to_string())
                .collect();

            // Preferred: ask GanttML for the minimal cycle. If it can build
            // the graph and finds a cycle, use those task IDs (a chain like
            // R1 -> R2 -> R4 -> R1). Fall back to the SCC set on any error.
            let (cycle_nodes, cycle_chain): (Vec<String>, Option<String>) =
                match gantt_ml::graph::ScheduleGraph::from_project(project) {
                    Ok(graph) => match graph.detect_cycles() {
                        Some(cycle) if !cycle.is_empty() => {
                            let chain = cycle.join(" -> ");
                            // Dedupe for affected_tasks; keep chain form for msg.
                            let mut seen = HashSet::new();
                            let unique: Vec<String> = cycle
                                .into_iter()
                                .filter(|s| seen.insert(s.clone()))
                                .collect();
                            (unique, Some(chain))
                        }
                        _ => (scc_nodes.clone(), None),
                    },
                    Err(err) => {
                        // The error message may already contain the precise
                        // chain (e.g. "... [R1, R2, R4, R1]"). Try to parse it.
                        let raw = err.to_string();
                        if let Some(start) = raw.find('[') {
                            if let Some(end) = raw[start..].find(']') {
                                let inner = &raw[start + 1..start + end];
                                let parsed: Vec<String> = inner
                                    .split(',')
                                    .map(|s| s.trim().trim_matches('"').to_string())
                                    .filter(|s| !s.is_empty() && task_ids.contains(s.as_str()))
                                    .collect();
                                if !parsed.is_empty() {
                                    // Dedupe while preserving the chain order
                                    // for the message.
                                    let chain = parsed.join(" -> ");
                                    let mut seen = HashSet::new();
                                    let unique: Vec<String> = parsed
                                        .into_iter()
                                        .filter(|s| seen.insert(s.clone()))
                                        .collect();
                                    (unique, Some(chain))
                                } else {
                                    (scc_nodes.clone(), None)
                                }
                            } else {
                                (scc_nodes.clone(), None)
                            }
                        } else {
                            (scc_nodes.clone(), None)
                        }
                    }
                };

            let m = if let Some(chain) = cycle_chain {
                format!("Cycle detected: {}", chain)
            } else {
                format!(
                    "Cycle detected involving tasks: {}",
                    cycle_nodes.join(", ")
                )
            };
            errors.push(m.clone());
            issues.push(Issue {
                severity: "error",
                message: m,
                affected_tasks: cycle_nodes,
            });
        }
    }

    let valid = errors.is_empty();
    let can_optimize = valid && task_count > 0;

    let issues_json: Vec<serde_json::Value> = issues
        .iter()
        .map(|i| {
            json!({
                "severity": i.severity,
                "message": i.message,
                "affected_tasks": i.affected_tasks,
            })
        })
        .collect();

    let output = json!({
        "valid": valid,
        "issues": issues_json,
        "errors": errors,
        "warnings": warnings,
        "can_optimize": can_optimize,
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
