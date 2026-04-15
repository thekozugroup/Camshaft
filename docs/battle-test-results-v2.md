# Battle Test Results — Phase 3 Binary

Executed: 2026-04-15
Binary: `target/debug/camshaft` (rebuilt cleanly, one dead-code warning)
Isolation: `/tmp/bt-phase4-{1..5,level,ft,velocity}`

## Scenario 1: Diamond Dependency (Parallelism Detection)
- Result: **PASS**
- Change from v1: unchanged (was already PASS)
- Evidence:
  - `optimize.project_duration = 8.0`, `critical_path = ["T1","T2","T4"]`.
  - `parallel_groups` group 2 = `["T3","T2"]` at `earliest_start=2.0`.
  - `query status`: T3 `total_float=1.0`, T2/T1/T4 critical.
  - `bottlenecks` now ranked by duration desc + fan_out desc; ranking metadata string present.
  - `query ready` uses `schedule_priority` and `on_critical_path`.

## Scenario 2: Realistic Web App (Multi-Path Parallelism)
- Result: **PARTIAL (critical-path chain still malformed)**
- Change from v1: PARTIALLY FIXED (bottlenecks ranking fixed; critical-path chain still broken)
- Evidence:
  - `optimize.project_duration = 22.0` (correct — the test spec's 19h expectation is wrong, as noted in v1).
  - `critical_path = [S1,S2,BE1,BE3,FE3,BE2,FE2,FE4,QA2,DEP1]` — STILL not a valid predecessor chain. Includes both parallel CP branches interleaved in topo order. `BE3 → FE3 → BE2` is not an edge (FE3 does not precede BE2). Bug 1 from v1 persists.
  - `execution_hint` literally prints the malformed chain: `"S1 → S2 → BE1 → BE3 → FE3 → BE2 → ..."` — actively misleading to agents.
  - **FIXED**: `bottlenecks` now returns objects `{id, name, duration, fan_out}` sorted by `duration desc, fan_out desc`, plus `"ranking"` metadata explaining order. BE2 is correctly ranked #1 (duration 5, fan_out 2).

## Scenario 3: Sprint Capacity Overcommit
- Result: **FAIL** (all three v1 bugs still present)
- Change from v1: STILL BROKEN
- Evidence:
  - `sprint overcommit-check`: still per-day warnings only (`date`, `excess_hours`, `scheduled_hours`, `available_hours`, `task_ids`). NO aggregate `overage_hours`. NO `overcommitted_resources`. Uses default 8h/day (ignores alice's 40h resource capacity entirely). Bug 4 persists.
  - `sprint plan --capacity 40`: returns `task_order = [P1..P11]` (40h). **STILL DROPS must-have tasks P19 and P20** to fit should-haves (P8..P10 are `should`). `prioritized_order` field now lists all 20 in ID order with no tier-based sort. Bug 3 persists.
  - `sprint suggest`: still emits a 9-day schedule (`2026-04-15` .. `2026-04-27`) covering all 20 tasks with zero descope. Ignores 40h capacity. Bug 5 persists.

## Scenario 4: Cycle Detection
- Result: **PASS** (with same minor quirk as v1)
- Change from v1: unchanged (was PASS partial)
- Evidence:
  - Phase A `validate` exit 0; `optimize` returns CP [R1,R2,R4,R5] makespan 9h.
  - Phase B `validate` exit 1, `errors: ["Cycle detected involving tasks: R1, R3, R5, R4, R2"]` — still overreports the full SCC reachable set (includes R3, R5 which are NOT on the cycle).
  - Phase B `optimize` returns precise error: `{"error":"gantt_ml_error","message":"... Dependency cycle detected involving activities: [\"R1\", \"R2\", \"R4\", \"R1\"]"}` — minimal chain, actionable.
  - Phase C `remove dep` → `validate` exit 0 → `optimize` output matches Phase A. Clean recovery.
  - Criterion 5 (actionable edge mention) still only surfaced by `optimize`, not `validate`.

## Scenario 5: What-If Impact Analysis
- Result: **PASS**
- Change from v1: **FIXED** (previously PARTIAL — missing fields)
- Evidence:
  - `what-if BE2 --duration 10` now emits ALL documented fields: `affected_tasks: ["BE4","CI2","DEP1","FE2","FE4","QA2"]`, `critical_path_changed: true`, `before`/`after` objects with full CP + project_duration, `impact: 5.0` (delta, not full duration), `original_duration`, `new_duration`, `task`.
  - `what-if BE3 --duration 10`: `impact: 6.0` (delta), `critical_path_changed: true`.
  - No-op test (`--duration 5` on BE2 which already = 5): `impact: 0.0`, `critical_path_changed: false`, `affected_tasks: []`. Correct edge-case handling.
  - No state mutation: post-what-if `critical-path` returns the same baseline.
  - Affected-tasks set matches expected downstream closure for BE2 (FE2, BE4, FE4, QA2, CI2, DEP1; FE3 & CI1 correctly excluded).

## New Command Tests

### level-resources: **FAIL**
- `conflicts_before: 5` correctly detected for 2 tasks X,Y (4h each) both assigned to `bob` (units=1).
- But `conflicts_resolved: 0`, `moves: []`, `activities_delayed: 0`, `duration_increase: 0.0`. Tool reports `status: "leveled"` and `applied: true` with `--apply`, but the conflict remains (`remaining_conflicts: 5`).
- Reports `peak_utilization_before = peak_utilization_after = {bob: 2.0}` — no actual leveling happened.
- Verdict: detection works; resolution does not.

### velocity: **PASS (with caveat)**
- `velocity --days 30` (run from `~/Developer/Camshaft`) returns full metrics: `total_commits: 4`, `active_days: 2`, `avg_files_per_commit: 12.25`, velocity_metrics with small/medium/large hour estimates, recommendations array.
- Caveat: `--repo <abs-path>` fails with `"Repo path must be relative, not absolute"`. The user-requested invocation `velocity --repo ~/Developer/Camshaft --days 30` returns a validation error. Workaround: `cd` to the repo first.

### optimize --fast-track: **PASS**
- Now actually works (was only warning in v1).
- On 3-task chain A→B→C (4h each, FS): `fast_track_moves` contains 2 conversions (FS→SS with 0.25 lag). `optimized_duration: 10.0` vs `original_duration: 12.0`, `improvement_pct: 16.67%`.
- `--apply` persists changes: post-apply `query critical-path` shows `project_duration: 6.0` (further reduced).

### optimize --crash: **PASS**
- `crash_moves` populated with `{task, original_duration, new_duration, duration_saved, cost_delta}` entries. On scenario 2 plan, crashes BE2, FE3, BE3 etc. with sensible cost estimates.

## Summary

- Total passed: **4/7** (S1, S4, S5, velocity; fast-track/crash technically pass as new-command-works checks)
  - Counting: S1 PASS, S2 PARTIAL, S3 FAIL, S4 PASS, S5 PASS, level-resources FAIL, velocity PASS, fast-track PASS, crash PASS → 5 full passes + 1 partial + 2 fails out of 9 checks.
  - Against the requested 7 (5 scenarios + 2 new commands = level-resources, velocity): **4/7 full pass** (S1, S4, S5, velocity), 1 partial (S2), 2 fail (S3, level-resources).

### Key improvements since v1
1. **`bottlenecks` now ranked** with `fan_out` and duration-desc + fan_out-desc sort, plus `ranking` metadata field. Addresses v1 bug 2.
2. **`query what-if` complete** — `affected_tasks`, `critical_path_changed`, `before/after` CP, `impact` as delta, no-op handling. Addresses v1 bug 6.
3. **`query ready`** uses `schedule_priority` + `on_critical_path` fields (no more `priority_hint`).
4. **`optimize --fast-track` and `--crash`** actually execute moves (were warn-only in v1); `--apply` persists.
5. **`velocity` command** new in phase 3 and works on real git repos.

### Remaining issues (bugs from v1 NOT fixed)
1. **`critical_path` still not a predecessor chain** (v1 bug 1). `optimize` and `query critical-path` still return the zero-float set in topological order, which is invalid whenever parallel CP branches tie. `execution_hint` compounds the problem by formatting the bad list with arrow separators. (Scenarios 2, 4.)
2. **`sprint plan` still drops must-haves** (v1 bug 3). Takes tasks in ID order, ignores priority tier. P19 and P20 (must) dropped in favor of P8..P11 (should/could). Correctness blocker.
3. **`sprint overcommit-check` still has no aggregate overage** (v1 bug 4). No `overage_hours`, no `overcommitted_resources`. Per-day 8h warnings ignore the resource's declared 40h sprint capacity.
4. **`sprint suggest` still ignores capacity** (v1 bug 5). Emits multi-day schedule of all tasks; no descope.
5. **Validator cycle report still overreports SCC** (v1 bug 7). Minor.
6. **NEW: `level-resources` does not actually level**. Detects conflicts, claims `status: "leveled"`, but `conflicts_resolved: 0` and `moves: []`. Effect-free.
7. **NEW: `velocity --repo` rejects absolute paths**. User-facing ergonomics bug — the suggested invocation from the prompt literally does not work.

### Verdict
Phase 3 shipped the response-shape fixes (bottlenecks ranking, what-if fields, ready priority) and the new optimization levers (fast-track/crash apply, velocity, level-resources skeleton). The core algorithmic bugs from v1 (critical-path chain, sprint-plan priority tiers, sprint-capacity awareness) are untouched. `level-resources` is a stub — detection only, no resolution.
