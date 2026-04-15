use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use serde_json::json;

use crate::error::{CamshaftError, Result};

/// Estimate task durations from git history.
///
/// Analyzes recent commits in a repo to produce velocity metrics an AI agent
/// can use to calibrate duration estimates during planning.
pub fn run(repo_path: Option<&str>, days: u32) -> Result<()> {
    let repo = repo_path.unwrap_or(".");
    validate_repo_path(repo)?;

    // Must be a git repo.
    ensure_git_repo(repo)?;

    let since = format!("{} days ago", days);

    // --- Pass 1: commits (hash | ISO date | author | subject) ---
    let log_output = run_git(
        repo,
        &[
            "log",
            &format!("--since={}", since),
            "--format=%H|%ci|%an|%s",
        ],
    )?;

    let commit_lines: Vec<&str> = log_output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();

    let total_commits = commit_lines.len();

    // Empty-history guard.
    if total_commits == 0 {
        let output = json!({
            "repo_path": repo,
            "analysis_window_days": days,
            "total_commits": 0,
            "commits_per_day_avg": 0.0,
            "active_days": 0,
            "avg_files_per_commit": 0.0,
            "velocity_metrics": {
                "small_change_hours": 0.0,
                "medium_change_hours": 0.0,
                "large_change_hours": 0.0,
            },
            "recommendations": [
                format!("No commits found in the last {} days. Cannot derive velocity.", days),
                "Use default estimates until history accumulates.",
            ],
            "warning": "empty_history",
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        return Ok(());
    }

    let mut distinct_days: HashSet<String> = HashSet::new();
    for line in &commit_lines {
        // %ci format: "YYYY-MM-DD HH:MM:SS +ZZZZ" — take the date portion.
        if let Some(date_part) = line.split('|').nth(1) {
            if let Some(day) = date_part.split_whitespace().next() {
                distinct_days.insert(day.to_string());
            }
        }
    }
    let active_days = distinct_days.len();

    // --- Pass 2: numstat for files-per-commit + size bucketing ---
    let numstat_output = run_git(
        repo,
        &[
            "log",
            &format!("--since={}", since),
            "--numstat",
            "--format=__COMMIT__%H",
        ],
    )?;

    let (total_files, small, medium, large) = parse_numstat(&numstat_output);

    let avg_files_per_commit = if total_commits > 0 {
        total_files as f64 / total_commits as f64
    } else {
        0.0
    };

    let commits_per_day_avg = if active_days > 0 {
        total_commits as f64 / active_days as f64
    } else {
        0.0
    };

    // Heuristic hour estimates.
    // Baseline bucket sizes: small=1.5, medium=5, large=12 files.
    // Formula (per spec): size * multiplier, multiplier shrinks with bucket (diminishing returns).
    let small_change_hours = 1.5_f64 * 1.5;
    let medium_change_hours = 5.0_f64 * 1.0;
    let large_change_hours = 12.0_f64 * 0.8;

    let mut recommendations: Vec<String> = Vec::new();
    recommendations.push(format!(
        "For tasks estimated as 'small' (1-2 file change), use {:.1}h duration",
        small_change_hours
    ));
    recommendations.push(format!(
        "For tasks estimated as 'medium' (3-7 files), use {:.1}h duration",
        medium_change_hours
    ));
    recommendations.push(format!(
        "For tasks estimated as 'large' (8+ files), use {:.1}h duration",
        large_change_hours
    ));
    recommendations.push(format!(
        "Historical data suggests {:.1} commits/day on active days — plan capacity accordingly",
        commits_per_day_avg
    ));
    if avg_files_per_commit > 0.0 {
        recommendations.push(format!(
            "Average commit touches {:.1} files — use this as a 'typical task' reference",
            avg_files_per_commit
        ));
    }

    let output = json!({
        "repo_path": repo,
        "analysis_window_days": days,
        "total_commits": total_commits,
        "commits_per_day_avg": round2(commits_per_day_avg),
        "active_days": active_days,
        "avg_files_per_commit": round2(avg_files_per_commit),
        "size_distribution": {
            "small": small,
            "medium": medium,
            "large": large,
        },
        "velocity_metrics": {
            "small_change_hours": small_change_hours,
            "medium_change_hours": medium_change_hours,
            "large_change_hours": large_change_hours,
        },
        "recommendations": recommendations,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    Ok(())
}

/// Reject traversal and absolute paths (same pattern as export.rs).
fn validate_repo_path(repo_path: &str) -> Result<()> {
    let path = Path::new(repo_path);

    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return Err(CamshaftError::ValidationFailed(
                "Repo path must not contain '..' components.".to_string(),
            ));
        }
    }

    if path.is_absolute() {
        return Err(CamshaftError::ValidationFailed(
            "Repo path must be relative, not absolute.".to_string(),
        ));
    }

    Ok(())
}

fn ensure_git_repo(repo: &str) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map_err(|e| CamshaftError::Io(format!("Failed to invoke git: {}", e)))?;

    if !output.status.success() {
        return Err(CamshaftError::ValidationFailed(format!(
            "Not a git repository (or git not available): {}",
            repo
        )));
    }

    Ok(())
}

fn run_git(repo: &str, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| CamshaftError::Io(format!("Failed to invoke git: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(CamshaftError::Io(format!("git failed: {}", stderr.trim())));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Parse `git log --numstat --format=__COMMIT__%H` output.
///
/// Sections separated by `__COMMIT__<hash>` lines. Each numstat line is
/// `added<TAB>removed<TAB>path`. Binary files show `-` in added/removed —
/// per spec we skip those in the file count.
///
/// Returns (total_files_changed, small_count, medium_count, large_count).
fn parse_numstat(output: &str) -> (usize, usize, usize, usize) {
    let mut total_files: usize = 0;
    let mut small = 0usize;
    let mut medium = 0usize;
    let mut large = 0usize;

    let mut current_commit_files: usize = 0;
    let mut in_commit = false;

    let flush = |files: usize, small: &mut usize, medium: &mut usize, large: &mut usize| {
        if files == 0 {
            return;
        }
        if files <= 2 {
            *small += 1;
        } else if files <= 7 {
            *medium += 1;
        } else {
            *large += 1;
        }
    };

    for line in output.lines() {
        if let Some(_rest) = line.strip_prefix("__COMMIT__") {
            if in_commit {
                flush(current_commit_files, &mut small, &mut medium, &mut large);
            }
            current_commit_files = 0;
            in_commit = true;
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // numstat: added<TAB>removed<TAB>path
        let mut parts = trimmed.split('\t');
        let added = match parts.next() {
            Some(s) => s,
            None => continue,
        };
        let _removed = match parts.next() {
            Some(s) => s,
            None => continue,
        };
        let path = match parts.next() {
            Some(s) => s,
            None => continue,
        };

        // Binary files: git prints "-\t-\tpath". Skip per spec.
        if added == "-" {
            continue;
        }
        if path.is_empty() {
            continue;
        }

        current_commit_files += 1;
        total_files += 1;
    }

    if in_commit {
        flush(current_commit_files, &mut small, &mut medium, &mut large);
    }

    (total_files, small, medium, large)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
