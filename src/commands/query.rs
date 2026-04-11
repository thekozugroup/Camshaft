use std::collections::{HashMap, HashSet};

use gantt_ml::cpm::CpmEngine;
use gantt_ml::model::types::Duration;
use gantt_ml::Project;
use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::plan::load_plan;

/// Compute parallel groups from CPM results (same logic as optimize.rs).
fn compute_parallel_groups(
    cpm: &gantt_ml::cpm::CpmResult,
    project: &Project,
) -> Vec<(usize, Vec<String>, f64)> {
    let mut start_groups: HashMap<i64, Vec<String>> = HashMap::new();
    for (id, &(early_start, _)) in &cpm.early_dates {
        let key = (early_start * 1000.0) as i64;
        start_groups.entry(key).or_default().push(id.clone());
    }

    let dep_set: HashSet<(&str, &str)> = project
        .dependencies
        .iter()
        .map(|d| (d.predecessor_id.as_str(), d.successor_id.as_str()))
        .collect();

    let mut groups: Vec<(f64, Vec<String>)> = Vec::new();
    for (_, tasks) in &start_groups {
        let task_set: HashSet<&str> = tasks.iter().map(|s| s.as_str()).collect();
        let valid: Vec<String> = tasks
            .iter()
            .filter(|t| {
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

    groups.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    groups
        .into_iter()
        .enumerate()
        .map(|(i, (es, tasks))| (i + 1, tasks, es))
        .collect()
}

/// Helper: load plan and run CPM, returning both.
fn load_and_calculate() -> Result<(crate::plan::CamshaftFile, gantt_ml::cpm::CpmResult)> {
    let plan = load_plan()?;
    let cpm = CpmEngine::calculate(&plan.project)
        .map_err(|e| CamshaftError::GanttMl(e.to_string()))?;
    Ok((plan, cpm))
}

/// Print the critical path and project duration.
pub fn critical_path() -> Result<()> {
    let (_plan, cpm) = load_and_calculate()?;

    let output = json!({
        "critical_path": cpm.critical_path,
        "project_duration": cpm.project_duration,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    Ok(())
}

/// Print status of each activity with CPM-computed fields.
pub fn status() -> Result<()> {
    let (plan, cpm) = load_and_calculate()?;

    let activities: Vec<serde_json::Value> = plan
        .project
        .activities
        .iter()
        .map(|(id, act)| {
            let (es, ef) = cpm.early_dates.get(id).copied().unwrap_or((0.0, 0.0));
            let tf = cpm.total_float.get(id).copied().unwrap_or(0.0);
            let ff = cpm.free_float.get(id).copied().unwrap_or(0.0);

            json!({
                "id": id,
                "name": act.name,
                "duration": act.original_duration.days,
                "early_start": es,
                "early_finish": ef,
                "total_float": tf,
                "free_float": ff,
                "is_critical": tf == 0.0,
            })
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&activities).unwrap());
    Ok(())
}

/// Print tasks that are bottlenecks (total float == 0).
pub fn bottlenecks() -> Result<()> {
    let (plan, cpm) = load_and_calculate()?;

    let bottleneck_list: Vec<serde_json::Value> = cpm
        .total_float
        .iter()
        .filter(|(_, &f)| f == 0.0)
        .filter_map(|(id, _)| {
            plan.project.activities.get(id).map(|act| {
                json!({
                    "id": id,
                    "name": act.name,
                    "duration": act.original_duration.days,
                })
            })
        })
        .collect();

    let output = json!({
        "bottlenecks": bottleneck_list,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    Ok(())
}

/// Print parallel groups.
pub fn parallel() -> Result<()> {
    let (plan, cpm) = load_and_calculate()?;

    let groups = compute_parallel_groups(&cpm, &plan.project);

    let groups_json: Vec<serde_json::Value> = groups
        .iter()
        .map(|(num, tasks, es)| {
            json!({
                "group": num,
                "tasks": tasks,
                "earliest_start": es,
            })
        })
        .collect();

    let output = json!({
        "parallel_groups": groups_json,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    Ok(())
}

/// What-if analysis: compare original vs modified duration for a task.
pub fn what_if(task_id: &str, new_duration: f64) -> Result<()> {
    let (plan, cpm_before) = load_and_calculate()?;

    // Verify the task exists
    if !plan.project.activities.contains_key(task_id) {
        return Err(CamshaftError::TaskNotFound(task_id.to_string()));
    }

    let original_duration = plan.project.activities[task_id].original_duration.days;

    // Clone the project and modify the task's duration
    let mut modified_project = plan.project.clone();
    if let Some(activity) = modified_project.activities.get_mut(task_id) {
        activity.original_duration = Duration::new(new_duration);
        activity.remaining_duration = Duration::new(new_duration);
    }

    let cpm_after = CpmEngine::calculate(&modified_project)
        .map_err(|e| CamshaftError::GanttMl(e.to_string()))?;

    let impact = cpm_after.project_duration - cpm_before.project_duration;

    let output = json!({
        "task": task_id,
        "original_duration": original_duration,
        "new_duration": new_duration,
        "before": {
            "project_duration": cpm_before.project_duration,
            "critical_path": cpm_before.critical_path,
        },
        "after": {
            "project_duration": cpm_after.project_duration,
            "critical_path": cpm_after.critical_path,
        },
        "impact": impact,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    Ok(())
}

/// Suggest execution order based on parallel groups.
pub fn suggest_order() -> Result<()> {
    let (plan, cpm) = load_and_calculate()?;

    let groups = compute_parallel_groups(&cpm, &plan.project);

    let execution_order: Vec<serde_json::Value> = groups
        .iter()
        .map(|(num, tasks, _)| {
            json!({
                "step": num,
                "tasks": tasks,
                "parallel": tasks.len() > 1,
            })
        })
        .collect();

    let output = json!({
        "execution_order": execution_order,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    Ok(())
}
