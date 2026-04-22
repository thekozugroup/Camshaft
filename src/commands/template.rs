// template command — scaffolds common plan types

use std::fs;
use std::path::Path;

use gantt_ml::model::activity::Activity;
use gantt_ml::model::dependency::Dependency;
use gantt_ml::model::types::ActivityType;
use gantt_ml::Project;
use serde_json::json;

use crate::error::{CamshaftError, Result};
use crate::modes::PlanMode;
use crate::plan::{plan_exists, save_plan, CamshaftFile};

struct TemplateTask {
    id: &'static str,
    name: &'static str,
    duration: f64,
    is_milestone: bool,
}

struct TemplateDep {
    from: &'static str,
    to: &'static str,
}

struct TemplateSpec {
    name: &'static str,
    description: &'static str,
    tasks: &'static [TemplateTask],
    deps: &'static [TemplateDep],
}

const FEATURE_IMPL: TemplateSpec = TemplateSpec {
    name: "feature-impl",
    description: "Standard feature implementation: design, build, test, document, review.",
    tasks: &[
        TemplateTask { id: "design-schema", name: "Design schema", duration: 2.0, is_milestone: false },
        TemplateTask { id: "impl-core", name: "Implement core", duration: 4.0, is_milestone: false },
        TemplateTask { id: "write-unit-tests", name: "Write unit tests", duration: 2.0, is_milestone: false },
        TemplateTask { id: "write-integration-tests", name: "Write integration tests", duration: 2.0, is_milestone: false },
        TemplateTask { id: "update-docs", name: "Update docs", duration: 1.0, is_milestone: false },
        TemplateTask { id: "code-review", name: "Code review", duration: 1.0, is_milestone: false },
    ],
    deps: &[
        TemplateDep { from: "design-schema", to: "impl-core" },
        TemplateDep { from: "impl-core", to: "write-unit-tests" },
        TemplateDep { from: "impl-core", to: "write-integration-tests" },
        TemplateDep { from: "write-unit-tests", to: "code-review" },
        TemplateDep { from: "write-integration-tests", to: "code-review" },
        TemplateDep { from: "impl-core", to: "update-docs" },
    ],
};

const BUG_FIX: TemplateSpec = TemplateSpec {
    name: "bug-fix",
    description: "Investigation and fix cycle: reproduce, diagnose, fix, verify, review.",
    tasks: &[
        TemplateTask { id: "reproduce", name: "Reproduce bug", duration: 1.0, is_milestone: false },
        TemplateTask { id: "diagnose", name: "Diagnose root cause", duration: 2.0, is_milestone: false },
        TemplateTask { id: "fix", name: "Implement fix", duration: 2.0, is_milestone: false },
        TemplateTask { id: "test-fix", name: "Test fix", duration: 1.0, is_milestone: false },
        TemplateTask { id: "regression-test", name: "Regression test", duration: 1.0, is_milestone: false },
        TemplateTask { id: "code-review", name: "Code review", duration: 1.0, is_milestone: false },
    ],
    deps: &[
        TemplateDep { from: "reproduce", to: "diagnose" },
        TemplateDep { from: "diagnose", to: "fix" },
        TemplateDep { from: "fix", to: "test-fix" },
        TemplateDep { from: "test-fix", to: "regression-test" },
        TemplateDep { from: "regression-test", to: "code-review" },
    ],
};

const MIGRATION: TemplateSpec = TemplateSpec {
    name: "migration",
    description: "Data or system migration: analyze, design, build, dry-run, execute, verify.",
    tasks: &[
        TemplateTask { id: "analyze-current", name: "Analyze current state", duration: 4.0, is_milestone: false },
        TemplateTask { id: "design-target", name: "Design target state", duration: 3.0, is_milestone: false },
        TemplateTask { id: "build-migrator", name: "Build migrator", duration: 6.0, is_milestone: false },
        TemplateTask { id: "dry-run", name: "Dry run", duration: 2.0, is_milestone: false },
        TemplateTask { id: "validate", name: "Validate dry-run output", duration: 2.0, is_milestone: false },
        TemplateTask { id: "execute", name: "Execute migration", duration: 3.0, is_milestone: false },
        TemplateTask { id: "verify", name: "Verify migration", duration: 2.0, is_milestone: false },
    ],
    deps: &[
        TemplateDep { from: "analyze-current", to: "design-target" },
        TemplateDep { from: "design-target", to: "build-migrator" },
        TemplateDep { from: "build-migrator", to: "dry-run" },
        TemplateDep { from: "dry-run", to: "validate" },
        TemplateDep { from: "validate", to: "execute" },
        TemplateDep { from: "execute", to: "verify" },
    ],
};

const LAUNCH: TemplateSpec = TemplateSpec {
    name: "launch",
    description: "Feature launch workflow with launch-ready milestone gate.",
    tasks: &[
        TemplateTask { id: "qa-signoff", name: "QA sign-off", duration: 2.0, is_milestone: false },
        TemplateTask { id: "stakeholder-demo", name: "Stakeholder demo", duration: 1.0, is_milestone: false },
        TemplateTask { id: "docs-final", name: "Finalize docs", duration: 2.0, is_milestone: false },
        TemplateTask { id: "launch-ready", name: "Launch ready", duration: 0.0, is_milestone: true },
        TemplateTask { id: "feature-flag-enable", name: "Enable feature flag", duration: 0.5, is_milestone: false },
        TemplateTask { id: "monitor", name: "Monitor rollout", duration: 2.0, is_milestone: false },
        TemplateTask { id: "retrospective", name: "Retrospective", duration: 1.0, is_milestone: false },
    ],
    deps: &[
        TemplateDep { from: "qa-signoff", to: "launch-ready" },
        TemplateDep { from: "stakeholder-demo", to: "launch-ready" },
        TemplateDep { from: "launch-ready", to: "feature-flag-enable" },
        TemplateDep { from: "feature-flag-enable", to: "monitor" },
        TemplateDep { from: "monitor", to: "retrospective" },
    ],
};

const RESEARCH_SPIKE: TemplateSpec = TemplateSpec {
    name: "research-spike",
    description: "Time-boxed investigation: question, gather, experiment, document, share.",
    tasks: &[
        TemplateTask { id: "define-questions", name: "Define questions", duration: 1.0, is_milestone: false },
        TemplateTask { id: "gather-data", name: "Gather data", duration: 3.0, is_milestone: false },
        TemplateTask { id: "experiment", name: "Run experiments", duration: 4.0, is_milestone: false },
        TemplateTask { id: "document-findings", name: "Document findings", duration: 2.0, is_milestone: false },
        TemplateTask { id: "share-results", name: "Share results", duration: 1.0, is_milestone: false },
    ],
    deps: &[
        TemplateDep { from: "define-questions", to: "gather-data" },
        TemplateDep { from: "gather-data", to: "experiment" },
        TemplateDep { from: "experiment", to: "document-findings" },
        TemplateDep { from: "document-findings", to: "share-results" },
    ],
};

const ALL_TEMPLATES: &[&TemplateSpec] = &[
    &FEATURE_IMPL,
    &BUG_FIX,
    &MIGRATION,
    &LAUNCH,
    &RESEARCH_SPIKE,
];

fn find_template(name: &str) -> Option<&'static TemplateSpec> {
    ALL_TEMPLATES.iter().find(|t| t.name == name).copied()
}

pub fn list() -> Result<()> {
    let items: Vec<serde_json::Value> = ALL_TEMPLATES
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "tasks": t.tasks.len(),
                "dependencies": t.deps.len(),
            })
        })
        .collect();
    let output = json!({ "templates": items });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    Ok(())
}

pub fn run(template_name: &str, plan_name: &str, output_file: Option<&str>) -> Result<()> {
    let spec = find_template(template_name).ok_or_else(|| {
        let names: Vec<&str> = ALL_TEMPLATES.iter().map(|t| t.name).collect();
        CamshaftError::ValidationFailed(format!(
            "Unknown template: {}. Available: {}",
            template_name,
            names.join(", ")
        ))
    })?;

    if let Some(path_str) = output_file {
        // YAML output path — write a bulk-compatible YAML file, no plan.json.
        write_yaml(spec, plan_name, path_str)?;
        let output = json!({
            "status": "written",
            "template": spec.name,
            "plan_name": plan_name,
            "file": path_str,
            "tasks": spec.tasks.len(),
            "dependencies": spec.deps.len(),
            "next_action": format!(
                "Edit {} then run 'camshaft bulk --file {}' to create the plan.",
                path_str, path_str
            ),
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        return Ok(());
    }

    // Direct plan creation
    if plan_exists() {
        return Err(CamshaftError::PlanAlreadyExists);
    }

    let short_id = &uuid::Uuid::new_v4().to_string()[..8];
    let mut project = Project::new(short_id, plan_name);

    let mut tasks_created = 0usize;
    for t in spec.tasks {
        let mut activity = Activity::new(t.id, t.name, t.duration);
        if t.is_milestone {
            activity.activity_type = ActivityType::FinishMilestone;
        }
        project.activities.insert(t.id.to_string(), activity);
        tasks_created += 1;
    }

    let mut deps_created = 0usize;
    for d in spec.deps {
        let dep = Dependency::finish_to_start(d.from, d.to);
        project.dependencies.push(dep);
        deps_created += 1;
    }

    let camshaft_file = CamshaftFile::new(project, PlanMode::Sprint);
    save_plan(&camshaft_file)?;

    let output = json!({
        "status": "created",
        "template": spec.name,
        "plan_name": plan_name,
        "tasks_created": tasks_created,
        "dependencies_created": deps_created,
        "next_action": "Run 'camshaft validate' then 'camshaft optimize' to get execution order"
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    Ok(())
}

fn write_yaml(spec: &TemplateSpec, plan_name: &str, path_str: &str) -> Result<()> {
    let path = Path::new(path_str);

    // Security: reject paths with ".." components
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return Err(CamshaftError::ValidationFailed(
                "Output path must not contain '..' components.".to_string(),
            ));
        }
    }

    if path.is_absolute() {
        return Err(CamshaftError::ValidationFailed(
            "Output path must be relative, not absolute.".to_string(),
        ));
    }

    let mut yaml = String::new();
    yaml.push_str(&format!("name: {}\n", yaml_escape(plan_name)));
    yaml.push_str("mode: sprint\n");

    // Separate tasks vs milestones
    let tasks: Vec<&TemplateTask> = spec.tasks.iter().filter(|t| !t.is_milestone).collect();
    let milestones: Vec<&TemplateTask> = spec.tasks.iter().filter(|t| t.is_milestone).collect();

    yaml.push_str("tasks:\n");
    for t in &tasks {
        yaml.push_str(&format!("  - id: {}\n", t.id));
        yaml.push_str(&format!("    name: {}\n", yaml_escape(t.name)));
        yaml.push_str(&format!("    duration: {}\n", t.duration));
    }

    if !milestones.is_empty() {
        yaml.push_str("milestones:\n");
        for m in &milestones {
            yaml.push_str(&format!("  - id: {}\n", m.id));
            yaml.push_str(&format!("    name: {}\n", yaml_escape(m.name)));
        }
    }

    if !spec.deps.is_empty() {
        yaml.push_str("dependencies:\n");
        for d in spec.deps {
            yaml.push_str(&format!("  - from: {}\n    to: {}\n", d.from, d.to));
        }
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| CamshaftError::Io(e.to_string()))?;
        }
    }
    fs::write(path, yaml).map_err(|e| CamshaftError::Io(e.to_string()))?;
    Ok(())
}

fn yaml_escape(s: &str) -> String {
    // Quote if contains special YAML chars
    if s.contains(':') || s.contains('#') || s.contains('"') || s.starts_with(' ') || s.starts_with('-') {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}
