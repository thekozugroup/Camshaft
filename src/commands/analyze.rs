use std::collections::HashSet;

use gantt_ml::analysis::{AnomalyType, ScheduleAnalyzer, Severity};
use gantt_ml::cpm::CpmEngine;
use gantt_ml::Project;
use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::plan::load_plan;

/// Run schedule quality analysis on the active plan and emit a JSON report.
pub fn run() -> Result<()> {
    let plan = load_plan()?;
    let project = &plan.project;

    // GanttML schedule-quality analysis
    let report = ScheduleAnalyzer::analyze(project)
        .map_err(|e| CamshaftError::GanttMl(e.to_string()))?;

    // CPM to derive critical path & float distribution.
    let cpm = CpmEngine::calculate(project)
        .map_err(|e| CamshaftError::GanttMl(e.to_string()))?;

    let total_activities = report.total_activities;

    // Logic density: 0-1 scale (fraction of activities with predecessors).
    let logic_density = report
        .metrics
        .iter()
        .find(|m| m.name == "Logic Density")
        .map(|m| (m.value / 100.0 * 1000.0).round() / 1000.0)
        .unwrap_or(0.0);

    // Critical path length (number of activities on the CP).
    let critical_path_length = cpm.critical_path.len();

    // Float distribution buckets derived from CPM total float.
    let (mut zero_float, mut low_float, mut medium_float, mut high_float) = (0u32, 0u32, 0u32, 0u32);
    for (_, &f) in &cpm.total_float {
        if f <= 0.0 {
            zero_float += 1;
        } else if f <= 2.0 {
            low_float += 1;
        } else if f <= 10.0 {
            medium_float += 1;
        } else {
            high_float += 1;
        }
    }

    // Collect anomalies from GanttML report (dedup against locally detected ones).
    let mut anomalies: Vec<serde_json::Value> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();

    for a in &report.anomalies {
        let kind = anomaly_type_tag(&a.anomaly_type);
        let key = (a.activity_id.clone(), kind.to_string());
        if !seen.insert(key) {
            continue;
        }
        anomalies.push(json!({
            "type": kind,
            "task_id": a.activity_id,
            "task_name": a.activity_name,
            "severity": severity_tag(&a.severity),
            "message": a.description,
        }));
    }

    // Local heuristic anomaly detection (supplements GanttML):
    // 1. Orphan tasks: zero predecessors AND zero successors.
    for (id, act) in &project.activities {
        let is_orphan = project.predecessors(id).is_empty() && project.successors(id).is_empty();
        let key = (id.clone(), "orphan_task".to_string());
        if is_orphan && seen.insert(key) {
            anomalies.push(json!({
                "type": "orphan_task",
                "task_id": id,
                "task_name": act.name,
                "severity": "warning",
                "message": "Task has no predecessors or successors (orphan).",
            }));
        }
    }

    // 2. Long duration: task duration > 2x project average.
    let durations: Vec<f64> = project
        .activities
        .values()
        .filter(|a| !a.is_milestone())
        .map(|a| a.original_duration.days)
        .collect();
    let avg_duration = if !durations.is_empty() {
        durations.iter().sum::<f64>() / durations.len() as f64
    } else {
        0.0
    };
    if avg_duration > 0.0 {
        for (id, act) in &project.activities {
            if act.is_milestone() {
                continue;
            }
            let d = act.original_duration.days;
            if d > avg_duration * 2.0 {
                let key = (id.clone(), "long_duration".to_string());
                if seen.insert(key) {
                    anomalies.push(json!({
                        "type": "long_duration",
                        "task_id": id,
                        "task_name": act.name,
                        "severity": "info",
                        "message": format!(
                            "Task duration {:.1} is > 2x project average ({:.1}).",
                            d, avg_duration
                        ),
                    }));
                }
            }
        }
    }

    // 3. Zero-float + long duration: risky critical tasks.
    for (id, &tf) in &cpm.total_float {
        if tf > 0.0 {
            continue;
        }
        if let Some(act) = project.activities.get(id) {
            let d = act.original_duration.days;
            if avg_duration > 0.0 && d > avg_duration * 1.5 {
                let key = (id.clone(), "risky_critical".to_string());
                if seen.insert(key) {
                    anomalies.push(json!({
                        "type": "risky_critical",
                        "task_id": id,
                        "task_name": act.name,
                        "severity": "warning",
                        "message": format!(
                            "Critical task (zero float) with long duration {:.1} — delays impact project end.",
                            d
                        ),
                    }));
                }
            }
        }
    }

    // Rule-based recommendations on top of the collected data.
    let recommendations = build_recommendations(
        project,
        &cpm,
        avg_duration,
        logic_density,
        report.overall_score,
        &anomalies,
    );

    let summary = build_summary(report.overall_score);

    let out = json!({
        "health_score": report.overall_score.round() as i64,
        "summary": summary,
        "total_activities": total_activities,
        "logic_density": logic_density,
        "critical_path_length": critical_path_length,
        "project_duration": cpm.project_duration,
        "float_distribution": {
            "zero_float": zero_float,
            "low_float": low_float,
            "medium_float": medium_float,
            "high_float": high_float,
        },
        "metrics": report.metrics.iter().map(|m| json!({
            "name": m.name,
            "value": round2(m.value),
            "threshold": m.threshold,
            "passed": m.passed,
            "description": m.description,
        })).collect::<Vec<_>>(),
        "anomalies": anomalies,
        "recommendations": recommendations,
    });

    println!("{}", serde_json::to_string_pretty(&out).unwrap());
    Ok(())
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn anomaly_type_tag(t: &AnomalyType) -> &'static str {
    match t {
        AnomalyType::UnusualDuration => "unusual_duration",
        AnomalyType::MissingDependency => "missing_dependency",
        AnomalyType::ExcessiveFloat => "excessive_float",
        AnomalyType::NegativeFloat => "negative_float",
        AnomalyType::MissingResource => "missing_resource",
        AnomalyType::DurationOutlier => "duration_outlier",
        AnomalyType::HighRisk => "high_risk",
        AnomalyType::InconsistentDates => "inconsistent_dates",
        AnomalyType::OpenEndedActivity => "orphan_task",
    }
}

fn severity_tag(s: &Severity) -> &'static str {
    match s {
        Severity::Low => "info",
        Severity::Medium => "warning",
        Severity::High => "warning",
        Severity::Critical => "critical",
    }
}

fn build_summary(score: f64) -> String {
    if score >= 90.0 {
        "Plan is well-structured and healthy.".to_string()
    } else if score >= 75.0 {
        "Plan is well-structured with minor improvements possible.".to_string()
    } else if score >= 50.0 {
        "Plan has moderate issues that should be addressed.".to_string()
    } else if score > 0.0 {
        "Plan has significant quality issues — review anomalies and metrics.".to_string()
    } else {
        "Plan is empty or has no analyzable schedule data.".to_string()
    }
}

fn build_recommendations(
    project: &Project,
    cpm: &gantt_ml::cpm::CpmResult,
    avg_duration: f64,
    logic_density: f64,
    score: f64,
    anomalies: &[serde_json::Value],
) -> Vec<String> {
    let mut recs: Vec<String> = Vec::new();

    // Logic density < 0.9 -> encourage linking.
    if logic_density < 0.9 && project.activity_count() > 1 {
        recs.push(format!(
            "Logic density is {:.0}% — link more tasks with dependencies (target >=90%).",
            logic_density * 100.0
        ));
    }

    // Long tasks -> suggest break-down.
    if avg_duration > 0.0 {
        for (id, act) in &project.activities {
            if act.is_milestone() {
                continue;
            }
            let d = act.original_duration.days;
            if d > avg_duration * 2.0 {
                recs.push(format!(
                    "Consider breaking down '{}' ({:.0}) into smaller tasks.",
                    id, d
                ));
            }
        }
    }

    // Critical path with no float suggests buffer.
    if !cpm.critical_path.is_empty() {
        recs.push(
            "Add buffer time to the critical path to absorb unplanned delays.".to_string(),
        );
    }

    // Orphan tasks.
    let orphan_count = anomalies
        .iter()
        .filter(|a| a.get("type").and_then(|v| v.as_str()) == Some("orphan_task"))
        .count();
    if orphan_count > 0 {
        recs.push(format!(
            "Connect {} orphan task(s) to the schedule with dependencies.",
            orphan_count
        ));
    }

    // Low score -> general advice.
    if score < 75.0 && project.activity_count() > 0 {
        recs.push(
            "Review failing quality metrics (logic density, open-ended tasks, resource coverage)."
                .to_string(),
        );
    }

    if recs.is_empty() {
        recs.push("No actionable recommendations — plan looks healthy.".to_string());
    }

    recs
}
