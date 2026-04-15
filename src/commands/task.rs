use gantt_ml::model::types::ActivityStatus;
use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::plan::{load_plan, save_plan};

/// Mark a task as completed.
pub fn complete(id: &str) -> Result<()> {
    let mut file = load_plan()?;

    if !file.project.activities.contains_key(id) {
        return Err(CamshaftError::TaskNotFound(id.to_string()));
    }

    let name = {
        let activity = file.project.activities.get_mut(id).unwrap();
        activity.status = ActivityStatus::Completed;
        activity.percent_complete = 100.0;
        activity.name.clone()
    };

    save_plan(&file)?;

    let output = json!({
        "status": "completed",
        "id": id,
        "name": name,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}

/// Reopen a completed task (back to InProgress, 0% complete).
pub fn reopen(id: &str) -> Result<()> {
    let mut file = load_plan()?;

    if !file.project.activities.contains_key(id) {
        return Err(CamshaftError::TaskNotFound(id.to_string()));
    }

    let name = {
        let activity = file.project.activities.get_mut(id).unwrap();
        activity.status = ActivityStatus::InProgress;
        activity.percent_complete = 0.0;
        activity.name.clone()
    };

    save_plan(&file)?;

    let output = json!({
        "status": "reopened",
        "id": id,
        "name": name,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}

/// Show a single task's status details.
pub fn status(id: &str) -> Result<()> {
    let file = load_plan()?;

    let activity = file
        .project
        .activities
        .get(id)
        .ok_or_else(|| CamshaftError::TaskNotFound(id.to_string()))?;

    let status_str = match activity.status {
        ActivityStatus::NotStarted => "not_started",
        ActivityStatus::InProgress => "in_progress",
        ActivityStatus::Completed => "completed",
        ActivityStatus::Suspended => "suspended",
    };

    let output = json!({
        "id": id,
        "name": activity.name,
        "status": status_str,
        "percent_complete": activity.percent_complete,
        "duration": activity.original_duration.days,
        "remaining_duration": activity.remaining_duration.days,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}
