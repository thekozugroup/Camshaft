//! Build a proper predecessor-successor critical chain from a CPM result.
//!
//! GanttML's `CpmResult::critical_path` returns zero-float tasks in topological
//! order, not a single valid dependency chain. For diamond-shaped graphs this
//! can include both "sides" of the diamond when only one is actually on the
//! critical chain. This module walks the dependency graph to build a proper
//! chain whose adjacent tasks have a direct dependency.

use std::collections::{HashMap, HashSet};

use gantt_ml::cpm::CpmResult;
use gantt_ml::Project;

/// Build a proper critical chain from the CPM result.
///
/// Returns `(primary_chain, alternate_chains)` where:
/// - `primary_chain` is the longest valid chain of zero-float tasks,
///   with each adjacent pair connected by a direct dependency.
/// - `alternate_chains` are other zero-float chains tied on duration.
///
/// Tie-break order:
/// 1. Chain with the most tasks.
/// 2. Chain whose total duration matches the project duration most closely
///    (already implied by zero-float membership).
/// 3. Lexicographic task-id order (deterministic).
pub fn build_critical_chain(
    project: &Project,
    cpm: &CpmResult,
) -> (Vec<String>, Vec<Vec<String>>) {
    // Zero-float set.
    let zf: HashSet<String> = cpm
        .total_float
        .iter()
        .filter(|(_, &f)| f == 0.0)
        .map(|(id, _)| id.clone())
        .collect();

    if zf.is_empty() {
        return (Vec::new(), Vec::new());
    }

    // Edges restricted to zero-float tasks, keyed by predecessor -> successors.
    let mut succs: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut has_zf_pred: HashSet<&str> = HashSet::new();
    for dep in &project.dependencies {
        let p = dep.predecessor_id.as_str();
        let s = dep.successor_id.as_str();
        if zf.contains(p) && zf.contains(s) {
            succs.entry(p).or_default().push(s);
            has_zf_pred.insert(s);
        }
    }

    // Chain starts = zero-float tasks with no zero-float predecessor.
    let mut starts: Vec<&str> = zf
        .iter()
        .map(|s| s.as_str())
        .filter(|id| !has_zf_pred.contains(id))
        .collect();
    starts.sort();

    // Enumerate all maximal chains via DFS.
    let mut all_chains: Vec<Vec<String>> = Vec::new();
    for start in &starts {
        let mut path: Vec<&str> = vec![*start];
        enumerate_chains(&zf, &succs, cpm, &mut path, &mut all_chains);
    }

    if all_chains.is_empty() {
        // Degenerate: a single zero-float task with no connecting deps.
        let single = starts
            .first()
            .map(|s| vec![s.to_string()])
            .unwrap_or_default();
        return (single, Vec::new());
    }

    // Score each chain by (task count, sum of durations).
    let mut scored: Vec<(usize, f64, Vec<String>)> = all_chains
        .into_iter()
        .map(|chain| {
            let dur: f64 = chain
                .iter()
                .filter_map(|id| project.activities.get(id))
                .map(|a| a.original_duration.days)
                .sum();
            (chain.len(), dur, chain)
        })
        .collect();

    // Sort: longest duration first, then most tasks, then lexicographic.
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.0.cmp(&a.0))
            .then_with(|| a.2.cmp(&b.2))
    });

    let primary = scored[0].2.clone();
    let primary_dur = scored[0].1;
    let primary_len = scored[0].0;

    let alternates: Vec<Vec<String>> = scored
        .iter()
        .skip(1)
        .filter(|(len, dur, chain)| {
            (*dur - primary_dur).abs() < 1e-9 && *len == primary_len && *chain != primary
        })
        .map(|(_, _, c)| c.clone())
        .collect();

    (primary, alternates)
}

/// DFS over zero-float successors, emitting every maximal chain.
///
/// At each node we prefer successors whose `early_start` equals this node's
/// `early_finish` (tight continuation) — but we still enumerate siblings so
/// the caller can pick the longest.
fn enumerate_chains<'a>(
    zf: &HashSet<String>,
    succs: &HashMap<&'a str, Vec<&'a str>>,
    cpm: &CpmResult,
    path: &mut Vec<&'a str>,
    out: &mut Vec<Vec<String>>,
) {
    let current = *path.last().expect("path non-empty");
    let cur_ef = cpm.early_dates.get(current).map(|&(_, ef)| ef);

    let raw = succs.get(current);
    let mut next: Vec<&'a str> = raw
        .map(|v| v.iter().copied().filter(|s| zf.contains(*s)).collect())
        .unwrap_or_default();

    // Prefer tight-continuation successors (early_start == cur_ef) first.
    next.sort_by(|a, b| {
        let tight_a = cpm
            .early_dates
            .get(*a)
            .map(|&(es, _)| Some(es) == cur_ef)
            .unwrap_or(false);
        let tight_b = cpm
            .early_dates
            .get(*b)
            .map(|&(es, _)| Some(es) == cur_ef)
            .unwrap_or(false);
        tight_b.cmp(&tight_a).then_with(|| a.cmp(b))
    });

    if next.is_empty() {
        out.push(path.iter().map(|s| s.to_string()).collect());
        return;
    }

    for s in next {
        // Avoid cycles (shouldn't exist in a valid schedule, but be safe).
        if path.contains(&s) {
            continue;
        }
        path.push(s);
        enumerate_chains(zf, succs, cpm, path, out);
        path.pop();
    }
}

/// Sum the durations of the tasks on a chain — used as a sanity check in
/// outputs (`critical_path_duration`).
pub fn chain_duration(project: &Project, chain: &[String]) -> f64 {
    chain
        .iter()
        .filter_map(|id| project.activities.get(id))
        .map(|a| a.original_duration.days)
        .sum()
}

/// Whether every adjacent pair in `chain` has a direct dependency in the
/// project. A chain of length 0 or 1 is trivially a chain.
pub fn is_valid_chain(project: &Project, chain: &[String]) -> bool {
    if chain.len() < 2 {
        return true;
    }
    let dep_set: HashSet<(&str, &str)> = project
        .dependencies
        .iter()
        .map(|d| (d.predecessor_id.as_str(), d.successor_id.as_str()))
        .collect();
    chain
        .windows(2)
        .all(|w| dep_set.contains(&(w[0].as_str(), w[1].as_str())))
}
