use std::fs;
use std::path::Path;

use gantt_ml::format::GanttMlFile;
use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::modes::PlanMode;
use crate::plan::{plan_exists, save_plan, CamshaftFile};

/// Maximum import file size: 50 MB (prevents OOM from malicious/corrupt files)
const MAX_IMPORT_SIZE: u64 = 50 * 1024 * 1024;

pub fn run(file: &str, force: bool) -> Result<()> {
    let path = Path::new(file);

    // Security: reject paths with ".." components to prevent traversal
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return Err(CamshaftError::ValidationFailed(
                "Import path must not contain '..' components.".to_string(),
            ));
        }
    }

    // Security: reject absolute paths — imports should stay relative to CWD
    if path.is_absolute() {
        return Err(CamshaftError::ValidationFailed(
            "Import path must be relative, not absolute.".to_string(),
        ));
    }

    if !path.exists() {
        return Err(CamshaftError::Io(format!(
            "Import file does not exist: {}",
            file
        )));
    }

    // Security: check file size before reading to prevent OOM
    let metadata = fs::metadata(path).map_err(|e| CamshaftError::Io(e.to_string()))?;
    if metadata.len() > MAX_IMPORT_SIZE {
        return Err(CamshaftError::ValidationFailed(format!(
            "Import file exceeds maximum size of {} MB.",
            MAX_IMPORT_SIZE / (1024 * 1024)
        )));
    }

    let contents = fs::read_to_string(path).map_err(|e| CamshaftError::Io(e.to_string()))?;

    // Check existence BEFORE parsing so caller gets the clearest error first.
    if plan_exists() && !force {
        return Err(CamshaftError::PlanAlreadyExists);
    }

    // Try parsing as full Camshaft format first.
    let imported: CamshaftFile = match serde_json::from_str::<CamshaftFile>(&contents) {
        Ok(cf) => cf,
        Err(_camshaft_err) => {
            // Fall back to GanttML envelope; wrap with default Sprint mode.
            match serde_json::from_str::<GanttMlFile>(&contents) {
                Ok(gml) => CamshaftFile::new(gml.project, PlanMode::Sprint),
                Err(e) => {
                    return Err(CamshaftError::Serialization(format!(
                        "File is neither a valid Camshaft nor GanttML plan: {}",
                        e
                    )));
                }
            }
        }
    };

    let task_count = imported.project.activities.len();
    let mode_str = imported.mode.to_string();

    save_plan(&imported)?;

    let output = json!({
        "status": "imported",
        "file": file,
        "task_count": task_count,
        "mode": mode_str,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}
