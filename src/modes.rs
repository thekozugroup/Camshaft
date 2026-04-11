use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum PlanMode {
    Sprint,
    Roadmap,
}

impl std::fmt::Display for PlanMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanMode::Sprint => write!(f, "sprint"),
            PlanMode::Roadmap => write!(f, "roadmap"),
        }
    }
}
