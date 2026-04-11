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
