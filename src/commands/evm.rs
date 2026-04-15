use chrono::{NaiveDate, Utc};
use gantt_ml::evm::EvmCalculator;
use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::plan::load_plan;

/// Pick a data date for the EVM calculation.
///
/// Preference order:
///  1. Latest `actual_finish` across activities
///  2. Latest `actual_start` across activities
///  3. Latest `planned_finish`
///  4. Today (UTC) as a last resort
fn choose_data_date(project: &gantt_ml::Project) -> NaiveDate {
    let mut latest: Option<NaiveDate> = None;
    let mut bump = |d: Option<NaiveDate>| {
        if let Some(d) = d {
            if latest.map(|cur| d > cur).unwrap_or(true) {
                latest = Some(d);
            }
        }
    };

    for (_, a) in &project.activities {
        bump(a.actual_finish);
        bump(a.actual_start);
        bump(a.planned_finish);
    }

    latest.unwrap_or_else(|| Utc::now().date_naive())
}

pub fn run() -> Result<()> {
    let plan = load_plan()?;

    if plan.project.activities.is_empty() {
        return Err(CamshaftError::ValidationFailed(
            "No activities in plan. Add tasks before running EVM.".to_string(),
        ));
    }

    let data_date = choose_data_date(&plan.project);

    let result = EvmCalculator::calculate(&plan.project, data_date)
        .map_err(|e| CamshaftError::GanttMl(e.to_string()))?;

    let interpretation = interpret(&result);

    let output = json!({
        "data_date": data_date.to_string(),
        "bac": round2(result.bac),
        "pv": round2(result.pv),
        "ev": round2(result.ev),
        "ac": round2(result.ac),
        "sv": round2(result.sv),
        "cv": round2(result.cv),
        "spi": round3(result.spi),
        "cpi": round3(result.cpi),
        "eac": round2(result.eac),
        "etc": round2(result.etc),
        "vac": round2(result.vac),
        "tcpi": round3(result.tcpi),
        "percent_complete": round2(result.percent_complete),
        "interpretation": interpretation,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    Ok(())
}

fn round2(v: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    (v * 100.0).round() / 100.0
}

fn round3(v: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    (v * 1000.0).round() / 1000.0
}

fn interpret(r: &gantt_ml::evm::EvmResult) -> String {
    // Schedule component
    let schedule = if r.pv <= 0.0 {
        "Schedule performance unavailable (no planned value yet).".to_string()
    } else if (r.spi - 1.0).abs() < 0.01 {
        format!("Project is on schedule (SPI={:.2}).", r.spi)
    } else if r.spi >= 1.0 {
        format!("Project is ahead of schedule (SPI={:.2}).", r.spi)
    } else {
        format!("Project is behind schedule (SPI={:.2}).", r.spi)
    };

    // Cost component
    let cost = if r.ac <= 0.0 {
        "No actual cost recorded yet.".to_string()
    } else if (r.cpi - 1.0).abs() < 0.01 {
        format!("On budget (CPI={:.2}).", r.cpi)
    } else if r.cpi >= 1.0 {
        format!("Under budget (CPI={:.2}).", r.cpi)
    } else {
        format!("Over budget (CPI={:.2}).", r.cpi)
    };

    // Forecast component
    let forecast = if r.bac > 0.0 && r.eac.is_finite() && r.eac > 0.0 {
        let delta = r.bac - r.eac;
        if delta.abs() < 0.01 {
            format!("Forecast completion cost: {:.1} (on budget).", r.eac)
        } else if delta > 0.0 {
            format!(
                "Forecast completion cost: {:.1} ({:.1} under budget).",
                r.eac, delta
            )
        } else {
            format!(
                "Forecast completion cost: {:.1} ({:.1} over budget).",
                r.eac,
                -delta
            )
        }
    } else {
        String::new()
    };

    let mut parts = vec![schedule, cost];
    if !forecast.is_empty() {
        parts.push(forecast);
    }
    parts.join(" ")
}
