use gantt_ml::model::activity::Activity;
use gantt_ml::model::dependency::Dependency;
use gantt_ml::model::resource::{Resource, ResourceAssignment};
use gantt_ml::model::types::{ActivityType, DependencyType, ResourceType};
use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::plan::{load_plan, save_plan};

pub fn add_task(
    id: &str,
    name: &str,
    duration: f64,
    priority: &str,
    category: Option<&str>,
) -> Result<()> {
    let mut file = load_plan()?;

    if file.project.activities.contains_key(id) {
        return Err(CamshaftError::DuplicateTask(id.to_string()));
    }

    let mut activity = Activity::new(id, name, duration);
    // Persist priority in custom_fields so downstream commands (e.g. sprint)
    // can honour it. Normalised to lowercase for stable matching.
    let normalized_priority = priority.to_lowercase();
    activity
        .custom_fields
        .insert("priority".to_string(), serde_json::json!(normalized_priority));
    if let Some(cat) = category {
        activity
            .custom_fields
            .insert("category".to_string(), serde_json::json!(cat));
    }
    file.project.activities.insert(id.to_string(), activity);
    save_plan(&file)?;

    let output = json!({
        "status": "added",
        "type": "task",
        "id": id,
        "name": name,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}

pub fn add_dep(from: &str, to: &str, dep_type: &str, lag: f64) -> Result<()> {
    let mut file = load_plan()?;

    if !file.project.activities.contains_key(from) {
        return Err(CamshaftError::InvalidDependency {
            predecessor: from.to_string(),
            successor: to.to_string(),
        });
    }
    if !file.project.activities.contains_key(to) {
        return Err(CamshaftError::InvalidDependency {
            predecessor: from.to_string(),
            successor: to.to_string(),
        });
    }

    let dtype = parse_dependency_type(dep_type)?;
    let dep = Dependency::finish_to_start(from, to)
        .with_type(dtype)
        .with_lag(lag);

    file.project.dependencies.push(dep);
    save_plan(&file)?;

    let output = json!({
        "status": "added",
        "type": "dependency",
        "from": from,
        "to": to,
        "dep_type": dep_type,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}

pub fn add_milestone(id: &str, name: &str) -> Result<()> {
    let mut file = load_plan()?;

    if file.project.activities.contains_key(id) {
        return Err(CamshaftError::DuplicateTask(id.to_string()));
    }

    let mut activity = Activity::new(id, name, 0.0);
    activity.activity_type = ActivityType::FinishMilestone;
    file.project.activities.insert(id.to_string(), activity);
    save_plan(&file)?;

    let output = json!({
        "status": "added",
        "type": "milestone",
        "id": id,
        "name": name,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}

pub fn add_resource(id: &str, name: &str, res_type: &str, units: f64) -> Result<()> {
    let mut file = load_plan()?;

    let resource_type = parse_resource_type(res_type)?;
    let resource = Resource::new(id, name, resource_type, units);
    file.project.resources.insert(id.to_string(), resource);
    save_plan(&file)?;

    let output = json!({
        "status": "added",
        "type": "resource",
        "id": id,
        "name": name,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}

pub fn assign_resource(task_id: &str, resource_id: &str, units: f64) -> Result<()> {
    let mut file = load_plan()?;

    if !file.project.activities.contains_key(task_id) {
        return Err(CamshaftError::TaskNotFound(task_id.to_string()));
    }
    if !file.project.resources.contains_key(resource_id) {
        return Err(CamshaftError::TaskNotFound(format!(
            "resource: {resource_id}"
        )));
    }

    let assignment = ResourceAssignment::new(task_id, resource_id, units);
    file.project.resource_assignments.push(assignment);
    save_plan(&file)?;

    let output = json!({
        "status": "assigned",
        "type": "resource_assignment",
        "task_id": task_id,
        "resource_id": resource_id,
        "units": units,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}

fn parse_dependency_type(s: &str) -> Result<DependencyType> {
    match s.to_lowercase().as_str() {
        "fs" | "finish_to_start" => Ok(DependencyType::FinishToStart),
        "ss" | "start_to_start" => Ok(DependencyType::StartToStart),
        "ff" | "finish_to_finish" => Ok(DependencyType::FinishToFinish),
        "sf" | "start_to_finish" => Ok(DependencyType::StartToFinish),
        _ => Err(CamshaftError::ValidationFailed(format!(
            "Unknown dependency type: {s}. Use fs, ss, ff, or sf."
        ))),
    }
}

fn parse_resource_type(s: &str) -> Result<ResourceType> {
    match s.to_lowercase().as_str() {
        "labor" => Ok(ResourceType::Labor),
        "material" => Ok(ResourceType::Material),
        "equipment" => Ok(ResourceType::Equipment),
        "nonlabor" | "non_labor" => Ok(ResourceType::NonLabor),
        _ => Err(CamshaftError::ValidationFailed(format!(
            "Unknown resource type: {s}. Use labor, material, equipment, or nonlabor."
        ))),
    }
}
