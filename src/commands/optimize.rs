use std::collections::{HashMap, HashSet};

use chrono::{NaiveDate, Utc};
use gantt_ml::cpm::CpmEngine;
use gantt_ml::model::types::DependencyType;
use gantt_ml::optimizer::{
    GeneticOptimizer, OptimizationConfig, OptimizationObjective, ScheduleCompressor,
};
use gantt_ml::Project;
use serde_json::{json, Value};

use crate::error::{CamshaftError, Result};
use crate::plan::{load_plan, save_plan};

/// Default crash pct applied when `--crash` is requested. Matches the common
/// PM heuristic of compressing critical-path tasks by up to 25 percent.
const DEFAULT_CRASH_PCT: f64 = 0.25;

/// Default fast-track overlap fraction (must match ScheduleCompressor internals).
const DEFAULT_FAST_TRACK_OVERLAP: f64 = 0.25;

/// Estimated cost-per-day delta when crashing. Used purely for reporting —
/// real crashing cost would come from resource rates. `500.0` is a neutral
/// placeholder that makes the `cost_delta` field meaningful without inventing
/// rates that don't exist in the plan.
const CRASH_COST_PER_DAY: f64 = 500.0;

/// Compute parallel groups from CPM results.
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

fn parse_objective(s: &str) -> OptimizationObjective {
    match s {
        "min-cost" => OptimizationObjective::MinimizeCost,
        "min-resource" | "min-resource-peak" => OptimizationObjective::MinimizeResourcePeak,
        "multi-objective" | "multi" => OptimizationObjective::MultiObjective {
            duration_weight: 0.5,
            cost_weight: 0.3,
            resource_weight: 0.2,
        },
        // Default: minimize duration
        _ => OptimizationObjective::MinimizeDuration,
    }
}

/// Build the detailed fast-track move records the spec describes. We mirror
/// `ScheduleCompressor::fast_track`'s logic so we can expose the pair
/// `(predecessor, successor)` and the lag we would apply on `--apply`.
fn build_fast_track_moves(project: &Project) -> Vec<(String, String, f64, f64)> {
    // (pred_id, succ_id, overlap_days, pred_duration)
    let mut out = Vec::new();
    for dep in &project.dependencies {
        if dep.dependency_type != DependencyType::FinishToStart {
            continue;
        }
        let pred = match project.activities.get(&dep.predecessor_id) {
            Some(a) => a,
            None => continue,
        };
        let succ = match project.activities.get(&dep.successor_id) {
            Some(a) => a,
            None => continue,
        };
        if !pred.is_critical() || !succ.is_critical() {
            continue;
        }
        let pred_dur = pred.original_duration.days;
        let overlap = pred_dur * DEFAULT_FAST_TRACK_OVERLAP;
        if overlap > 0.0 {
            out.push((
                dep.predecessor_id.clone(),
                dep.successor_id.clone(),
                overlap,
                pred_dur,
            ));
        }
    }
    out
}

/// Mutate the project in place to apply fast-track moves: convert qualifying
/// FS dependencies into SS with positive lag equal to the fast-track overlap.
fn apply_fast_track(project: &mut Project, moves: &[(String, String, f64, f64)]) {
    let keys: HashSet<(String, String)> = moves
        .iter()
        .map(|(p, s, _, _)| (p.clone(), s.clone()))
        .collect();
    for dep in project.dependencies.iter_mut() {
        if dep.dependency_type == DependencyType::FinishToStart
            && keys.contains(&(dep.predecessor_id.clone(), dep.successor_id.clone()))
        {
            // lag == overlap days (SS lag = how far into predecessor before
            // successor may start). Recompute from moves to keep it consistent.
            if let Some((_, _, overlap, _)) = moves
                .iter()
                .find(|(p, s, _, _)| *p == dep.predecessor_id && *s == dep.successor_id)
            {
                dep.dependency_type = DependencyType::StartToStart;
                dep.lag_days = *overlap;
            }
        }
    }
}

/// Mutate the project in place to apply crash moves: reduce `original_duration`
/// on each named activity by `crash_pct`.
fn apply_crash(project: &mut Project, moves: &[(String, f64, f64)], crash_pct: f64) {
    for (id, _orig, _saved) in moves {
        if let Some(act) = project.activities.get_mut(id) {
            let new_days = act.original_duration.days * (1.0 - crash_pct);
            act.original_duration.days = new_days.max(0.0);
        }
    }
}

pub fn run(
    objective: &str,
    fast_track: bool,
    crash: bool,
    apply: bool,
) -> Result<()> {
    let mut plan = load_plan()?;

    if plan.project.activities.is_empty() {
        return Err(CamshaftError::OptimizationFailed(
            "No activities in plan. Add tasks before optimizing.".to_string(),
        ));
    }

    // Run CPM in a way that writes total_float/early_start/... back onto each
    // activity — required for `is_critical()` inside ScheduleCompressor.
    // We use a fixed reference date (project planned_start if set, else today)
    // because schedule() wants a calendar anchor but we only care about floats.
    let anchor = plan
        .project
        .planned_start
        .or_else(|| Some(Utc::now().date_naive()))
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());

    let cpm = {
        let mut proj_for_cpm = plan.project.clone();
        let res = CpmEngine::schedule(&mut proj_for_cpm, anchor)
            .map_err(|e| CamshaftError::GanttMl(e.to_string()))?;
        // Write the float info back onto the real project so subsequent
        // compressor calls can see is_critical() correctly.
        for (id, act) in proj_for_cpm.activities.iter() {
            if let Some(live) = plan.project.activities.get_mut(id) {
                live.total_float = act.total_float;
                live.free_float = act.free_float;
                live.early_start = act.early_start;
                live.early_finish = act.early_finish;
                live.late_start = act.late_start;
                live.late_finish = act.late_finish;
            }
        }
        res
    };

    let parallel_groups = compute_parallel_groups(&cpm, &plan.project);

    let bottlenecks: Vec<String> = cpm
        .total_float
        .iter()
        .filter(|(_, &f)| f == 0.0)
        .map(|(id, _)| id.clone())
        .collect();

    let suggested_order: Vec<String> = parallel_groups
        .iter()
        .map(|(num, tasks, _)| format!("group{}: {}", num, tasks.join(" || ")))
        .collect();

    let groups_json: Vec<Value> = parallel_groups
        .iter()
        .map(|(num, tasks, es)| {
            json!({
                "group": num,
                "tasks": tasks,
                "earliest_start": es,
            })
        })
        .collect();

    // ─── Collect optimization moves ────────────────────────────────────
    let mut optimization_moves: Vec<Value> = Vec::new();
    let mut fast_track_json: Vec<Value> = Vec::new();
    let mut crash_json: Vec<Value> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // --fast-track
    let ft_pairs: Vec<(String, String, f64, f64)> = if fast_track {
        if cpm.critical_path.is_empty() {
            warnings.push(
                "No critical path — fast-tracking has nothing to compress.".to_string(),
            );
            Vec::new()
        } else {
            match ScheduleCompressor::fast_track(&plan.project) {
                Ok(_moves) => {
                    // Use our richer pair-aware view (mirrors the compressor).
                    let pairs = build_fast_track_moves(&plan.project);
                    for (pred, succ, overlap, _pred_dur) in &pairs {
                        let entry = json!({
                            "type": "fast_track",
                            "tasks": [pred, succ],
                            "original_dep_type": "FinishToStart",
                            "new_dep_type": "StartToStart",
                            "lag": overlap,
                            "duration_saved": overlap,
                        });
                        fast_track_json.push(entry.clone());
                        optimization_moves.push(entry);
                    }
                    pairs
                }
                Err(e) => {
                    warnings.push(format!("Fast-track skipped: {}", e));
                    Vec::new()
                }
            }
        }
    } else {
        Vec::new()
    };

    // --crash
    let crash_entries: Vec<(String, f64, f64)> = if crash {
        if cpm.critical_path.is_empty() {
            warnings.push("No critical path — crashing has nothing to compress.".to_string());
            Vec::new()
        } else {
            match ScheduleCompressor::crash_schedule(&plan.project, DEFAULT_CRASH_PCT) {
                Ok(moves) => {
                    let mut out = Vec::new();
                    for m in &moves {
                        let orig = plan
                            .project
                            .activities
                            .get(&m.activity_id)
                            .map(|a| a.original_duration.days)
                            .unwrap_or(0.0);
                        let saved = -m.duration_impact;
                        let new_dur = (orig - saved).max(0.0);
                        let entry = json!({
                            "type": "crash",
                            "task": m.activity_id,
                            "original_duration": orig,
                            "new_duration": new_dur,
                            "duration_saved": saved,
                            "cost_delta": saved * CRASH_COST_PER_DAY,
                        });
                        crash_json.push(entry.clone());
                        optimization_moves.push(entry);
                        out.push((m.activity_id.clone(), orig, saved));
                    }
                    out
                }
                Err(e) => {
                    warnings.push(format!("Crash skipped: {}", e));
                    Vec::new()
                }
            }
        }
    } else {
        Vec::new()
    };

    // ─── Genetic optimization (optional) ──────────────────────────────
    let mut genetic_json: Option<Value> = None;
    let wants_genetic = matches!(
        objective,
        "min-duration" | "min-cost" | "multi-objective" | "multi" | "min-resource"
            | "min-resource-peak"
    );
    // Only bother if there's something to reorder.
    if wants_genetic && plan.project.activities.len() >= 2 {
        let config = OptimizationConfig {
            objective: parse_objective(objective),
            // Keep runtime bounded — agents call this often.
            max_iterations: 200,
            population_size: 30,
            convergence_window: 40,
            ..Default::default()
        };
        match GeneticOptimizer::new(config).optimize(&plan.project) {
            Ok(result) => {
                if result.optimized_duration + 1e-6 < result.original_duration {
                    genetic_json = Some(json!({
                        "original_duration": result.original_duration,
                        "optimized_duration": result.optimized_duration,
                        "improvement_pct": result.improvement_pct,
                        "iterations_used": result.iterations_used,
                        "converged": result.converged,
                        "move_count": result.moves.len(),
                    }));
                }
            }
            Err(e) => {
                warnings.push(format!("Genetic optimizer skipped: {}", e));
            }
        }
    }

    // ─── Duration summary ──────────────────────────────────────────────
    // Pessimistic sum of fast-track + crash savings on the same pass —
    // NOT re-running CPM because we keep this operation read-only unless
    // --apply is set.
    let savings: f64 = ft_pairs.iter().map(|(_, _, o, _)| *o).sum::<f64>()
        + crash_entries.iter().map(|(_, _, s)| *s).sum::<f64>();
    let original_duration = cpm.project_duration;
    let optimized_duration = (original_duration - savings).max(0.0);
    let improvement_pct = if original_duration > 0.0 {
        (savings / original_duration) * 100.0
    } else {
        0.0
    };

    // ─── Apply changes if requested ───────────────────────────────────
    let mut applied = false;
    if apply && (fast_track || crash) {
        if fast_track {
            apply_fast_track(&mut plan.project, &ft_pairs);
        }
        if crash {
            apply_crash(
                &mut plan.project,
                &crash_entries,
                DEFAULT_CRASH_PCT,
            );
        }
        applied = true;
    }

    // Always record an optimization run so other commands can see it happened.
    plan.meta.last_optimized = Some(Utc::now());
    plan.meta.optimization_runs += 1;
    plan.meta.parallelism_groups = parallel_groups
        .iter()
        .map(|(_, tasks, _)| tasks.clone())
        .collect();

    // Save plan: always (to record metadata), but only persist mutated
    // project when --apply was honoured.
    save_plan(&plan)?;

    let next_ready_tasks: Vec<String> = parallel_groups
        .first()
        .map(|(_, tasks, _)| tasks.clone())
        .unwrap_or_default();

    let execution_hint: String = {
        let start_clause = if next_ready_tasks.is_empty() {
            "No ready tasks.".to_string()
        } else if next_ready_tasks.len() == 1 {
            format!("Start with: {}.", next_ready_tasks[0])
        } else {
            format!(
                "Start with: {} (run in parallel).",
                next_ready_tasks.join(", ")
            )
        };

        let path_clause = if cpm.critical_path.is_empty() {
            String::new()
        } else {
            format!(
                " Then proceed through critical path: {}",
                cpm.critical_path.join(" \u{2192} ")
            )
        };

        format!("{}{}", start_clause, path_clause)
    };

    let mut output = json!({
        "project_duration": cpm.project_duration,
        "critical_path": cpm.critical_path,
        "parallel_groups": groups_json,
        "total_float": cpm.total_float,
        "bottlenecks": bottlenecks,
        "suggested_order": suggested_order,
        "next_ready_tasks": next_ready_tasks,
        "execution_hint": execution_hint,
        "optimization_moves": optimization_moves,
        "fast_track_moves": fast_track_json,
        "crash_moves": crash_json,
        "original_duration": original_duration,
        "optimized_duration": optimized_duration,
        "improvement_pct": improvement_pct,
        "applied": applied,
        "mode": if apply { "apply" } else { "analysis" },
    });

    if let Some(g) = genetic_json {
        output["genetic_optimization"] = g;
    }
    if !warnings.is_empty() {
        output["warnings"] = json!(warnings);
    }

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    Ok(())
}
