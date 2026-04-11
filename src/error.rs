use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum CamshaftError {
    #[error("No plan found. Run `camshaft init` first.")]
    NoPlan,

    #[error("Cycle detected: {0}")]
    CycleDetected(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Duplicate task: {0}")]
    DuplicateTask(String),

    #[error("Dependency references nonexistent task: {predecessor} -> {successor}")]
    InvalidDependency {
        predecessor: String,
        successor: String,
    },

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Optimization failed: {0}")]
    OptimizationFailed(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("GanttML error: {0}")]
    GanttMl(String),

    #[error("Plan already exists. Use --force to overwrite.")]
    PlanAlreadyExists,
}

pub type Result<T> = std::result::Result<T, CamshaftError>;

#[derive(Serialize)]
pub struct ErrorOutput {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub affected_tasks: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl From<CamshaftError> for ErrorOutput {
    fn from(err: CamshaftError) -> Self {
        let (error_type, suggestion, affected) = match &err {
            CamshaftError::NoPlan => (
                "no_plan",
                Some("Run `camshaft init` to create a plan.".to_string()),
                vec![],
            ),
            CamshaftError::CycleDetected(msg) => (
                "cycle_detected",
                Some("Remove one dependency to break the cycle.".to_string()),
                parse_cycle_tasks(msg),
            ),
            CamshaftError::TaskNotFound(id) => (
                "task_not_found",
                Some(format!("Check task ID. Available tasks can be listed with `camshaft query status`.")),
                vec![id.clone()],
            ),
            CamshaftError::DuplicateTask(id) => (
                "duplicate_task",
                Some(format!("Use a different ID or remove the existing task first.")),
                vec![id.clone()],
            ),
            CamshaftError::InvalidDependency { predecessor, successor } => (
                "invalid_dependency",
                Some("Ensure both tasks exist before adding a dependency.".to_string()),
                vec![predecessor.clone(), successor.clone()],
            ),
            CamshaftError::ValidationFailed(_) => (
                "validation_failed",
                Some("Fix the reported issues and try again.".to_string()),
                vec![],
            ),
            CamshaftError::OptimizationFailed(_) => (
                "optimization_failed",
                Some("Check plan structure. Ensure no cycles and at least one task.".to_string()),
                vec![],
            ),
            CamshaftError::Serialization(_) => ("serialization_error", None, vec![]),
            CamshaftError::Io(_) => ("io_error", None, vec![]),
            CamshaftError::GanttMl(_) => ("gantt_ml_error", None, vec![]),
            CamshaftError::PlanAlreadyExists => (
                "plan_already_exists",
                Some("Use --force to overwrite.".to_string()),
                vec![],
            ),
        };

        ErrorOutput {
            error: error_type.to_string(),
            message: err.to_string(),
            affected_tasks: affected,
            suggestion: suggestion,
        }
    }
}

fn parse_cycle_tasks(msg: &str) -> Vec<String> {
    msg.split(" -> ")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn print_error(err: CamshaftError) {
    let output = ErrorOutput::from(err);
    if let Ok(json) = serde_json::to_string_pretty(&output) {
        eprintln!("{}", json);
    }
}
