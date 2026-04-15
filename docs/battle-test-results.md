# Battle Test Results

Executed: 2026-04-14
Binary: `target/debug/camshaft` (built at HEAD = 3d9f771)
Harness: shell-driven, isolated `/tmp/bt-scenario-N` project dirs, JSON output parsed with `python3`.

---

## Scenario 1: Diamond Dependency (Parallelism Detection)

- Setup: 4 tasks (T1..T4), classic diamond (T1 → {T2, T3} → T4), sprint mode.
- Expected: CP=[T1,T2,T4], makespan=8h, {T2,T3} parallel after T1, T3 slack=1h.
- Result: **PASS**
- Evidence:
  - `optimize.critical_path = ["T1","T2","T4"]` (matches exactly).
  - `optimize.project_duration = 8.0` (matches).
  - `parallel_groups` group 2 = `["T3","T2"]` at `earliest_start=2.0` (set equality holds).
  - `query status` → T3 `total_float=1.0`, T2 `total_float=0.0`, both critical flags correct.
  - `suggest-order`: step 1=T1, step 2={T3,T2} parallel, step 3=T4. Order valid.
- Notes: All 5 success criteria pass cleanly.

---

## Scenario 2: Realistic Web App (Multi-Path Parallelism)

- Setup: 15-task web app (S1..DEP1), 23 edges, sprint mode, infinite parallelism.
- Expected (per docs): CP=[S1,S2,BE1,BE2,BE4,QA2,DEP1], makespan=19h, BE2 top bottleneck.
- Result: **FAIL** (but test's "known-correct" is itself wrong — see note).
- Evidence:
  - `optimize.project_duration = 22.0` (not 19.0 as the spec claims).
  - The 22h figure is actually correct on this graph: FE3 depends on BE3 AND FE1, and FE3 (5h) chains into FE4 (2h) → QA2 (4h) → DEP1 (2h). Walking `S1→S2→BE1→BE3→FE3→FE4→QA2→DEP1` = 1+2+2+4+5+2+4+2 = **22h**. The spec's 19h computation omits the BE3→FE3 leg of the frontend chain.
  - So Camshaft's makespan is right; the test's expected value is wrong.
  - However, `critical_path` output is malformed: it returns `[S1,S2,BE1,BE3,FE3,BE2,FE2,FE4,QA2,DEP1]` — a list of ALL zero-float tasks in topo order, not a valid predecessor chain. `FE3 → BE2` is not an edge in the graph (neither direction); critical_path is supposed to be a single longest-path chain.
  - `parallel_groups`: {S2, FE1, CI1} group after S1 present (criterion 2 pass).
  - `suggest-order`: dep-respecting, S1 first, DEP1 last (criterion 4 pass).
  - `bottlenecks` output (criterion 3): FAIL — returns ALL 10 zero-float tasks as bottlenecks, in topo order. Does not rank BE2 (or FE3 for that matter) as the top bottleneck by duration × downstream-impact.
- Notes (VALUABLE FINDINGS):
  - **Bug 1 — critical_path is "zero-float set", not a chain.** Camshaft collects every task with total_float=0 and emits them in topological order. With parallel CPs tied at 22h (via BE2 and via BE3/FE3), this dumps both chains interleaved into one list. An agent reading this output cannot tell which tasks are actually sequential on the critical path. Confirmed by `query critical-path` returning the same malformed list.
  - **Bug 2 — `bottlenecks` doesn't rank.** Returns zero-float tasks in topo order, undifferentiated. Should sort by duration (or duration × fan-out, per spec). An agent asking "what should I prioritize?" gets no signal.

---

## Scenario 3: Sprint Capacity Overcommit

- Setup: 20 tasks, all assigned to `alice` (capacity 40h), total effort 67h.
- Expected: `overcommit-check` reports overage=27h; `sprint suggest` keeps 7 must-haves + should-haves fitting in 40h.
- Result: **FAIL**
- Evidence:
  - `overcommit-check` does NOT emit `overage_hours` or `overcommitted_resources`. Instead, it reports day-by-day warnings (each day has `excess_hours: 8.0`) based on an 8h/day default. No aggregate sprint overage surfaced. Task lists per day look suspiciously uniform (each day sums to 16h regardless of task durations).
  - `sprint suggest` ignores the 40h capacity entirely — emits a 9-day plan covering nearly all 20 tasks (~72h scheduled). No priority-aware descope. `unscheduled: []`.
  - `sprint plan --capacity 40` selects: `task_order = [P1,P2,P3,P4,P5,P6,P7,P8,P9,P10,P11,P15]` (12 tasks, 40h).
    - **Drops must-have tasks P19 and P20** (both tagged `must` in the spec).
    - Includes should-haves (P5, P6, P7, P8, P10) and a could-have (P9) over a must-have.
    - Priority tier not enforced in selection.
- Notes (VALUABLE FINDINGS):
  - **Bug 3 — overcommit-check reports no aggregate overage.** `overage_hours` and `overcommitted_resources` fields are absent. The per-day warning shape can't answer "by how much is the sprint overcommitted?" without client-side summation.
  - **Bug 4 — sprint plan drops must-haves.** Selection algorithm appears to take tasks in ID order (P1, P2, ..., P15) until capacity hits 40h, irrespective of priority tier. P19 and P20 (both `must`) are silently dropped. This is the exact failure mode the scenario calls out ("sprint suggest drops a must-have to fit a should-have").
  - **Bug 5 — sprint suggest ignores capacity.** The `--capacity` flag on `sprint plan` works, but `sprint suggest` has no capacity parameter and produces a schedule exceeding the sprint budget.

---

## Scenario 4: Cycle Detection

- Setup: 5-task payment refactor, Phase A clean, Phase B inject cycle `R4→R1`, Phase C remove.
- Expected: Phase A pass; Phase B validate fails naming R1,R2,R4; optimize doesn't hang; Phase C restores.
- Result: **PASS (partial — see notes)**
- Evidence:
  - Phase A: `validate` exit 0; `optimize` returns CP containing [R1, R2, R3, R4, R5], makespan=10h (both R2 and R3 branches tied — same bug 1 manifestation).
  - Phase B `validate`: exit 1, errors = `["Cycle detected involving tasks: R5, R1, R4, R2, R3"]`. Contains the required tokens `cycle`, `R1`, `R2`, `R4`.
  - Phase B `optimize`: returns error JSON `{"error":"gantt_ml_error","message":"... cycle detected involving activities: [R1, R2, R4, R1]"}` — notably, the underlying GanttML layer identifies the **precise cycle** `R1→R2→R4→R1`, which IS actionable.
  - Phase C: `remove dep` works, `validate` passes, `optimize` output identical to Phase A.
- Notes:
  - **Minor quirk — validator overreports cycle members.** `validate` lists all 5 tasks in the SCC / reachability set (R1,R2,R3,R4,R5) rather than the minimal cycle {R1,R2,R4}. R3 and R5 are not on the cycle. The deeper `optimize` error message is correct and minimal. Consider having `validate` surface the same precise cycle string.
  - Criterion 5 ("mentions which edge to remove"): `validate` does NOT name the offending edge; it just lists nodes. The `optimize` error does list the cycle chain R1→R2→R4→R1 which implicitly fingers the back-edge. Acceptable but not as actionable as "remove edge R4 → R1".
  - State recovery is clean.

---

## Scenario 5: What-If Impact Analysis

- Setup: Scenario 2 plan (15 tasks), apply what-if to BE2 and BE3.
- Expected: BE2 := 10 → makespan 24h, CP unchanged; BE3 := 10 → makespan 24h, CP shifts; non-destructive.
- Result: **PASS (partial — against Camshaft's own baseline)**
- Evidence:
  - Because baseline is 22h (not 19h — same Scenario 2 discrepancy), new makespans are 27h and 28h respectively, not 24h.
  - `what-if BE2 duration=10`: `impact=5.0`, new makespan 27h via CP `[S1,S2,BE1,BE2,FE2,FE4,QA2,DEP1]` (1+2+2+10+4+2+4+2 = 27h — correct). CP shifted off the BE3 chain.
  - `what-if BE3 duration=10`: `impact=6.0`, new makespan 28h via CP `[S1,S2,BE1,BE3,FE3,FE4,QA2,DEP1]` (1+2+2+10+5+2+4+2 = 28h — correct). Impact is 6h because BE3 went from 4→10 (delta 6h), whereas spec expected delta 5h (assumed BE3 baseline of 5h).
  - Non-destructive: confirmed. `query critical-path` after both what-ifs returns the same 22h baseline.
  - Output schema uses `before`/`after`/`impact`/`original_duration`/`new_duration` — clean and self-documenting.
- Notes:
  - **Missing fields — `affected_tasks` and `critical_path_changed`.** The spec (and agent consumers) would benefit from:
    - `affected_tasks`: set of tasks whose early/late times shifted. Currently an agent has to diff the before/after CP manually.
    - `critical_path_changed: bool` — obvious from diffing `before.critical_path` vs `after.critical_path`, but an explicit flag is cheaper for agents.
  - The math propagates correctly, CP shifts are detected, state is preserved. Core what-if engine is solid.

---

## Summary

- Passed: 2/5 (Scenario 1, Scenario 4)
- Failed: 2/5 (Scenario 2, Scenario 3)
- Partial / mixed: 1/5 (Scenario 5 — math correct, output shape missing documented fields)

### Key findings (bugs worth fixing)

1. **`critical_path` is not a chain.** Both `optimize` and `query critical-path` return the set of zero-float tasks in topological order. When multiple longest paths tie (or simply when a parallel critical segment exists), this emits an invalid "chain" mixing branches (e.g. `[...,BE3,FE3,BE2,FE2,...]` where no BE3→BE2 edge exists). Agents cannot follow this as a predecessor chain. Fix: compute one deterministic longest-path walk from source to sink. (Scenarios 2, 4, 5.)

2. **`bottlenecks` returns zero-float set unordered.** Not ranked by duration, downstream fan-out, or any meaningful metric. Agent asking "what's the top bottleneck?" gets no answer. (Scenario 2.)

3. **`sprint plan` ignores priority tiers.** Greedy fill by task ID order, drops must-have tasks (P19, P20) to fit should-haves and could-haves. This is a correctness blocker for the sprint-planning use case — an AI agent relying on this output would silently ship a plan missing required work. (Scenario 3.)

4. **`sprint overcommit-check` has no aggregate overage.** Per-day warnings only; no `overage_hours` or `overcommitted_resources` fields in the JSON. Also, the day-by-day scheduling underneath looks buggy (uniform 16h on multiple days regardless of task mix). (Scenario 3.)

5. **`sprint suggest` ignores sprint capacity entirely.** Emits a schedule of all tasks across however many days are needed, no descope. (Scenario 3.)

6. **`query what-if` output omits documented fields.** `affected_tasks` and `critical_path_changed` not emitted. The underlying computation is correct; only the response shape is thin. (Scenario 5.)

7. **Validator cycle report overreports SCC members.** Lists all reachable nodes, not just the minimal cycle. The deeper GanttML error is precise (`R1→R2→R4→R1`) — surface that instead. (Scenario 4.)

### What Camshaft does well

- Core CPM math (forward/backward pass, total float, project duration) is correct, including on the 15-task realistic graph.
- Parallel-group extraction is correct and useful (earliest_start, set membership).
- Cycle detection triggers and refuses to optimize cyclic graphs — no hangs, clean JSON error.
- What-if analysis is genuinely non-destructive; CP-shift detection works even though the boolean flag isn't surfaced.
- CLI ergonomics are good: every command emits JSON by default, exit codes are meaningful, state persists via `plan.json` in cwd.

### Test-spec issues noted

- Scenario 2's "known-correct" makespan of 19h is itself wrong given the dep set (the spec's own table implies 22h because FE3 depends on BE3). Camshaft's 22h is mathematically correct for the documented graph. The spec should either update the expected value to 22h or drop the BE3→FE3 dependency. This cascades into Scenario 5 expected values (24h → 27h/28h).
