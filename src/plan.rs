use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use gantt_ml::Project;
use serde::{Deserialize, Serialize};

use crate::error::{CamshaftError, Result};
use crate::modes::PlanMode;

const PLAN_DIR: &str = ".camshaft";
const PLAN_FILE: &str = "plan.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct CamshaftFile {
    pub version: String,
    pub format: String,
    pub mode: PlanMode,
    pub created_at: DateTime<Utc>,
    pub project: Project,
    #[serde(default)]
    pub meta: CamshaftMeta,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamshaftMeta {
    pub last_optimized: Option<DateTime<Utc>>,
    pub optimization_runs: u32,
    #[serde(default)]
    pub parallelism_groups: Vec<Vec<String>>,
}

impl CamshaftFile {
    pub fn new(project: Project, mode: PlanMode) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            format: "camshaft".to_string(),
            mode,
            created_at: Utc::now(),
            project,
            meta: CamshaftMeta::default(),
        }
    }
}

fn plan_dir() -> PathBuf {
    PathBuf::from(PLAN_DIR)
}

fn plan_path() -> PathBuf {
    plan_dir().join(PLAN_FILE)
}

pub fn plan_exists() -> bool {
    plan_path().exists()
}

pub fn save_plan(file: &CamshaftFile) -> Result<()> {
    let dir = plan_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| CamshaftError::Io(e.to_string()))?;
    }
    let json = serde_json::to_string_pretty(file)
        .map_err(|e| CamshaftError::Serialization(e.to_string()))?;
    fs::write(plan_path(), json).map_err(|e| CamshaftError::Io(e.to_string()))?;
    Ok(())
}

/// Maximum plan file size: 50 MB (prevents OOM from malicious/corrupt files)
const MAX_PLAN_SIZE: u64 = 50 * 1024 * 1024;

pub fn load_plan() -> Result<CamshaftFile> {
    let path = plan_path();
    if !path.exists() {
        return Err(CamshaftError::NoPlan);
    }

    // Security: check file size before reading to prevent OOM
    let metadata = fs::metadata(&path).map_err(|e| CamshaftError::Io(e.to_string()))?;
    if metadata.len() > MAX_PLAN_SIZE {
        return Err(CamshaftError::ValidationFailed(format!(
            "Plan file exceeds maximum size of {} MB.",
            MAX_PLAN_SIZE / (1024 * 1024)
        )));
    }

    let contents = fs::read_to_string(&path).map_err(|e| CamshaftError::Io(e.to_string()))?;
    let file: CamshaftFile =
        serde_json::from_str(&contents).map_err(|e| CamshaftError::Serialization(e.to_string()))?;
    Ok(file)
}

pub fn load_plan_from(path: &Path) -> Result<CamshaftFile> {
    let contents = fs::read_to_string(path).map_err(|e| CamshaftError::Io(e.to_string()))?;
    let file: CamshaftFile =
        serde_json::from_str(&contents).map_err(|e| CamshaftError::Serialization(e.to_string()))?;
    Ok(file)
}
