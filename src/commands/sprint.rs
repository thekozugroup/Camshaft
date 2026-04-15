use chrono::Local;
use gantt_ml::agentic::*;
use serde_json::json;
use std::collections::HashMap;

use crate::error::Result;
use crate::plan::{load_plan, CamshaftFile};

/// Parse a priority string (case-insensitive) into the agentic `TaskPriority` enum.
/// Falls back to `TaskPriority::Medium` for unknown values.
fn parse_priority(s: &str) -> TaskPriority {
    match s.trim().to_lowercase().as_str() {
        "critical" | "crit" | "must" | "must-have" | "must_have" => TaskPriority::Critical,
        "high" | "should" | "should-have" | "should_have" => TaskPriority::High,
        "medium" | "med" | "normal" | "default" => TaskPriority::Medium,
        "low" | "could" | "could-have" | "could_have" | "nice" | "nice-to-have" => {
            TaskPriority::Low
        }
        _ => TaskPriority::Medium,
    }
}

/// Extract a priority from an Activity: first from `custom_fields["priority"]`,
/// then from a bracketed description prefix like `[CRITICAL]`.
fn activity_priority(activity: &gantt_ml::model::activity::Activity) -> TaskPriority {
    if let Some(val) = activity.custom_fields.get("priority") {
        if let Some(s) = val.as_str() {
            return parse_priority(s);
        }
    }
    if let Some(desc) = &activity.description {
        let trimmed = desc.trim_start();
        if let Some(rest) = trimmed.strip_prefix('[') {
            if let Some(end) = rest.find(']') {
                return parse_priority(&rest[..end]);
            }
        }
    }
    TaskPriority::Medium
}

fn activities_to_smart_tasks(plan: &CamshaftFile) -> Vec<SmartTask> {
    let project = &plan.project;

    // Build a lookup: successor_id -> list of predecessor_ids
    let mut blocked_by_map: HashMap<&str, Vec<String>> = HashMap::new();
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
            task.priority = activity_priority(activity);
            task
        })
        .collect()
}

pub fn plan(capacity: f64, _hours_per_day: f64) -> Result<()> {
    let plan = load_plan()?;
    let tasks = activities_to_smart_tasks(&plan);

    // `auto_sprint_plan` already prioritises internally via `TaskScheduler::prioritize`,
    // so as long as SmartTask priorities are populated (above), must-haves are selected
    // before should/could-haves.
    let sprint = TaskScheduler::auto_sprint_plan(&tasks, 10, capacity);
    let prioritized_order = TaskScheduler::prioritize(&tasks);

    let allocated = sprint.allocated_hours(&tasks);
    let remaining = sprint.remaining_capacity(&tasks);

    // Identify tasks that were not selected so callers can see what got descoped.
    let selected: std::collections::HashSet<&str> =
        sprint.tasks.iter().map(|s| s.as_str()).collect();
    let dropped: Vec<String> = tasks
        .iter()
        .filter(|t| !t.is_completed() && !selected.contains(t.id.as_str()))
        .map(|t| t.id.clone())
        .collect();

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
        "dropped": dropped,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}

pub fn suggest(capacity: Option<f64>, horizon_days: usize) -> Result<()> {
    let plan = load_plan()?;
    let tasks = activities_to_smart_tasks(&plan);

    // If the caller supplied a sprint capacity, treat it as a hard cap spread
    // evenly across the horizon. Otherwise fall back to an 8h/day default.
    let (available_per_day, sprint_cap) = match capacity {
        Some(cap) if horizon_days > 0 => (cap / horizon_days as f64, Some(cap)),
        _ => (8.0, None),
    };

    // If total non-completed effort exceeds the sprint budget, descope the
    // lowest-priority tasks up-front so `suggest_schedule` receives a feasible
    // set instead of silently overflowing the horizon.
    let (included_tasks, mut descoped): (Vec<SmartTask>, Vec<String>) = match sprint_cap {
        Some(cap) => {
            let ordered = TaskScheduler::prioritize(&tasks);
            let task_map: HashMap<&str, &SmartTask> =
                tasks.iter().map(|t| (t.id.as_str(), t)).collect();
            let mut budget = cap;
            let mut kept: Vec<SmartTask> = Vec::new();
            let mut dropped: Vec<String> = Vec::new();
            let mut kept_ids: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for id in ordered {
                let Some(task) = task_map.get(id.as_str()) else { continue };
                if task.is_completed() {
                    kept.push((*task).clone());
                    kept_ids.insert(id.clone());
                    continue;
                }
                let hours = task.effective_hours();
                if hours <= budget {
                    budget -= hours;
                    kept.push((*task).clone());
                    kept_ids.insert(id);
                } else {
                    dropped.push(id);
                }
            }
            // Preserve any tasks we never visited (shouldn't happen, but be safe).
            for t in &tasks {
                if !kept_ids.contains(&t.id) && !dropped.contains(&t.id) {
                    dropped.push(t.id.clone());
                }
            }
            (kept, dropped)
        }
        None => (tasks.clone(), Vec::new()),
    };

    let suggestion = TaskScheduler::suggest_schedule(
        &included_tasks,
        available_per_day,
        Local::now().date_naive(),
    );

    // Anything `suggest_schedule` couldn't fit joins the descoped list.
    for id in &suggestion.unscheduled {
        if !descoped.contains(id) {
            descoped.push(id.clone());
        }
    }

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
        "descoped": descoped,
        "warnings": suggestion.warnings,
        "capacity": sprint_cap,
        "hours_per_day": available_per_day,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}

pub fn overcommit_check(hours_per_day: f64) -> Result<()> {
    let plan = load_plan()?;
    let tasks = activities_to_smart_tasks(&plan);

    let warnings = TaskScheduler::detect_overcommitment(&tasks, hours_per_day);
    let overcommitted = !warnings.is_empty();

    // Aggregate stats: sum of excess hours across all days, count of affected
    // days, and the set of resources/assignees touched.
    let total_overage_hours: f64 = warnings.iter().map(|w| w.excess_hours).sum();
    let overcommitted_days = warnings.len();

    // Resources affected = union of each warning's implicated resources.
    // `SmartTask::resource` is the canonical owner; fall back to the warning's
    // first task ID if no resources are recorded.
    let mut resources: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let task_lookup: HashMap<&str, &SmartTask> =
        tasks.iter().map(|t| (t.id.as_str(), t)).collect();
    for warning in &warnings {
        for task_id in &warning.task_ids {
            if let Some(task) = task_lookup.get(task_id.as_str()) {
                if let Some(assignee) = task.assignee.as_ref() {
                    resources.insert(assignee.clone());
                }
            }
        }
    }
    let resources_affected: Vec<String> = resources.into_iter().collect();

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
        "total_overage_hours": total_overage_hours,
        "overcommitted_days": overcommitted_days,
        "resources_affected": resources_affected,
        "warnings": warning_list,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_priority_maps_must_have_to_critical() {
        assert!(matches!(parse_priority("critical"), TaskPriority::Critical));
        assert!(matches!(parse_priority("CRITICAL"), TaskPriority::Critical));
        assert!(matches!(parse_priority("must"), TaskPriority::Critical));
        assert!(matches!(parse_priority("high"), TaskPriority::High));
        assert!(matches!(parse_priority("should"), TaskPriority::High));
        assert!(matches!(parse_priority("medium"), TaskPriority::Medium));
        assert!(matches!(parse_priority("low"), TaskPriority::Low));
        assert!(matches!(parse_priority("could"), TaskPriority::Low));
        assert!(matches!(parse_priority("nonsense"), TaskPriority::Medium));
    }

    #[test]
    fn activity_priority_reads_custom_fields_over_description() {
        let mut a = gantt_ml::model::activity::Activity::new("T1", "Task", 4.0);
        a.custom_fields
            .insert("priority".to_string(), serde_json::json!("critical"));
        a.description = Some("[LOW] ignore me".to_string());
        assert!(matches!(activity_priority(&a), TaskPriority::Critical));
    }

    #[test]
    fn activity_priority_falls_back_to_description_prefix() {
        let mut a = gantt_ml::model::activity::Activity::new("T1", "Task", 4.0);
        a.description = Some("[high] urgent fix".to_string());
        assert!(matches!(activity_priority(&a), TaskPriority::High));
    }

    #[test]
    fn activities_to_smart_tasks_honours_priority_and_selects_must_haves_first() {
        // Reproduce the Scenario 3 failure: 3 tasks, capacity only fits 2.
        // Without priority propagation the greedy IDs order picks P1+P2 and
        // drops P3 (a must-have). With the fix, P3 should be included.
        use gantt_ml::model::activity::Activity;
        use gantt_ml::model::project::Project;

        let mut project = Project::new("proj-1", "test");
        let mut p1 = Activity::new("P1", "low task", 20.0);
        p1.custom_fields
            .insert("priority".to_string(), serde_json::json!("low"));
        let mut p2 = Activity::new("P2", "low task two", 20.0);
        p2.custom_fields
            .insert("priority".to_string(), serde_json::json!("low"));
        let mut p3 = Activity::new("P3", "must have", 10.0);
        p3.custom_fields
            .insert("priority".to_string(), serde_json::json!("critical"));
        project
            .activities
            .insert("P1".to_string(), p1);
        project
            .activities
            .insert("P2".to_string(), p2);
        project
            .activities
            .insert("P3".to_string(), p3);

        let file = CamshaftFile::new(project, crate::modes::PlanMode::Sprint);

        let tasks = activities_to_smart_tasks(&file);
        let p3_task = tasks.iter().find(|t| t.id == "P3").unwrap();
        assert!(matches!(p3_task.priority, TaskPriority::Critical));

        // Capacity 30h fits P3 (10h) + exactly one of P1/P2 (20h each).
        // The critical task MUST survive.
        let sprint = TaskScheduler::auto_sprint_plan(&tasks, 10, 30.0);
        assert!(
            sprint.tasks.contains(&"P3".to_string()),
            "Critical task P3 was dropped from the sprint: {:?}",
            sprint.tasks
        );
    }
}
