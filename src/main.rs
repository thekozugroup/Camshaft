mod commands;
mod critical_chain;
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

        /// Write optimized plan back to .camshaft/plan.json (destructive).
        /// Without this flag the command is read-only and only reports analysis.
        #[arg(long, default_value_t = false)]
        apply: bool,
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

    /// Task-level operations (complete, reopen, status)
    Task {
        #[command(subcommand)]
        what: TaskCommands,
    },

    /// Validate plan integrity
    Validate,

    /// Export plan as JSON
    Export {
        /// Output file path (stdout if omitted)
        #[arg(long)]
        file: Option<String>,
    },

    /// Monte Carlo schedule risk analysis
    RiskAnalysis {
        /// Number of simulation iterations
        #[arg(long, default_value_t = 10000)]
        iterations: usize,

        /// Confidence level for interval (e.g. 0.80 = 80%)
        #[arg(long, default_value_t = 0.80)]
        confidence: f64,
    },

    /// Import a plan from a Camshaft or GanttML JSON file
    Import {
        /// Input file path (relative, no '..' components)
        #[arg(long)]
        file: String,

        /// Overwrite existing plan
        #[arg(long, default_value_t = false)]
        force: bool,
    },

    /// Analyze schedule quality and health (GanttML-backed)
    Analyze,

    /// Earned Value Management metrics (PV, EV, AC, SPI, CPI, EAC, ...)
    Evm,

    /// Diff the current plan against a baseline plan file
    Diff {
        /// Baseline plan file path (relative, no '..' components)
        #[arg(long)]
        baseline: String,
    },

    /// Estimate task durations from git history
    Velocity {
        /// Repo path (relative, no '..' components). Defaults to current directory.
        #[arg(long)]
        repo: Option<String>,

        /// Analysis window in days
        #[arg(long, default_value_t = 90)]
        days: u32,
    },

    /// Detect and resolve resource over-allocation (resource leveling)
    LevelResources {
        /// Persist the leveled plan (otherwise analysis-only)
        #[arg(long, default_value_t = false)]
        apply: bool,
    },

    /// Bulk-create a plan from a YAML or JSON file
    Bulk {
        /// Input file path (relative, no '..' components; .yaml/.yml/.json)
        #[arg(long)]
        file: String,

        /// Overwrite existing plan
        #[arg(long, default_value_t = false)]
        force: bool,
    },

    /// Scaffold a plan from a built-in template
    Template {
        /// Template name (e.g. feature-impl, bug-fix, migration, launch, research-spike)
        #[arg(default_value = "")]
        template: String,

        /// Plan name (used for plan metadata)
        #[arg(long, default_value = "")]
        name: String,

        /// Output as YAML file instead of creating the plan directly
        #[arg(long)]
        output: Option<String>,

        /// List available templates
        #[arg(long, default_value_t = false)]
        list: bool,
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

    /// List tasks that are ready to work on (all predecessors completed)
    Ready,
}

#[derive(Subcommand)]
enum TaskCommands {
    /// Mark a task as completed
    Complete {
        /// Task ID
        id: String,
    },

    /// Reopen a previously completed task
    Reopen {
        /// Task ID
        id: String,
    },

    /// Show a single task's status
    Status {
        /// Task ID
        id: String,
    },
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
    Suggest {
        /// Optional total sprint capacity in hours. When provided, lowest-
        /// priority tasks are descoped first and per-day hours are capped at
        /// capacity / horizon_days.
        #[arg(long)]
        capacity: Option<f64>,

        /// Working-day horizon used with --capacity. Defaults to 10.
        #[arg(long, default_value_t = 10)]
        horizon_days: usize,
    },

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
        Commands::Optimize { objective, fast_track, crash, apply } => {
            commands::optimize::run(&objective, fast_track, crash, apply)
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
            QueryCommands::Ready => commands::query::ready(),
        },
        Commands::Sprint { what } => match what {
            SprintCommands::Plan { capacity, hours_per_day } => {
                commands::sprint::plan(capacity, hours_per_day)
            }
            SprintCommands::Suggest { capacity, horizon_days } => {
                commands::sprint::suggest(capacity, horizon_days)
            }
            SprintCommands::OvercommitCheck { hours_per_day } => {
                commands::sprint::overcommit_check(hours_per_day)
            }
        },
        Commands::Task { what } => match what {
            TaskCommands::Complete { id } => commands::task::complete(&id),
            TaskCommands::Reopen { id } => commands::task::reopen(&id),
            TaskCommands::Status { id } => commands::task::status(&id),
        },
        Commands::Validate => commands::validate::run(),
        Commands::Export { file } => commands::export::run(file.as_deref()),
        Commands::RiskAnalysis { iterations, confidence } => {
            commands::risk::run(iterations, confidence)
        }
        Commands::Import { file, force } => {
            commands::import::run(&file, force)
        }
        Commands::Analyze => commands::analyze::run(),
        Commands::Evm => commands::evm::run(),
        Commands::Diff { baseline } => commands::diff::run(&baseline),
        Commands::Velocity { repo, days } => {
            commands::velocity::run(repo.as_deref(), days)
        }
        Commands::LevelResources { apply } => commands::level::run(apply),
        Commands::Bulk { file, force } => commands::bulk::run(&file, force),
        Commands::Template { template, name, output, list } => {
            if list {
                commands::template::list()
            } else if template.is_empty() {
                Err(crate::error::CamshaftError::ValidationFailed(
                    "Template name required. Pass a template name or --list to see options.".to_string(),
                ))
            } else if name.is_empty() {
                Err(crate::error::CamshaftError::ValidationFailed(
                    "Plan name required. Pass --name <plan name>.".to_string(),
                ))
            } else {
                commands::template::run(&template, &name, output.as_deref())
            }
        }
    };

    if let Err(e) = result {
        print_error(e);
        std::process::exit(1);
    }
}
