use std::collections::{HashMap, HashSet};

use chrono::Utc;
use gantt_ml::cpm::CpmEngine;
use gantt_ml::Project;
use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::plan::{load_plan, save_plan};

/// Compute parallel groups from CPM results.
///
/// Groups tasks by their early_start value (tasks sharing the same early_start
/// can potentially run in parallel). For each candidate group, verifies that no
/// task depends on another task within the same group. Returns a sorted vec of
/// (group_number, task_ids, earliest_start).
fn compute_parallel_groups(
    cpm: &gantt_ml::cpm::CpmResult,
    project: &Project,
) -> Vec<(usize, Vec<String>, f64)> {
    // Group task IDs by early_start
    let mut start_groups: HashMap<i64, Vec<String>> = HashMap::new();
    for (id, &(early_start, _)) in &cpm.early_dates {
        // Use integer key (millidays) to avoid float grouping issues
        let key = (early_start * 1000.0) as i64;
        start_groups.entry(key).or_default().push(id.clone());
    }

    // Build a set of direct dependencies for quick lookup
    let dep_set: HashSet<(&str, &str)> = project
        .dependencies
        .iter()
        .map(|d| (d.predecessor_id.as_str(), d.successor_id.as_str()))
        .collect();

    // For each group, filter out tasks that depend on other tasks in the same group
    let mut groups: Vec<(f64, Vec<String>)> = Vec::new();
    for (_, tasks) in &start_groups {
        let task_set: HashSet<&str> = tasks.iter().map(|s| s.as_str()).collect();
        let valid: Vec<String> = tasks
            .iter()
            .filter(|t| {
                // Keep t if no other task in the group is its predecessor
                !task_set.iter().any(|&other| {
                    other != t.as_str() && dep_set.contains(&(other, t.as_str()))
                })
            })
            .cloned()
            .collect();
        if !valid.is_empty() {
            let es = cpm.early_dates[&valid[0]].0;
            groups.push((es, valid));
        }
    }

    // Sort by earliest_start ascending
    groups.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    groups
        .into_iter()
        .enumerate()
        .map(|(i, (es, tasks))| (i + 1, tasks, es))
        .collect()
}

pub fn run(_objective: &str, fast_track: bool, crash: bool) -> Result<()> {
    if fast_track {
        eprintln!("Warning: --fast-track is not yet implemented (future feature).");
    }
    if crash {
        eprintln!("Warning: --crash is not yet implemented (future feature).");
    }

    let mut plan = load_plan()?;

    if plan.project.activities.is_empty() {
        return Err(CamshaftError::OptimizationFailed(
            "No activities in plan. Add tasks before optimizing.".to_string(),
        ));
    }

    let cpm = CpmEngine::calculate(&plan.project)
        .map_err(|e| CamshaftError::GanttMl(e.to_string()))?;

    let parallel_groups = compute_parallel_groups(&cpm, &plan.project);

    // Build bottlenecks: tasks with zero total float
    let bottlenecks: Vec<String> = cpm
        .total_float
        .iter()
        .filter(|(_, &f)| f == 0.0)
        .map(|(id, _)| id.clone())
        .collect();

    // Build suggested order strings
    let suggested_order: Vec<String> = parallel_groups
        .iter()
        .map(|(num, tasks, _)| {
            format!("group{}: {}", num, tasks.join(" || "))
        })
        .collect();

    // Build parallel_groups JSON
    let groups_json: Vec<serde_json::Value> = parallel_groups
        .iter()
        .map(|(num, tasks, es)| {
            json!({
                "group": num,
                "tasks": tasks,
                "earliest_start": es,
            })
        })
        .collect();

    // Update plan metadata
    plan.meta.last_optimized = Some(Utc::now());
    plan.meta.optimization_runs += 1;
    plan.meta.parallelism_groups = parallel_groups
        .iter()
        .map(|(_, tasks, _)| tasks.clone())
        .collect();

    save_plan(&plan)?;

    let output = json!({
        "project_duration": cpm.project_duration,
        "critical_path": cpm.critical_path,
        "parallel_groups": groups_json,
        "total_float": cpm.total_float,
        "bottlenecks": bottlenecks,
        "suggested_order": suggested_order,
        "optimization_moves": [],
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    Ok(())
}
