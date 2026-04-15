use std::collections::BTreeMap;

use chrono::NaiveDate;
use gantt_ml::cpm::CpmEngine;
use gantt_ml::resource::ResourceAnalyzer;
use gantt_ml::Project;
use serde_json::{json, Value};

use crate::error::{CamshaftError, Result};
use crate::plan::{load_plan, save_plan};

/// Run resource leveling on the active plan.
///
/// When `apply` is true, mutates the plan's activities (shifting
/// `early_start`/`early_finish`) and persists the result. Otherwise only
/// emits an analysis JSON report.
pub fn run(apply: bool) -> Result<()> {
    let mut plan = load_plan()?;

    // Guard: no activities -> nothing to level.
    if plan.project.activities.is_empty() {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "status": "no_conflicts",
                "message": "No activities in plan. Nothing to level."
            }))
            .unwrap()
        );
        return Ok(());
    }

    // Guard: no resources defined -> nothing to level.
    if plan.project.resources.is_empty() || plan.project.resource_assignments.is_empty() {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "status": "no_conflicts",
                "message": "No resource over-allocation detected. Plan is already level."
            }))
            .unwrap()
        );
        return Ok(());
    }

    // 1) Run CPM to populate early_start / early_finish / total_float on
    //    activities (required by ResourceAnalyzer::detect_conflicts and
    //    ResourceAnalyzer::level_resources).
    let project_start = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap_or(NaiveDate::MIN);
    let cpm_before = CpmEngine::schedule(&mut plan.project, project_start)
        .map_err(|e| CamshaftError::GanttMl(e.to_string()))?;
    let original_duration = cpm_before.project_duration;

    // Snapshot pre-level starts (in day-offsets from project_start) for
    // computing per-activity moves after leveling.
    let starts_before: BTreeMap<String, f64> = cpm_before
        .early_dates
        .iter()
        .map(|(id, (es, _))| (id.clone(), *es))
        .collect();

    // 2) Detect conflicts before leveling.
    let conflicts_before = ResourceAnalyzer::detect_conflicts(&plan.project)
        .map_err(|e| CamshaftError::GanttMl(e.to_string()))?;

    if conflicts_before.is_empty() {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "status": "no_conflicts",
                "message": "No resource over-allocation detected. Plan is already level."
            }))
            .unwrap()
        );
        return Ok(());
    }

    // 3) Peak utilization before (units per resource across all dates).
    let peak_before = peak_utilization(&plan.project);

    // 4) Snapshot conflict count before.
    let total_conflicts_before = conflicts_before.len();

    // 5) Run the resource leveler. Operates on a working clone so we can
    //    discard if the caller only asked for analysis.
    let mut project_work: Project = plan.project.clone();
    let leveling = ResourceAnalyzer::level_resources(&mut project_work)
        .map_err(|e| CamshaftError::GanttMl(e.to_string()))?;

    // 6) Recompute CPM on the leveled project to get the new duration.
    //    level_resources mutates early_start/early_finish directly, but we
    //    want a comparable project_duration derived from those updated
    //    positions. Re-running CPM::schedule regenerates the dates from
    //    dependencies — which would overwrite the leveled shifts — so
    //    instead derive the leveled duration from the max early_finish
    //    offset present on activities after leveling.
    let leveled_duration = derive_project_duration(&project_work, project_start);

    // 7) Build per-activity moves by diffing early_start before vs. after.
    let moves_json: Vec<Value> = project_work
        .activities
        .iter()
        .filter_map(|(id, act)| {
            let new_start = offset_days(act.early_start, project_start)?;
            let orig_start = *starts_before.get(id)?;
            let delta = new_start - orig_start;
            if delta.abs() < 1e-9 {
                return None;
            }
            Some(json!({
                "task_id": id,
                "original_start": orig_start,
                "new_start": new_start,
                "reason": reason_for_move(id, &conflicts_before),
            }))
        })
        .collect();

    // 8) Peak utilization after.
    let peak_after = peak_utilization(&project_work);

    let conflicts_resolved = total_conflicts_before.saturating_sub(leveling.remaining_conflicts);

    let output = json!({
        "status": "leveled",
        "original_duration": round3(original_duration),
        "leveled_duration": round3(leveled_duration),
        "duration_increase": round3(leveled_duration - original_duration),
        "activities_delayed": leveling.activities_delayed,
        "max_delay_days": leveling.max_delay_days,
        "moves": moves_json,
        "peak_utilization_before": peak_before,
        "peak_utilization_after": peak_after,
        "conflicts_before": total_conflicts_before,
        "conflicts_resolved": conflicts_resolved,
        "remaining_conflicts": leveling.remaining_conflicts,
        "applied": apply,
    });

    // 9) Persist if requested.
    if apply {
        plan.project = project_work;
        save_plan(&plan)?;
    }

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    Ok(())
}

/// Convert an optional NaiveDate to day-offset from `origin`. Returns None
/// when the date is not set.
fn offset_days(date: Option<NaiveDate>, origin: NaiveDate) -> Option<f64> {
    date.map(|d| (d - origin).num_days() as f64)
}

/// Max early_finish offset from project_start across all activities.
fn derive_project_duration(project: &Project, origin: NaiveDate) -> f64 {
    project
        .activities
        .values()
        .filter_map(|a| offset_days(a.early_finish, origin))
        .fold(0.0_f64, f64::max)
}

/// Peak daily units demand per resource across the planning horizon.
/// Matches the ResourceAnalyzer::build_profiles approach at a surface level.
fn peak_utilization(project: &Project) -> Value {
    let mut peak: BTreeMap<String, f64> = BTreeMap::new();
    // Initialise keys so the output is stable even when a resource is unused.
    for (rid, _) in &project.resources {
        peak.insert(rid.clone(), 0.0);
    }

    // daily_demand[resource_id][date] = units
    let mut daily: BTreeMap<String, BTreeMap<NaiveDate, f64>> = BTreeMap::new();
    for assignment in &project.resource_assignments {
        let Some(activity) = project.activities.get(&assignment.activity_id) else {
            continue;
        };
        let (Some(start), Some(finish)) = (activity.early_start, activity.early_finish) else {
            continue;
        };
        let upd = assignment
            .units_per_day
            .unwrap_or(assignment.planned_units);
        let mut d = start;
        while d <= finish {
            *daily
                .entry(assignment.resource_id.clone())
                .or_default()
                .entry(d)
                .or_insert(0.0) += upd;
            d += chrono::TimeDelta::days(1);
        }
    }

    for (rid, by_date) in daily {
        let p = by_date.values().cloned().fold(0.0_f64, f64::max);
        peak.insert(rid, p);
    }

    // Serialise as a plain object.
    let obj: serde_json::Map<String, Value> = peak
        .into_iter()
        .map(|(k, v)| (k, json!(round3(v))))
        .collect();
    Value::Object(obj)
}

/// Pick a human-readable reason for why an activity was moved, based on the
/// conflicts detected before leveling. Uses the first conflict this activity
/// contributed to as representative.
fn reason_for_move(
    activity_id: &str,
    conflicts_before: &[gantt_ml::resource::ResourceConflict],
) -> String {
    for c in conflicts_before {
        if c.contributing_activities.iter().any(|a| a == activity_id) {
            let other: Vec<&str> = c
                .contributing_activities
                .iter()
                .filter(|a| *a != activity_id)
                .map(String::as_str)
                .collect();
            if other.is_empty() {
                return format!("Resource '{}' over-allocated on {}", c.resource_id, c.date);
            }
            return format!(
                "Resource '{}' conflict with '{}'",
                c.resource_id,
                other.join(", ")
            );
        }
    }
    "Resource leveling shift".to_string()
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
