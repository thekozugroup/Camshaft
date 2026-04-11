use chrono::NaiveDate;
use gantt_ml::Project;
use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::modes::PlanMode;
use crate::plan::{plan_exists, save_plan, CamshaftFile};

pub fn run(name: &str, mode: PlanMode, start: Option<&str>, force: bool) -> Result<()> {
    if plan_exists() && !force {
        return Err(CamshaftError::PlanAlreadyExists);
    }

    let short_id = &uuid::Uuid::new_v4().to_string()[..8];
    let mut project = Project::new(short_id, name);

    if let Some(date_str) = start {
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|e| CamshaftError::ValidationFailed(format!("Invalid date: {e}")))?;
        project.planned_start = Some(date);
    }

    let file = CamshaftFile::new(project, mode);
    save_plan(&file)?;

    let output = json!({
        "status": "created",
        "name": name,
        "mode": mode.to_string(),
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}
