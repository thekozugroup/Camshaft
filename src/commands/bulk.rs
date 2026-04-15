// bulk command — single-file plan creation from YAML or JSON

use std::fs;
use std::path::Path;

use chrono::NaiveDate;
use gantt_ml::model::activity::Activity;
use gantt_ml::model::dependency::Dependency;
use gantt_ml::model::resource::{Resource, ResourceAssignment};
use gantt_ml::model::types::{ActivityType, DependencyType, ResourceType};
use gantt_ml::Project;
use serde::Deserialize;
use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::modes::PlanMode;
use crate::plan::{plan_exists, save_plan, CamshaftFile};

/// Maximum bulk file size: 10 MB
const MAX_BULK_SIZE: u64 = 10 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct BulkPlan {
    name: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    start: Option<String>,
    #[serde(default)]
    tasks: Vec<BulkTask>,
    #[serde(default)]
    dependencies: Vec<BulkDep>,
    #[serde(default)]
    milestones: Vec<BulkMilestone>,
    #[serde(default)]
    resources: Vec<BulkResource>,
    #[serde(default)]
    assignments: Vec<BulkAssignment>,
}

#[derive(Debug, Deserialize)]
struct BulkTask {
    id: String,
    name: String,
    duration: f64,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    category: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BulkDep {
    Tuple([String; 2]),
    Detailed {
        from: String,
        to: String,
        #[serde(default, rename = "type")]
        dep_type: Option<String>,
        #[serde(default)]
        lag: Option<f64>,
    },
}

#[derive(Debug, Deserialize)]
struct BulkMilestone {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct BulkResource {
    id: String,
    name: String,
    #[serde(default, rename = "type")]
    res_type: Option<String>,
    #[serde(default)]
    units: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct BulkAssignment {
    task: String,
    resource: String,
    #[serde(default)]
    units: Option<f64>,
}

pub fn run(file: &str, force: bool) -> Result<()> {
    let path = Path::new(file);

    // Security: reject paths with ".." components to prevent traversal
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return Err(CamshaftError::ValidationFailed(
                "Bulk path must not contain '..' components.".to_string(),
            ));
        }
    }

    // Security: reject absolute paths — bulk files should stay relative to CWD
    if path.is_absolute() {
        return Err(CamshaftError::ValidationFailed(
            "Bulk path must be relative, not absolute.".to_string(),
        ));
    }

    if !path.exists() {
        return Err(CamshaftError::Io(format!(
            "Bulk file does not exist: {}",
            file
        )));
    }

    // Security: file size limit
    let metadata = fs::metadata(path).map_err(|e| CamshaftError::Io(e.to_string()))?;
    if metadata.len() > MAX_BULK_SIZE {
        return Err(CamshaftError::ValidationFailed(format!(
            "Bulk file exceeds maximum size of {} MB.",
            MAX_BULK_SIZE / (1024 * 1024)
        )));
    }

    // Check existence BEFORE parsing for clearest error ordering
    if plan_exists() && !force {
        return Err(CamshaftError::PlanAlreadyExists);
    }

    let contents = fs::read_to_string(path).map_err(|e| CamshaftError::Io(e.to_string()))?;

    // Auto-detect format by extension (fall back to JSON, then YAML)
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let bulk: BulkPlan = match ext.as_str() {
        "yaml" | "yml" => serde_yaml::from_str(&contents).map_err(|e| {
            CamshaftError::Serialization(format!("YAML parse error: {}", e))
        })?,
        "json" => serde_json::from_str(&contents).map_err(|e| {
            CamshaftError::Serialization(format!("JSON parse error: {}", e))
        })?,
        _ => {
            // Try JSON first, then YAML
            match serde_json::from_str::<BulkPlan>(&contents) {
                Ok(b) => b,
                Err(_) => serde_yaml::from_str(&contents).map_err(|e| {
                    CamshaftError::Serialization(format!(
                        "File is neither valid JSON nor YAML: {}",
                        e
                    ))
                })?,
            }
        }
    };

    // Resolve mode
    let mode = match bulk.mode.as_deref().unwrap_or("sprint").to_lowercase().as_str() {
        "sprint" => PlanMode::Sprint,
        "roadmap" => PlanMode::Roadmap,
        other => {
            return Err(CamshaftError::ValidationFailed(format!(
                "Unknown mode: {}. Use 'sprint' or 'roadmap'.",
                other
            )));
        }
    };

    // Build project in-memory and accumulate errors so we never partial-write
    let short_id = &uuid::Uuid::new_v4().to_string()[..8];
    let mut project = Project::new(short_id, &bulk.name);

    if let Some(date_str) = &bulk.start {
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|e| CamshaftError::ValidationFailed(format!("Invalid start date: {e}")))?;
        project.planned_start = Some(date);
    }

    let mut errors: Vec<String> = Vec::new();

    // Tasks
    let mut tasks_created = 0usize;
    for t in &bulk.tasks {
        if project.activities.contains_key(&t.id) {
            errors.push(format!("Duplicate task id: {}", t.id));
            continue;
        }
        let mut activity = Activity::new(&t.id, &t.name, t.duration);
        if let Some(p) = &t.priority {
            activity
                .custom_fields
                .insert("priority".to_string(), serde_json::Value::String(p.clone()));
        }
        if let Some(c) = &t.category {
            activity
                .custom_fields
                .insert("category".to_string(), serde_json::Value::String(c.clone()));
        }
        project.activities.insert(t.id.clone(), activity);
        tasks_created += 1;
    }

    // Milestones
    let mut milestones_created = 0usize;
    for m in &bulk.milestones {
        if project.activities.contains_key(&m.id) {
            errors.push(format!("Duplicate milestone/task id: {}", m.id));
            continue;
        }
        let mut activity = Activity::new(&m.id, &m.name, 0.0);
        activity.activity_type = ActivityType::FinishMilestone;
        project.activities.insert(m.id.clone(), activity);
        milestones_created += 1;
    }

    // Resources
    let mut resources_created = 0usize;
    for r in &bulk.resources {
        if project.resources.contains_key(&r.id) {
            errors.push(format!("Duplicate resource id: {}", r.id));
            continue;
        }
        let res_type_str = r.res_type.as_deref().unwrap_or("labor");
        let res_type = match parse_resource_type(res_type_str) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("Resource {}: {}", r.id, format_err(&e)));
                continue;
            }
        };
        let units = r.units.unwrap_or(8.0);
        let resource = Resource::new(&r.id, &r.name, res_type, units);
        project.resources.insert(r.id.clone(), resource);
        resources_created += 1;
    }

    // Dependencies — must reference existing activities
    let mut dependencies_created = 0usize;
    for d in &bulk.dependencies {
        let (from, to, dep_type_str, lag) = match d {
            BulkDep::Tuple([f, t]) => (f.clone(), t.clone(), "fs".to_string(), 0.0),
            BulkDep::Detailed { from, to, dep_type, lag } => (
                from.clone(),
                to.clone(),
                dep_type.clone().unwrap_or_else(|| "fs".to_string()),
                lag.unwrap_or(0.0),
            ),
        };

        if !project.activities.contains_key(&from) {
            errors.push(format!(
                "Dependency references nonexistent task: {} -> {}",
                from, to
            ));
            continue;
        }
        if !project.activities.contains_key(&to) {
            errors.push(format!(
                "Dependency references nonexistent task: {} -> {}",
                from, to
            ));
            continue;
        }

        let dtype = match parse_dependency_type(&dep_type_str) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("Dependency {} -> {}: {}", from, to, format_err(&e)));
                continue;
            }
        };
        let dep = Dependency::finish_to_start(&from, &to)
            .with_type(dtype)
            .with_lag(lag);
        project.dependencies.push(dep);
        dependencies_created += 1;
    }

    // Assignments — must reference existing tasks & resources
    let mut assignments_created = 0usize;
    for a in &bulk.assignments {
        if !project.activities.contains_key(&a.task) {
            errors.push(format!("Assignment references nonexistent task: {}", a.task));
            continue;
        }
        if !project.resources.contains_key(&a.resource) {
            errors.push(format!(
                "Assignment references nonexistent resource: {}",
                a.resource
            ));
            continue;
        }
        let units = a.units.unwrap_or(1.0);
        let assignment = ResourceAssignment::new(&a.task, &a.resource, units);
        project.resource_assignments.push(assignment);
        assignments_created += 1;
    }

    if !errors.is_empty() {
        return Err(CamshaftError::ValidationFailed(format!(
            "Bulk file has {} error(s): {}",
            errors.len(),
            errors.join("; ")
        )));
    }

    // All valid — save once, atomically
    let camshaft_file = CamshaftFile::new(project, mode);
    save_plan(&camshaft_file)?;

    let output = json!({
        "status": "created",
        "file": file,
        "summary": {
            "tasks_created": tasks_created,
            "dependencies_created": dependencies_created,
            "milestones_created": milestones_created,
            "resources_created": resources_created,
            "assignments_created": assignments_created,
        }
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

fn format_err(e: &CamshaftError) -> String {
    format!("{}", e)
}
