use chrono::Local;
use gantt_ml::agentic::*;
use serde_json::json;

use crate::error::Result;
use crate::plan::{load_plan, CamshaftFile};

fn activities_to_smart_tasks(plan: &CamshaftFile) -> Vec<SmartTask> {
    let project = &plan.project;

    // Build a lookup: successor_id -> list of predecessor_ids
    let mut blocked_by_map: std::collections::HashMap<&str, Vec<String>> =
        std::collections::HashMap::new();
    for dep in &project.dependencies {
        blocked_by_map
            .entry(dep.successor_id.as_str())
            .or_default()
            .push(dep.predecessor_id.clone());
    }

    project
        .activities
        .iter()
        .map(|(id, activity)| {
            let mut task = SmartTask::new(id.clone(), activity.name.clone());
            // In sprint mode, durations represent hours
            task.estimated_hours = Some(activity.original_duration.days);
            task.blocked_by = blocked_by_map
                .get(id.as_str())
                .cloned()
                .unwrap_or_default();
            task
        })
        .collect()
}

pub fn plan(capacity: f64, _hours_per_day: f64) -> Result<()> {
    let plan = load_plan()?;
    let tasks = activities_to_smart_tasks(&plan);

    let sprint = TaskScheduler::auto_sprint_plan(&tasks, 10, capacity);
    let prioritized_order = TaskScheduler::prioritize(&tasks);

    let allocated = sprint.allocated_hours(&tasks);
    let remaining = sprint.remaining_capacity(&tasks);

    let output = json!({
        "sprint": {
            "id": sprint.id,
            "name": sprint.name,
            "start": sprint.start_date.to_string(),
            "end": sprint.end_date.to_string(),
            "capacity": sprint.capacity_hours,
        },
        "allocated_hours": allocated,
        "remaining_capacity": remaining,
        "task_order": sprint.tasks,
        "prioritized_order": prioritized_order,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}

pub fn suggest() -> Result<()> {
    let plan = load_plan()?;
    let tasks = activities_to_smart_tasks(&plan);

    let suggestion =
        TaskScheduler::suggest_schedule(&tasks, 8.0, Local::now().date_naive());

    let daily_plan: Vec<_> = suggestion
        .daily_plan
        .iter()
        .map(|day| {
            json!({
                "date": day.date.to_string(),
                "tasks": day.tasks.iter().map(|(id, hours)| {
                    json!({ "task_id": id, "hours": hours })
                }).collect::<Vec<_>>(),
                "total_hours": day.total_hours,
            })
        })
        .collect();

    let output = json!({
        "daily_plan": daily_plan,
        "unscheduled": suggestion.unscheduled,
        "warnings": suggestion.warnings,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}

pub fn overcommit_check(hours_per_day: f64) -> Result<()> {
    let plan = load_plan()?;
    let tasks = activities_to_smart_tasks(&plan);

    let warnings = TaskScheduler::detect_overcommitment(&tasks, hours_per_day);
    let overcommitted = !warnings.is_empty();

    let warning_list: Vec<_> = warnings
        .iter()
        .map(|w| {
            json!({
                "date": w.date.to_string(),
                "scheduled_hours": w.scheduled_hours,
                "available_hours": w.available_hours,
                "excess_hours": w.excess_hours,
                "task_ids": w.task_ids,
            })
        })
        .collect();

    let output = json!({
        "overcommitted": overcommitted,
        "warnings": warning_list,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}
