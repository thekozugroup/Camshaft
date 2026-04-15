use std::path::{Component, Path};

use gantt_ml::diff::{compare, FieldDiff};
use serde_json::{json, Value};

use crate::error::{CamshaftError, Result};
use crate::plan::{load_plan, load_plan_from};

pub fn run(baseline_path: &str) -> Result<()> {
    // Security: baseline path must be relative and must not contain `..`
    let path = Path::new(baseline_path);
    if path.is_absolute() {
        return Err(CamshaftError::ValidationFailed(
            "Baseline path must be relative, not absolute.".to_string(),
        ));
    }
    for component in path.components() {
        if let Component::ParentDir = component {
            return Err(CamshaftError::ValidationFailed(
                "Baseline path must not contain '..' components.".to_string(),
            ));
        }
    }
    if !path.exists() {
        return Err(CamshaftError::ValidationFailed(format!(
            "Baseline file not found: {}",
            baseline_path
        )));
    }

    let current_plan = load_plan()?;
    let baseline_plan = load_plan_from(path)?;

    let diff = compare(&baseline_plan.project, &current_plan.project);

    // --- Build changes list ---
    let mut changes: Vec<Value> = Vec::new();

    for id in &diff.activities_added {
        let name = current_plan
            .project
            .activities
            .get(id)
            .map(|a| a.name.clone())
            .unwrap_or_else(|| id.clone());
        changes.push(json!({
            "type": "task_added",
            "task_id": id,
            "name": name,
        }));
    }

    for id in &diff.activities_removed {
        let name = baseline_plan
            .project
            .activities
            .get(id)
            .map(|a| a.name.clone())
            .unwrap_or_else(|| id.clone());
        changes.push(json!({
            "type": "task_removed",
            "task_id": id,
            "name": name,
        }));
    }

    for modified in &diff.activities_modified {
        for change in &modified.changes {
            match change {
                FieldDiff::DurationChanged { old, new } => {
                    changes.push(json!({
                        "type": "task_duration_changed",
                        "task_id": modified.activity_id,
                        "from": old,
                        "to": new,
                    }));
                }
                FieldDiff::StartChanged { old, new } => {
                    changes.push(json!({
                        "type": "task_start_changed",
                        "task_id": modified.activity_id,
                        "from": old.map(|d| d.to_string()),
                        "to": new.map(|d| d.to_string()),
                    }));
                }
                FieldDiff::FinishChanged { old, new } => {
                    changes.push(json!({
                        "type": "task_finish_changed",
                        "task_id": modified.activity_id,
                        "from": old.map(|d| d.to_string()),
                        "to": new.map(|d| d.to_string()),
                    }));
                }
                FieldDiff::FloatChanged { old, new } => {
                    changes.push(json!({
                        "type": "task_float_changed",
                        "task_id": modified.activity_id,
                        "from": old,
                        "to": new,
                    }));
                }
                FieldDiff::StatusChanged { old, new } => {
                    changes.push(json!({
                        "type": "task_status_changed",
                        "task_id": modified.activity_id,
                        "from": format!("{:?}", old),
                        "to": format!("{:?}", new),
                    }));
                }
                FieldDiff::NameChanged { old, new } => {
                    changes.push(json!({
                        "type": "task_name_changed",
                        "task_id": modified.activity_id,
                        "from": old,
                        "to": new,
                    }));
                }
            }
        }
    }

    for dep in &diff.dependencies_added {
        changes.push(json!({
            "type": "dependency_added",
            "from": dep.predecessor_id,
            "to": dep.successor_id,
        }));
    }

    for dep in &diff.dependencies_removed {
        changes.push(json!({
            "type": "dependency_removed",
            "from": dep.predecessor_id,
            "to": dep.successor_id,
        }));
    }

    if diff.summary.critical_path_affected {
        let delta = diff.summary.duration_delta;
        let description = if delta.abs() < f64::EPSILON {
            "Critical path composition changed (duration unchanged)".to_string()
        } else if delta > 0.0 {
            format!("Critical path is now {:.1} days longer", delta)
        } else {
            format!("Critical path is now {:.1} days shorter", -delta)
        };
        changes.push(json!({
            "type": "critical_path_impact",
            "description": description,
        }));
    }

    // Summary counts
    let modified_count = diff.activities_modified.len();

    let output = json!({
        "baseline": baseline_path,
        "current": ".camshaft/plan.json",
        "summary": {
            "tasks_added": diff.activities_added.len(),
            "tasks_removed": diff.activities_removed.len(),
            "tasks_modified": modified_count,
            "dependencies_added": diff.dependencies_added.len(),
            "dependencies_removed": diff.dependencies_removed.len(),
            "duration_delta_days": round1(diff.summary.duration_delta),
            "critical_path_changed": diff.summary.critical_path_affected,
            "total_changes": diff.summary.total_changes,
        },
        "changes": changes,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    Ok(())
}

fn round1(v: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    (v * 10.0).round() / 10.0
}
