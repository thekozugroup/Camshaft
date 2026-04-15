use std::collections::HashMap;

use gantt_ml::monte_carlo::{DurationDistribution, MonteCarloConfig, MonteCarloEngine};
use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::plan::load_plan;

/// Run a Monte Carlo schedule risk analysis on the current plan.
///
/// Each task is given a Triangular distribution centered on its current
/// duration with +/- 20% uncertainty (min = 0.8x, most_likely = 1.0x,
/// max = 1.2x). The simulation runs `iterations` times and reports summary
/// statistics, a confidence interval at `confidence` level, a criticality
/// index per task, and narrative recommendations.
pub fn run(iterations: usize, confidence: f64) -> Result<()> {
    if iterations == 0 {
        return Err(CamshaftError::ValidationFailed(
            "iterations must be > 0".to_string(),
        ));
    }
    if !(0.0 < confidence && confidence < 1.0) {
        return Err(CamshaftError::ValidationFailed(
            "confidence must be strictly between 0 and 1".to_string(),
        ));
    }

    let plan = load_plan()?;
    let project = &plan.project;

    if project.activities.is_empty() {
        return Err(CamshaftError::ValidationFailed(
            "No activities in plan. Add tasks before running risk analysis.".to_string(),
        ));
    }

    // Build distributions: Triangular ±20% per task, default to effective duration
    let mut distributions: HashMap<String, DurationDistribution> = HashMap::new();
    for (id, activity) in &project.activities {
        let d = activity.effective_duration();
        // Skip zero/negative durations — use deterministic.
        if d <= 0.0 {
            distributions.insert(id.clone(), DurationDistribution::Deterministic(d.max(0.0)));
            continue;
        }
        distributions.insert(
            id.clone(),
            DurationDistribution::Triangular {
                min: d * 0.8,
                most_likely: d,
                max: d * 1.2,
            },
        );
    }

    // Build confidence levels: always include the requested level plus standard percentiles
    let mut levels: Vec<f64> = vec![0.5, 0.8, 0.95, 0.99, confidence];
    // Dedup (floats — use rounding to 4 decimals for key)
    levels.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    levels.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    let config = MonteCarloConfig::new(iterations).with_confidence_levels(levels);

    let result = MonteCarloEngine::run(project, &distributions, &config)
        .map_err(|e| CamshaftError::GanttMl(e.to_string()))?;

    // Lookup percentile by label (e.g. "P50", "P80"). If the precise requested
    // confidence level doesn't line up with a default label, fall back to
    // computing it from the sorted durations.
    let pct_label = |level: f64| -> String { format!("P{}", (level * 100.0).round() as u32) };
    let get_pct = |level: f64| -> f64 {
        let label = pct_label(level);
        if let Some(v) = result.percentiles.get(&label) {
            return *v;
        }
        let n = result.durations.len();
        if n == 0 {
            return 0.0;
        }
        let idx = ((level * n as f64) as usize).min(n - 1);
        result.durations[idx]
    };

    // Compute median (P50) explicitly
    let median = {
        let n = result.durations.len();
        if n == 0 {
            0.0
        } else if n % 2 == 1 {
            result.durations[n / 2]
        } else {
            (result.durations[n / 2 - 1] + result.durations[n / 2]) / 2.0
        }
    };

    let p50 = get_pct(0.50);
    let p80 = get_pct(0.80);
    let p95 = get_pct(0.95);
    let p99 = get_pct(0.99);

    // Confidence interval: symmetric around the median at the given level.
    // E.g. 0.80 -> [P10, P90].
    let tail = (1.0 - confidence) / 2.0;
    let lower_level = tail;
    let upper_level = 1.0 - tail;
    // These levels may not be in the default percentile map — compute from sorted durations.
    let n = result.durations.len();
    let pct_from_durations = |level: f64| -> f64 {
        if n == 0 {
            return 0.0;
        }
        let idx = ((level * n as f64) as usize).min(n - 1);
        result.durations[idx]
    };
    let ci_lower = pct_from_durations(lower_level);
    let ci_upper = pct_from_durations(upper_level);

    // Criticality index: sorted descending for deterministic output, but emit as an object
    let mut crit_sorted: Vec<(&String, &f64)> = result.activity_criticality.iter().collect();
    crit_sorted.sort_by(|a, b| {
        b.1.partial_cmp(a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(b.0))
    });
    let mut criticality_map = serde_json::Map::new();
    for (id, crit) in &crit_sorted {
        criticality_map.insert(
            (*id).clone(),
            serde_json::Value::from(round_to(**crit, 4)),
        );
    }

    // Recommendations
    let mut recommendations: Vec<String> = Vec::new();

    // Top critical tasks (>=0.80 critical)
    for (id, crit) in crit_sorted.iter().take(5) {
        if **crit >= 0.80 {
            let pct = (**crit * 100.0).round() as u32;
            // Try to use the task's display name
            let display = project
                .activities
                .get(*id)
                .map(|a| format!("'{}' ({})", a.name, id))
                .unwrap_or_else(|| format!("'{}'", id));
            recommendations.push(format!(
                "Task {} is critical in {}% of simulations — focus risk mitigation here",
                display, pct
            ));
        }
    }

    // Schedule risk summary at the requested confidence level
    let overrun_pct = ((1.0 - confidence) * 100.0).round() as u32;
    recommendations.push(format!(
        "Plan has {}% chance of exceeding {:.1} {}",
        overrun_pct,
        ci_upper,
        duration_unit(&plan.mode),
    ));

    // Variability signal
    let cv = if result.mean.abs() > f64::EPSILON {
        result.std_dev / result.mean
    } else {
        0.0
    };
    if cv > 0.15 {
        recommendations.push(format!(
            "High schedule variability (CV = {:.0}%) — consider adding buffer or reducing uncertainty on top critical tasks",
            cv * 100.0
        ));
    }

    // Output JSON
    let output = json!({
        "iterations": result.iterations,
        "duration_stats": {
            "mean": round_to(result.mean, 4),
            "median": round_to(median, 4),
            "std_dev": round_to(result.std_dev, 4),
            "min": round_to(result.min, 4),
            "max": round_to(result.max, 4),
            "p50": round_to(p50, 4),
            "p80": round_to(p80, 4),
            "p95": round_to(p95, 4),
            "p99": round_to(p99, 4),
        },
        "confidence_interval": {
            "level": confidence,
            "lower": round_to(ci_lower, 4),
            "upper": round_to(ci_upper, 4),
        },
        "criticality_index": serde_json::Value::Object(criticality_map),
        "recommendations": recommendations,
        "distribution": {
            "type": "triangular",
            "uncertainty": 0.20,
            "note": "Each task modeled as Triangular(0.8x, 1.0x, 1.2x) of its current duration.",
        }
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .map_err(|e| CamshaftError::Serialization(e.to_string()))?
    );

    Ok(())
}

fn round_to(v: f64, digits: u32) -> f64 {
    let factor = 10f64.powi(digits as i32);
    (v * factor).round() / factor
}

fn duration_unit(mode: &crate::modes::PlanMode) -> &'static str {
    match mode {
        crate::modes::PlanMode::Sprint => "hours",
        crate::modes::PlanMode::Roadmap => "days",
    }
}
