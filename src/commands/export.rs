use std::fs;
use std::path::Path;

use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::plan::load_plan;

pub fn run(file: Option<&str>) -> Result<()> {
    let plan = load_plan()?;

    let json_str =
        serde_json::to_string_pretty(&plan).map_err(|e| CamshaftError::Serialization(e.to_string()))?;

    match file {
        Some(path_str) => {
            let path = Path::new(path_str);

            // Security: reject paths with ".." components to prevent traversal
            for component in path.components() {
                if let std::path::Component::ParentDir = component {
                    return Err(CamshaftError::ValidationFailed(
                        "Export path must not contain '..' components.".to_string(),
                    ));
                }
            }

            // Security: reject absolute paths — exports should stay relative to CWD
            if path.is_absolute() {
                return Err(CamshaftError::ValidationFailed(
                    "Export path must be relative, not absolute.".to_string(),
                ));
            }

            fs::write(path, &json_str).map_err(|e| CamshaftError::Io(e.to_string()))?;
            let output = json!({
                "status": "exported",
                "file": path_str,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        None => {
            println!("{}", json_str);
        }
    }

    Ok(())
}
