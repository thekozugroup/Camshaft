use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn camshaft() -> Command {
    Command::cargo_bin("camshaft").unwrap()
}

fn setup_test_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    dir
}

#[test]
fn test_help() {
    camshaft()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("GanttML-powered planning engine"));
}

#[test]
fn test_version() {
    camshaft()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("camshaft"));
}

#[test]
fn test_init_sprint() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Test Sprint", "--mode", "sprint"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"created\""))
        .stdout(predicate::str::contains("\"mode\": \"sprint\""));

    assert!(dir.path().join(".camshaft/plan.json").exists());
}

#[test]
fn test_init_roadmap() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Q2 Roadmap", "--mode", "roadmap", "--start", "2026-05-01"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"mode\": \"roadmap\""));
}

#[test]
fn test_init_duplicate_fails() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Test", "--mode", "sprint"])
        .assert()
        .success();

    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Test2", "--mode", "sprint"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("plan_already_exists"));
}

#[test]
fn test_init_force_overwrite() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Test", "--mode", "sprint"])
        .assert()
        .success();

    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Test2", "--mode", "roadmap", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"mode\": \"roadmap\""));
}

#[test]
fn test_add_task() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Test", "--mode", "sprint"])
        .assert()
        .success();

    camshaft()
        .current_dir(dir.path())
        .args(["add", "task", "t1", "--name", "Task One", "--duration", "5"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\": \"t1\""));
}

#[test]
fn test_add_duplicate_task_fails() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Test", "--mode", "sprint"])
        .assert()
        .success();

    camshaft()
        .current_dir(dir.path())
        .args(["add", "task", "t1", "--name", "Task One", "--duration", "5"])
        .assert()
        .success();

    camshaft()
        .current_dir(dir.path())
        .args(["add", "task", "t1", "--name", "Duplicate", "--duration", "3"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate_task"));
}

#[test]
fn test_add_dependency() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Test", "--mode", "sprint"])
        .assert()
        .success();

    camshaft()
        .current_dir(dir.path())
        .args(["add", "task", "a", "--name", "Task A", "--duration", "3"])
        .assert()
        .success();
    camshaft()
        .current_dir(dir.path())
        .args(["add", "task", "b", "--name", "Task B", "--duration", "5"])
        .assert()
        .success();

    camshaft()
        .current_dir(dir.path())
        .args(["add", "dep", "a", "b"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"type\": \"dependency\""));
}

#[test]
fn test_add_dep_missing_task_fails() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Test", "--mode", "sprint"])
        .assert()
        .success();

    camshaft()
        .current_dir(dir.path())
        .args(["add", "task", "a", "--name", "A", "--duration", "1"])
        .assert()
        .success();

    camshaft()
        .current_dir(dir.path())
        .args(["add", "dep", "a", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid_dependency"));
}

#[test]
fn test_add_milestone() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Test", "--mode", "sprint"])
        .assert()
        .success();

    camshaft()
        .current_dir(dir.path())
        .args(["add", "milestone", "m1", "--name", "MVP"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"type\": \"milestone\""));
}

#[test]
fn test_remove_task() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Test", "--mode", "sprint"])
        .assert()
        .success();

    camshaft()
        .current_dir(dir.path())
        .args(["add", "task", "t1", "--name", "T1", "--duration", "3"])
        .assert()
        .success();

    camshaft()
        .current_dir(dir.path())
        .args(["remove", "task", "t1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"removed\""));
}

#[test]
fn test_validate_clean_plan() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Test", "--mode", "sprint"])
        .assert()
        .success();

    camshaft()
        .current_dir(dir.path())
        .args(["add", "task", "a", "--name", "A", "--duration", "3"])
        .assert()
        .success();
    camshaft()
        .current_dir(dir.path())
        .args(["add", "task", "b", "--name", "B", "--duration", "5"])
        .assert()
        .success();
    camshaft()
        .current_dir(dir.path())
        .args(["add", "dep", "a", "b"])
        .assert()
        .success();

    camshaft()
        .current_dir(dir.path())
        .args(["validate"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"valid\": true"));
}

#[test]
fn test_full_optimize_pipeline() {
    let dir = setup_test_dir();
    // Init
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Full Test", "--mode", "sprint"])
        .assert().success();

    // Add tasks
    for (id, name, dur) in [("a", "Design", "4"), ("b", "Implement", "8"), ("c", "Test", "6"), ("d", "Deploy", "2")] {
        camshaft().current_dir(dir.path())
            .args(["add", "task", id, "--name", name, "--duration", dur])
            .assert().success();
    }

    // Add deps: a->b, a->c, b->d, c->d
    for (from, to) in [("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")] {
        camshaft().current_dir(dir.path())
            .args(["add", "dep", from, to])
            .assert().success();
    }

    // Optimize
    camshaft().current_dir(dir.path())
        .args(["optimize"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"project_duration\""))
        .stdout(predicate::str::contains("\"critical_path\""))
        .stdout(predicate::str::contains("\"parallel_groups\""))
        .stdout(predicate::str::contains("\"suggested_order\""));

    // Query critical path
    camshaft().current_dir(dir.path())
        .args(["query", "critical-path"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"project_duration\": 14.0"));

    // Query parallel groups — b and c should be parallel
    camshaft().current_dir(dir.path())
        .args(["query", "parallel"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"parallel_groups\""));

    // What-if: double task b
    camshaft().current_dir(dir.path())
        .args(["query", "what-if", "b", "--duration", "16"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"impact\""));

    // Suggest order
    camshaft().current_dir(dir.path())
        .args(["query", "suggest-order"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"execution_order\""));
}

#[test]
fn test_sprint_planning() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Sprint Test", "--mode", "sprint"])
        .assert().success();

    for (id, name, dur) in [("t1", "Task 1", "4"), ("t2", "Task 2", "6"), ("t3", "Task 3", "3")] {
        camshaft().current_dir(dir.path())
            .args(["add", "task", id, "--name", name, "--duration", dur])
            .assert().success();
    }

    camshaft().current_dir(dir.path())
        .args(["add", "dep", "t1", "t2"])
        .assert().success();

    camshaft().current_dir(dir.path())
        .args(["sprint", "plan", "--capacity", "40"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"sprint\""))
        .stdout(predicate::str::contains("\"allocated_hours\""));

    camshaft().current_dir(dir.path())
        .args(["sprint", "overcommit-check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"overcommitted\""));
}

#[test]
fn test_export() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Export Test", "--mode", "sprint"])
        .assert().success();

    camshaft().current_dir(dir.path())
        .args(["add", "task", "x", "--name", "X", "--duration", "5"])
        .assert().success();

    // Export to stdout
    camshaft().current_dir(dir.path())
        .args(["export"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"format\": \"camshaft\""));

    // Export to file (relative path — security requires no absolute paths)
    camshaft().current_dir(dir.path())
        .args(["export", "--file", "exported.json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"exported\""));

    assert!(dir.path().join("exported.json").exists());
}

#[test]
fn test_no_plan_error() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["add", "task", "t1", "--name", "T", "--duration", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no_plan"));
}

#[test]
fn test_resource_workflow() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Resource Test", "--mode", "sprint"])
        .assert().success();

    camshaft().current_dir(dir.path())
        .args(["add", "task", "t1", "--name", "Task", "--duration", "5"])
        .assert().success();

    camshaft().current_dir(dir.path())
        .args(["add", "resource", "r1", "--name", "Dev Agent", "--type", "labor", "--units", "8"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"type\": \"resource\""));

    camshaft().current_dir(dir.path())
        .args(["add", "assign", "t1", "r1", "--units", "4"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"type\": \"resource_assignment\""));
}

#[test]
fn test_export_rejects_absolute_path() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Test", "--mode", "sprint"])
        .assert().success();

    camshaft().current_dir(dir.path())
        .args(["export", "--file", "/tmp/evil.json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("validation_failed"));
}

#[test]
fn test_export_rejects_path_traversal() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Test", "--mode", "sprint"])
        .assert().success();

    camshaft().current_dir(dir.path())
        .args(["export", "--file", "../../etc/evil.json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("validation_failed"));
}

#[test]
fn test_task_complete_and_ready() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Task Complete Test", "--mode", "sprint"])
        .assert().success();

    // diamond: A -> B, A -> C, B -> D, C -> D
    for (id, name, dur) in [("A", "A", "4"), ("B", "B", "6"), ("C", "C", "8"), ("D", "D", "3")] {
        camshaft().current_dir(dir.path())
            .args(["add", "task", id, "--name", name, "--duration", dur])
            .assert().success();
    }
    for (from, to) in [("A", "B"), ("A", "C"), ("B", "D"), ("C", "D")] {
        camshaft().current_dir(dir.path())
            .args(["add", "dep", from, to])
            .assert().success();
    }

    // Initially only A should be ready
    camshaft().current_dir(dir.path())
        .args(["query", "ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\": \"A\""));

    // Complete A
    camshaft().current_dir(dir.path())
        .args(["task", "complete", "A"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"completed\""));

    // Now B and C should be ready
    camshaft().current_dir(dir.path())
        .args(["query", "ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\": \"B\""))
        .stdout(predicate::str::contains("\"id\": \"C\""));
}

#[test]
fn test_optimize_includes_next_ready_tasks() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Next Ready Test", "--mode", "sprint"])
        .assert().success();
    camshaft().current_dir(dir.path())
        .args(["add", "task", "a", "--name", "A", "--duration", "3"])
        .assert().success();
    camshaft().current_dir(dir.path())
        .args(["add", "task", "b", "--name", "B", "--duration", "5"])
        .assert().success();
    camshaft().current_dir(dir.path())
        .args(["add", "dep", "a", "b"])
        .assert().success();

    camshaft().current_dir(dir.path())
        .args(["optimize"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"next_ready_tasks\""))
        .stdout(predicate::str::contains("\"execution_hint\""));
}

#[test]
fn test_analyze() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Analyze Test", "--mode", "sprint"])
        .assert().success();
    camshaft().current_dir(dir.path())
        .args(["add", "task", "t1", "--name", "T1", "--duration", "5"])
        .assert().success();

    camshaft().current_dir(dir.path())
        .args(["analyze"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"health_score\""));
}

#[test]
fn test_risk_analysis() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Risk Test", "--mode", "sprint"])
        .assert().success();
    for (id, dur) in [("a", "4"), ("b", "6")] {
        camshaft().current_dir(dir.path())
            .args(["add", "task", id, "--name", id, "--duration", dur])
            .assert().success();
    }
    camshaft().current_dir(dir.path())
        .args(["add", "dep", "a", "b"])
        .assert().success();

    camshaft().current_dir(dir.path())
        .args(["risk-analysis", "--iterations", "500"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"duration_stats\""))
        .stdout(predicate::str::contains("\"criticality_index\""));
}

#[test]
fn test_evm() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "EVM Test", "--mode", "sprint"])
        .assert().success();
    camshaft().current_dir(dir.path())
        .args(["add", "task", "t1", "--name", "T1", "--duration", "5"])
        .assert().success();

    camshaft().current_dir(dir.path())
        .args(["evm"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"bac\""))
        .stdout(predicate::str::contains("\"spi\""));
}

#[test]
fn test_diff() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Diff Test", "--mode", "sprint"])
        .assert().success();
    camshaft().current_dir(dir.path())
        .args(["add", "task", "a", "--name", "A", "--duration", "3"])
        .assert().success();

    // Export baseline
    camshaft().current_dir(dir.path())
        .args(["export", "--file", "baseline.json"])
        .assert().success();

    // Modify plan
    camshaft().current_dir(dir.path())
        .args(["add", "task", "b", "--name", "B", "--duration", "5"])
        .assert().success();

    // Diff against baseline
    camshaft().current_dir(dir.path())
        .args(["diff", "--baseline", "baseline.json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"tasks_added\""));
}

#[test]
fn test_import_roundtrip() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Roundtrip", "--mode", "sprint"])
        .assert().success();
    camshaft().current_dir(dir.path())
        .args(["add", "task", "x", "--name", "X", "--duration", "3"])
        .assert().success();
    camshaft().current_dir(dir.path())
        .args(["export", "--file", "plan-copy.json"])
        .assert().success();

    // Re-import with force
    camshaft().current_dir(dir.path())
        .args(["import", "--file", "plan-copy.json", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"imported\""))
        .stdout(predicate::str::contains("\"task_count\": 1"));
}

#[test]
fn test_optimize_fast_track_dry_run() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "FT Test", "--mode", "sprint"])
        .assert().success();
    for (id, dur) in [("a", "4"), ("b", "6"), ("c", "3")] {
        camshaft().current_dir(dir.path())
            .args(["add", "task", id, "--name", id, "--duration", dur])
            .assert().success();
    }
    camshaft().current_dir(dir.path())
        .args(["add", "dep", "a", "b"])
        .assert().success();
    camshaft().current_dir(dir.path())
        .args(["add", "dep", "b", "c"])
        .assert().success();

    // Dry-run: should not mutate plan
    camshaft().current_dir(dir.path())
        .args(["optimize", "--fast-track"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"optimization_moves\""));
}

#[test]
fn test_level_resources_no_conflicts() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Level Test", "--mode", "sprint"])
        .assert().success();
    camshaft().current_dir(dir.path())
        .args(["add", "task", "t1", "--name", "T1", "--duration", "3"])
        .assert().success();

    // No resources — should return no_conflicts status
    camshaft().current_dir(dir.path())
        .args(["level-resources"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"no_conflicts\""));
}

#[test]
fn test_velocity_not_a_repo() {
    let dir = setup_test_dir();
    // Not a git repo — should error cleanly
    camshaft().current_dir(dir.path())
        .args(["velocity"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("validation_failed"));
}

#[test]
fn test_bottlenecks_sorted() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Bottleneck Sort", "--mode", "sprint"])
        .assert().success();
    // 3 critical tasks with different durations
    for (id, dur) in [("short", "2"), ("long", "10"), ("medium", "5")] {
        camshaft().current_dir(dir.path())
            .args(["add", "task", id, "--name", id, "--duration", dur])
            .assert().success();
    }
    camshaft().current_dir(dir.path())
        .args(["add", "dep", "short", "medium"])
        .assert().success();
    camshaft().current_dir(dir.path())
        .args(["add", "dep", "medium", "long"])
        .assert().success();

    camshaft().current_dir(dir.path())
        .args(["query", "bottlenecks"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ranking\""))
        .stdout(predicate::str::contains("\"fan_out\""));
}

#[test]
fn test_whatif_has_affected_tasks_and_cp_changed() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "WhatIf Fields", "--mode", "sprint"])
        .assert().success();
    for (id, dur) in [("a", "4"), ("b", "2")] {
        camshaft().current_dir(dir.path())
            .args(["add", "task", id, "--name", id, "--duration", dur])
            .assert().success();
    }
    camshaft().current_dir(dir.path())
        .args(["add", "dep", "a", "b"])
        .assert().success();

    camshaft().current_dir(dir.path())
        .args(["query", "what-if", "a", "--duration", "10"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"affected_tasks\""))
        .stdout(predicate::str::contains("\"critical_path_changed\""));
}

#[test]
fn test_ready_has_schedule_priority_and_on_critical_path() {
    let dir = setup_test_dir();
    camshaft().current_dir(dir.path())
        .args(["init", "--name", "Ready Schema", "--mode", "sprint"])
        .assert().success();
    camshaft().current_dir(dir.path())
        .args(["add", "task", "a", "--name", "A", "--duration", "3"])
        .assert().success();

    camshaft().current_dir(dir.path())
        .args(["query", "ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"schedule_priority\""))
        .stdout(predicate::str::contains("\"on_critical_path\""));
}

#[test]
fn test_bulk_yaml_creates_plan() {
    let dir = setup_test_dir();
    let yaml = r#"name: "Bulk Test"
mode: sprint
tasks:
  - id: a
    name: "Alpha"
    duration: 4
    priority: high
  - id: b
    name: "Bravo"
    duration: 2
  - id: c
    name: "Charlie"
    duration: 3
dependencies:
  - [a, b]
  - { from: a, to: c, type: ss, lag: 1 }
milestones:
  - id: m1
    name: "Milestone One"
resources:
  - id: r1
    name: "Agent"
    type: labor
    units: 8
assignments:
  - { task: b, resource: r1, units: 2 }
"#;
    std::fs::write(dir.path().join("plan.yaml"), yaml).unwrap();

    camshaft()
        .current_dir(dir.path())
        .args(["bulk", "--file", "plan.yaml"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"created\""))
        .stdout(predicate::str::contains("\"tasks_created\": 3"))
        .stdout(predicate::str::contains("\"dependencies_created\": 2"))
        .stdout(predicate::str::contains("\"milestones_created\": 1"))
        .stdout(predicate::str::contains("\"resources_created\": 1"))
        .stdout(predicate::str::contains("\"assignments_created\": 1"));

    assert!(dir.path().join(".camshaft/plan.json").exists());

    // Verify the plan is queryable.
    camshaft()
        .current_dir(dir.path())
        .args(["query", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"a\""))
        .stdout(predicate::str::contains("\"b\""))
        .stdout(predicate::str::contains("\"c\""));
}

#[test]
fn test_bulk_json_creates_plan() {
    let dir = setup_test_dir();
    let json = r#"{
        "name": "Bulk JSON",
        "mode": "sprint",
        "tasks": [
            {"id": "x", "name": "X", "duration": 2},
            {"id": "y", "name": "Y", "duration": 3}
        ],
        "dependencies": [["x", "y"]]
    }"#;
    std::fs::write(dir.path().join("plan.json"), json).unwrap();

    camshaft()
        .current_dir(dir.path())
        .args(["bulk", "--file", "plan.json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"tasks_created\": 2"))
        .stdout(predicate::str::contains("\"dependencies_created\": 1"));
}

#[test]
fn test_bulk_refuses_overwrite_without_force() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Existing", "--mode", "sprint"])
        .assert()
        .success();

    let yaml = "name: \"Bulk\"\nmode: sprint\ntasks: []\n";
    std::fs::write(dir.path().join("plan.yaml"), yaml).unwrap();

    camshaft()
        .current_dir(dir.path())
        .args(["bulk", "--file", "plan.yaml"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("plan_already_exists"));

    // With --force it succeeds
    camshaft()
        .current_dir(dir.path())
        .args(["bulk", "--file", "plan.yaml", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"created\""));
}

#[test]
fn test_bulk_atomic_on_invalid_dependency() {
    let dir = setup_test_dir();
    let yaml = r#"name: "Bad"
mode: sprint
tasks:
  - id: a
    name: "A"
    duration: 1
dependencies:
  - [a, nonexistent]
"#;
    std::fs::write(dir.path().join("bad.yaml"), yaml).unwrap();

    camshaft()
        .current_dir(dir.path())
        .args(["bulk", "--file", "bad.yaml"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("validation_failed"));

    // No plan file should have been written
    assert!(!dir.path().join(".camshaft/plan.json").exists());
}

#[test]
fn test_bulk_rejects_absolute_path() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["bulk", "--file", "/etc/passwd"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("validation_failed"));
}

/// Diamond graph: A(2) -> B(4), C(3); B -> D(1), C -> D(1).
/// Zero-float set is {A, B, D} (A->B->D = 7 > A->C->D = 6).
/// Verifies `critical_path` is a proper chain, not the topo-ordered
/// zero-float set (which could have been something like [A, B, D]
/// or [A, B, C, D] if GanttML ever widened it).
#[test]
fn test_critical_path_is_a_proper_chain_on_diamond() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["init", "--name", "Diamond", "--mode", "roadmap", "--start", "2026-01-05"])
        .assert()
        .success();

    for (id, dur) in [("a", "2"), ("b", "4"), ("c", "3"), ("d", "1")] {
        camshaft()
            .current_dir(dir.path())
            .args(["add", "task", id, "--name", id, "--duration", dur])
            .assert()
            .success();
    }
    for (from, to) in [("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")] {
        camshaft()
            .current_dir(dir.path())
            .args(["add", "dep", from, to])
            .assert()
            .success();
    }

    // optimize: critical_path should be [a, b, d]
    let out = camshaft()
        .current_dir(dir.path())
        .args(["optimize"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let chain: Vec<String> = v["critical_path"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.as_str().unwrap().to_string())
        .collect();
    assert_eq!(chain, vec!["a", "b", "d"], "critical_path should be [a,b,d], got {:?}", chain);
    assert_eq!(v["critical_path_is_chain"], serde_json::json!(true));
    assert!((v["critical_path_duration"].as_f64().unwrap() - 7.0).abs() < 1e-9);

    // query critical-path should return the same chain.
    let out2 = camshaft()
        .current_dir(dir.path())
        .args(["query", "critical-path"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text2 = String::from_utf8(out2).unwrap();
    let v2: serde_json::Value = serde_json::from_str(&text2).unwrap();
    let chain2: Vec<String> = v2["critical_path"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.as_str().unwrap().to_string())
        .collect();
    assert_eq!(chain2, vec!["a", "b", "d"]);
}

#[test]
fn test_template_list() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["template", "--list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"templates\""))
        .stdout(predicate::str::contains("feature-impl"))
        .stdout(predicate::str::contains("bug-fix"))
        .stdout(predicate::str::contains("migration"))
        .stdout(predicate::str::contains("launch"))
        .stdout(predicate::str::contains("research-spike"));

    // --list must not create a plan file
    assert!(!dir.path().join(".camshaft/plan.json").exists());
}

#[test]
fn test_template_feature_impl() {
    let dir = setup_test_dir();
    camshaft()
        .current_dir(dir.path())
        .args(["template", "feature-impl", "--name", "Auth Feature"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"created\""))
        .stdout(predicate::str::contains("\"template\": \"feature-impl\""))
        .stdout(predicate::str::contains("\"tasks_created\": 6"))
        .stdout(predicate::str::contains("\"dependencies_created\": 6"));

    assert!(dir.path().join(".camshaft/plan.json").exists());

    // Plan should validate cleanly
    camshaft()
        .current_dir(dir.path())
        .args(["validate"])
        .assert()
        .success();
}
