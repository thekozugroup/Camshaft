use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::plan::{load_plan, save_plan};

pub fn remove_task(id: &str) -> Result<()> {
    let mut plan = load_plan()?;
    let project = &mut plan.project;

    if !project.activities.contains_key(id) {
        return Err(CamshaftError::TaskNotFound(id.to_string()));
    }

    project.activities.shift_remove(id);

    let deps_before = project.dependencies.len();
    project
        .dependencies
        .retain(|d| d.predecessor_id != id && d.successor_id != id);
    let deps_removed = deps_before - project.dependencies.len();

    let assigns_before = project.resource_assignments.len();
    project
        .resource_assignments
        .retain(|a| a.activity_id != id);
    let assigns_removed = assigns_before - project.resource_assignments.len();

    save_plan(&plan)?;

    let output = json!({
        "status": "removed",
        "type": "task",
        "id": id,
        "also_removed": {
            "dependencies": deps_removed,
            "assignments": assigns_removed
        }
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}

pub fn remove_dep(from: &str, to: &str) -> Result<()> {
    let mut plan = load_plan()?;
    let project = &mut plan.project;

    let initial_len = project.dependencies.len();
    project
        .dependencies
        .retain(|d| !(d.predecessor_id == from && d.successor_id == to));

    if project.dependencies.len() == initial_len {
        return Err(CamshaftError::ValidationFailed(format!(
            "Dependency {} -> {} not found",
            from, to
        )));
    }

    save_plan(&plan)?;

    let output = json!({
        "status": "removed",
        "type": "dependency",
        "from": from,
        "to": to
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}
