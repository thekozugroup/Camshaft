mod commands;
mod error;
mod modes;
mod plan;

use clap::{Parser, Subcommand};

use crate::error::print_error;
use crate::modes::PlanMode;

#[derive(Parser)]
#[command(name = "camshaft", version, about = "GanttML-powered planning engine for AI code agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new plan
    Init {
        /// Plan name
        #[arg(long)]
        name: String,

        /// Planning mode
        #[arg(long, value_enum, default_value_t = PlanMode::Sprint)]
        mode: PlanMode,

        /// Project start date (YYYY-MM-DD)
        #[arg(long)]
        start: Option<String>,

        /// Overwrite existing plan
        #[arg(long, default_value_t = false)]
        force: bool,
    },

    /// Add tasks, dependencies, milestones, or resources
    Add {
        #[command(subcommand)]
        what: AddCommands,
    },

    /// Remove tasks or dependencies
    Remove {
        #[command(subcommand)]
        what: RemoveCommands,
    },

    /// Run CPM analysis and optimization
    Optimize {
        /// Optimization objective
        #[arg(long, default_value = "min-duration")]
        objective: String,

        /// Apply fast-tracking (convert FS to SS where safe)
        #[arg(long, default_value_t = false)]
        fast_track: bool,

        /// Apply crashing (compress critical path durations)
        #[arg(long, default_value_t = false)]
        crash: bool,
    },

    /// Query plan state without modifying
    Query {
        #[command(subcommand)]
        what: QueryCommands,
    },

    /// Sprint planning (agentic mode)
    Sprint {
        #[command(subcommand)]
        what: SprintCommands,
    },

    /// Validate plan integrity
    Validate,

    /// Export plan as JSON
    Export {
        /// Output file path (stdout if omitted)
        #[arg(long)]
        file: Option<String>,
    },
}

#[derive(Subcommand)]
enum AddCommands {
    /// Add a task
    Task {
        /// Task ID (short, unique)
        id: String,

        /// Task name
        #[arg(long)]
        name: String,

        /// Duration (hours in sprint mode, days in roadmap mode)
        #[arg(long)]
        duration: f64,

        /// Priority level
        #[arg(long, default_value = "medium")]
        priority: String,

        /// Task category
        #[arg(long)]
        category: Option<String>,
    },

    /// Add a dependency between tasks
    Dep {
        /// Predecessor task ID
        from: String,

        /// Successor task ID
        to: String,

        /// Dependency type (fs, ss, ff, sf)
        #[arg(long, default_value = "fs")]
        r#type: String,

        /// Lag in duration units
        #[arg(long, default_value_t = 0.0)]
        lag: f64,
    },

    /// Add a milestone
    Milestone {
        /// Milestone ID
        id: String,

        /// Milestone name
        #[arg(long)]
        name: String,
    },

    /// Add a resource
    Resource {
        /// Resource ID
        id: String,

        /// Resource name
        #[arg(long)]
        name: String,

        /// Resource type (labor, material, equipment)
        #[arg(long, default_value = "labor")]
        r#type: String,

        /// Max units available
        #[arg(long, default_value_t = 8.0)]
        units: f64,
    },

    /// Assign a resource to a task
    Assign {
        /// Task ID
        task: String,

        /// Resource ID
        resource: String,

        /// Units to assign
        #[arg(long, default_value_t = 1.0)]
        units: f64,
    },
}

#[derive(Subcommand)]
enum RemoveCommands {
    /// Remove a task and its dependencies
    Task {
        /// Task ID to remove
        id: String,
    },

    /// Remove a dependency
    Dep {
        /// Predecessor task ID
        from: String,

        /// Successor task ID
        to: String,
    },
}

#[derive(Subcommand)]
enum QueryCommands {
    /// Show the critical path
    CriticalPath,

    /// Show all tasks with computed dates and float
    Status,

    /// Identify bottleneck tasks (zero float)
    Bottlenecks,

    /// Show parallelizable task groups
    Parallel,

    /// What-if analysis: change a task's duration
    WhatIf {
        /// Task ID
        task: String,

        /// New duration
        #[arg(long)]
        duration: f64,
    },

    /// Suggest optimal execution order
    SuggestOrder,
}

#[derive(Subcommand)]
enum SprintCommands {
    /// Generate a sprint plan
    Plan {
        /// Total capacity in hours
        #[arg(long, default_value_t = 40.0)]
        capacity: f64,

        /// Hours per day
        #[arg(long, default_value_t = 8.0)]
        hours_per_day: f64,
    },

    /// Get AI-friendly daily schedule suggestion
    Suggest,

    /// Check for overcommitment
    OvercommitCheck {
        /// Hours per day
        #[arg(long, default_value_t = 8.0)]
        hours_per_day: f64,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { name, mode, start, force } => {
            commands::init::run(&name, mode, start.as_deref(), force)
        }
        Commands::Add { what } => match what {
            AddCommands::Task { id, name, duration, priority, category } => {
                commands::add::add_task(&id, &name, duration, &priority, category.as_deref())
            }
            AddCommands::Dep { from, to, r#type, lag } => {
                commands::add::add_dep(&from, &to, &r#type, lag)
            }
            AddCommands::Milestone { id, name } => {
                commands::add::add_milestone(&id, &name)
            }
            AddCommands::Resource { id, name, r#type, units } => {
                commands::add::add_resource(&id, &name, &r#type, units)
            }
            AddCommands::Assign { task, resource, units } => {
                commands::add::assign_resource(&task, &resource, units)
            }
        },
        Commands::Remove { what } => match what {
            RemoveCommands::Task { id } => commands::remove::remove_task(&id),
            RemoveCommands::Dep { from, to } => commands::remove::remove_dep(&from, &to),
        },
        Commands::Optimize { objective, fast_track, crash } => {
            commands::optimize::run(&objective, fast_track, crash)
        }
        Commands::Query { what } => match what {
            QueryCommands::CriticalPath => commands::query::critical_path(),
            QueryCommands::Status => commands::query::status(),
            QueryCommands::Bottlenecks => commands::query::bottlenecks(),
            QueryCommands::Parallel => commands::query::parallel(),
            QueryCommands::WhatIf { task, duration } => {
                commands::query::what_if(&task, duration)
            }
            QueryCommands::SuggestOrder => commands::query::suggest_order(),
        },
        Commands::Sprint { what } => match what {
            SprintCommands::Plan { capacity, hours_per_day } => {
                commands::sprint::plan(capacity, hours_per_day)
            }
            SprintCommands::Suggest => commands::sprint::suggest(),
            SprintCommands::OvercommitCheck { hours_per_day } => {
                commands::sprint::overcommit_check(hours_per_day)
            }
        },
        Commands::Validate => commands::validate::run(),
        Commands::Export { file } => commands::export::run(file.as_deref()),
    };

    if let Err(e) = result {
        print_error(e);
        std::process::exit(1);
    }
}
