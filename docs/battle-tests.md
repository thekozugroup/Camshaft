# Camshaft Battle Tests

Battle tests validate that Camshaft produces correct, useful planning output for AI coding agents. Each scenario has a known-correct answer, so regressions and subtle correctness bugs (wrong critical path, missed parallelism, bad suggestions) can be detected automatically.

Tests are framed around the **agent workflow**: the agent receives a project description, invokes Camshaft, and must make the right decision based on Camshaft's output. A test passes when (a) Camshaft's computed answers match the known-correct answer and (b) the agent workflow produces the expected plan.

Durations are in hours unless otherwise noted. Task IDs follow the convention `T1, T2, ...` or descriptive slugs.

---

## Scenario 1: Diamond Dependency (Parallelism Detection)

**One-line purpose:** Verify Camshaft detects the canonical "diamond" fan-out/fan-in and emits two independent tasks as a single parallel group.

### Setup

Project: "Add password reset to auth service."

Tasks (4 total):

| ID | Name | Duration | Depends on |
|----|------|----------|------------|
| T1 | Design reset token schema | 2h | — |
| T2 | Implement backend endpoint | 4h | T1 |
| T3 | Build frontend reset form | 3h | T1 |
| T4 | End-to-end test + docs | 2h | T2, T3 |

No resources, no milestones. Sprint mode.

### Expected Agent Workflow

```bash
camshaft init --mode sprint
camshaft add task T1 "Design reset token schema" --duration 2
camshaft add task T2 "Implement backend endpoint" --duration 4
camshaft add task T3 "Build frontend reset form" --duration 3
camshaft add task T4 "E2E test + docs" --duration 2
camshaft add dep T2 --depends-on T1
camshaft add dep T3 --depends-on T1
camshaft add dep T4 --depends-on T2
camshaft add dep T4 --depends-on T3
camshaft validate
camshaft optimize
camshaft query parallel
camshaft query critical-path
```

### Known-Correct Answer

- **Critical path:** `[T1, T2, T4]` (total duration = 2 + 4 + 2 = **8h**)
- **Parallel groups:** After T1 completes, `{T2, T3}` can run in parallel. T3 has 1h of slack.
- **Project makespan:** 8h (not 11h — if agent runs serially it wastes 3h)
- **T3 total float / slack:** 1h
- **T2 total float / slack:** 0h (on critical path)

### Success Criteria

1. `camshaft validate` exits 0 with no errors.
2. `camshaft optimize` JSON output:
   - `critical_path` equals `["T1", "T2", "T4"]` exactly, in order.
   - `makespan_hours` equals `8`.
   - `parallel_groups` contains a group with both `T2` and `T3` (order-independent), and that group executes after T1 finishes.
3. `camshaft query parallel` lists `{T2, T3}` as concurrently-runnable after T1.
4. `camshaft query suggest-order` places T1 first, then T2 and T3 adjacent (in either order), then T4 last.
5. Slack for T3 reported as `1h`; slack for T2 reported as `0h`.

### Failure Modes to Watch

- **Serialization bug:** critical path reported as `[T1, T2, T3, T4]` (all 4 tasks) — means the optimizer is walking the topological order instead of the longest path.
- **Wrong critical task:** CP picks T3 branch (duration 3) instead of T2 branch (duration 4).
- **Parallel group missed:** `parallel_groups` empty or contains T2 and T3 in separate groups — means the scheduler didn't recognise they're independent.
- **Agent behaviour:** agent sees parallel group but still suggests executing T3 only after T2 finishes. Expected behaviour: agent should tell the user "T2 and T3 can be done concurrently" or, if dispatching subagents, spawn them in parallel.
- **Float/slack inversion:** slack assigned to T2 instead of T3.

---

## Scenario 2: Realistic Web App (Multi-Path Parallelism)

**One-line purpose:** Validate Camshaft on a realistic 15-task web app build where multiple independent critical-path candidates exist, and confirm the agent picks the right one.

### Setup

Project: "Build a CRUD todo web app with auth, CI, and deploy."

Tasks (15 total; all durations in hours):

| ID | Name | Dur | Depends on |
|----|------|-----|------------|
| S1 | Scaffold repo + tooling | 1 | — |
| S2 | Choose + configure DB | 2 | S1 |
| BE1 | Define DB schema (users, todos) | 2 | S2 |
| BE2 | Implement auth endpoints | 5 | BE1 |
| BE3 | Implement todo CRUD endpoints | 4 | BE1 |
| BE4 | Backend integration tests | 3 | BE2, BE3 |
| FE1 | Design UI mockups | 3 | S1 |
| FE2 | Build auth screens | 4 | FE1, BE2 |
| FE3 | Build todo UI (list + form) | 5 | FE1, BE3 |
| FE4 | Frontend component tests | 2 | FE2, FE3 |
| QA1 | Write E2E test spec | 2 | FE1 |
| QA2 | Implement E2E tests | 4 | QA1, BE4, FE4 |
| CI1 | Configure CI pipeline | 2 | S1 |
| CI2 | Wire CI to run BE + FE tests | 1 | CI1, BE4, FE4 |
| DEP1 | Deploy to staging | 2 | QA2, CI2 |

No resources assigned (infinite-parallelism mode). Sprint mode.

### Expected Agent Workflow

```bash
camshaft init --mode sprint
# Add all tasks and deps (15 add task + ~18 add dep calls)
camshaft validate
camshaft optimize
camshaft query critical-path
camshaft query parallel
camshaft query bottlenecks
camshaft query suggest-order
```

### Known-Correct Answer

Compute longest path from source to sink.

Longest path candidates ending at DEP1 (all must route through QA2 or CI2):

- S1 → FE1 → FE3 → FE4 → QA2 → DEP1 = 1+3+5+2+4+2 = **17h**
- S1 → FE1 → FE2 → FE4 → QA2 → DEP1 = 1+3+4+2+4+2 = 16h
- S1 → S2 → BE1 → BE2 → BE4 → QA2 → DEP1 = 1+2+2+5+3+4+2 = 19h
- S1 → S2 → BE1 → BE3 → BE4 → QA2 → DEP1 = 1+2+2+4+3+4+2 = 18h
- S1 → S2 → BE1 → BE2 → BE4 → CI2 → DEP1 = 1+2+2+5+3+1+2 = 16h

- **Critical path:** `[S1, S2, BE1, BE2, BE4, QA2, DEP1]` = **19h**
- **Bottleneck task:** BE2 (5h, on critical path, blocks FE2 and BE4).
- **Independent parallel front after S1 completes:** `{S2, FE1, CI1}` (three independent subtrees).
- **After BE1 completes:** `{BE2, BE3}` are concurrent.
- **After FE1 completes:** `{FE2 (waits on BE2), FE3 (waits on BE3), QA1}` — QA1 has no other prereq and is fully parallel.

### Success Criteria

1. `camshaft optimize` returns `makespan_hours == 19` and `critical_path == ["S1","S2","BE1","BE2","BE4","QA2","DEP1"]`.
2. `camshaft query parallel` reports the `{S2, FE1, CI1}` group after S1.
3. `camshaft query bottlenecks` ranks BE2 as the top bottleneck (highest duration × downstream-fan-out on critical path).
4. `camshaft query suggest-order` produces an order where:
   - S1 is first.
   - S2, FE1, CI1 appear before any of their respective downstream tasks.
   - DEP1 is last.
   - No task appears before all its dependencies.
5. Agent, given the output, should communicate: "critical path is backend-auth chain; start BE2 as early as possible; FE1 and CI1 can run in parallel."

### Failure Modes to Watch

- **Wrong critical path:** picks a frontend chain (17h) because it counts edges instead of summing durations.
- **suggest-order violates deps:** e.g., FE2 placed before BE2.
- **Parallel group omits CI1:** agent misses that CI setup is orthogonal to backend work.
- **Bottleneck wrong:** flags BE3 (4h) or FE3 (5h, not on CP) instead of BE2 — means the bottleneck metric ignores whether task is on CP.
- **Agent behaviour:** agent serializes BE + FE work; fails to note that FE1 can start immediately after S1.

---

## Scenario 3: Sprint Capacity Overcommit

**One-line purpose:** Verify `sprint overcommit-check` correctly flags overcommitted resources and that `sprint suggest` proposes a feasible descope.

### Setup

Project: "2-week sprint, one senior engineer (40h capacity)."

Tasks (20 total, all assigned to resource `alice`; durations = effort):

| ID | Name | Effort | Depends on | Priority |
|----|------|--------|------------|----------|
| P1 | Login page | 4 | — | must |
| P2 | Login API | 6 | — | must |
| P3 | Session middleware | 3 | P2 | must |
| P4 | Logout flow | 2 | P1, P3 | must |
| P5 | Password reset email | 4 | P2 | should |
| P6 | Password reset form | 3 | P1, P5 | should |
| P7 | 2FA setup | 5 | P3 | should |
| P8 | 2FA verify | 4 | P7 | should |
| P9 | Remember-me cookie | 2 | P3 | could |
| P10 | Account lockout | 3 | P3 | should |
| P11 | Audit log schema | 2 | — | must |
| P12 | Audit log writer | 3 | P11, P3 | should |
| P13 | Admin user list | 4 | P12 | could |
| P14 | Admin deactivate user | 3 | P13 | could |
| P15 | Rate limit login | 2 | P2 | should |
| P16 | CAPTCHA on failed login | 4 | P15 | could |
| P17 | Email templating | 3 | — | should |
| P18 | Onboarding email | 2 | P17, P2 | could |
| P19 | Docs update | 2 | P4 | must |
| P20 | E2E test suite | 6 | P4, P7, P12 | must |

Resource: `alice`, capacity `40h` for the sprint.

Total effort = 4+6+3+2+4+3+5+4+2+3+2+3+4+3+2+4+3+2+2+6 = **67h**. Sprint capacity = 40h → **overcommitted by 27h.**

Must-have subset: P1+P2+P3+P4+P11+P19+P20 = 4+6+3+2+2+2+6 = **25h**. Feasible.
Must + Should: add P5,P6,P7,P8,P10,P12,P15,P17 = 25 + (4+3+5+4+3+3+2+3) = 25 + 27 = **52h**. Still overcommitted.

### Expected Agent Workflow

```bash
camshaft init --mode sprint
# add 20 tasks with --priority flags
camshaft add resource alice --capacity 40
# assign all tasks to alice
camshaft validate
camshaft sprint overcommit-check
camshaft sprint suggest --capacity 40
camshaft sprint plan --capacity 40
```

### Known-Correct Answer

- `overcommit-check`: reports overage of **27h** (67 - 40), lists alice as the sole overcommitted resource.
- `sprint suggest` should:
  - Include all 7 must-have tasks (25h).
  - Add highest-value should-have tasks up to 40h (15h of headroom).
  - Valid 15h should-have selections (respecting deps):
    - P5 (4) + P17 (3) + P15 (2) + P10 (3) + P12 (3) = 15h. Must also include P11 if P12 selected — but P11 is already must-have.
    - Order may vary; any subset of should-haves totaling ≤15h with all deps satisfied is acceptable.
  - **Must NOT select** a should-have whose dep chain pulls in another unscheduled task and overflows capacity (e.g., P8 requires P7; selecting P8 without P7 is invalid).
- `sprint plan`: outputs per-task schedule in topological order fitting in 40h.

### Success Criteria

1. `sprint overcommit-check` returns `overage_hours == 27` and `overcommitted_resources == ["alice"]`.
2. `sprint suggest` output:
   - Total effort of suggested tasks ≤ 40h.
   - All `priority == must` tasks (7 of them) present.
   - No selected task has an unsatisfied dependency in the selected set.
   - Suggestion strictly dominates a naive greedy (i.e., agent can't trivially find a higher-must-count plan).
3. `sprint plan` emits a day-by-day or slot-by-slot schedule where alice is never assigned >1 task at a time and cumulative hours ≤ 40.
4. Agent response to user: "Sprint is overcommitted by 27h. I suggest keeping all must-haves plus [list]. Here's what we should cut."

### Failure Modes to Watch

- **overcommit-check reports wrong overage** (off-by-one on capacity, or sums only assigned tasks, etc.).
- **sprint suggest drops a must-have** to fit a should-have.
- **Dep violation in suggestion:** picks P8 without P7, or P6 without P5.
- **suggest ignores priority tier** and picks randomly to fit 40h.
- **Agent behaviour:** agent doesn't run overcommit-check, plans all 20 tasks, hallucinates that alice can do 67h in a 2-week sprint.
- **Infinite task-count tie-break:** multiple equally-valid 15h-of-shoulds plans; tool should pick one deterministically, not error.

---

## Scenario 4: Cycle Detection

**One-line purpose:** Verify `validate` catches a cycle introduced by an agent mid-session and gives an actionable error.

### Setup

Project: "Refactor payment service" — agent initially creates a clean plan, then mistakenly adds a back-edge.

Phase A (clean, should pass):

| ID | Name | Duration | Depends on |
|----|------|----------|------------|
| R1 | Extract payment interface | 2 | — |
| R2 | Implement Stripe adapter | 4 | R1 |
| R3 | Implement PayPal adapter | 4 | R1 |
| R4 | Migrate callers | 3 | R2, R3 |
| R5 | Delete old implementation | 1 | R4 |

Phase B: agent adds an erroneous dep `R1 --depends-on R4` (circular: R1 → R2 → R4 → R1).

Phase C: agent fixes by removing the bad edge; validate should pass again.

### Expected Agent Workflow

```bash
# Phase A
camshaft init --mode sprint
# add R1..R5 and correct deps
camshaft validate         # expect PASS
camshaft optimize         # expect success

# Phase B — introduce cycle
camshaft add dep R1 --depends-on R4
camshaft validate         # expect FAIL with cycle report
camshaft optimize         # expect FAIL (refuse to optimize)

# Phase C — remove bad edge
camshaft remove dep R1 --depends-on R4
camshaft validate         # expect PASS again
```

### Known-Correct Answer

- Phase A: validate passes. CP = `[R1, R2, R4, R5]` (or R3 branch, both = 10h since R2 and R3 same duration — tool should pick one deterministically). Makespan = 10h.
- Phase B: validate fails with a cycle error. Error message must:
  - Name the cycle members (at minimum: `R1, R2, R4` or a rotation).
  - Identify the offending edge as `R1 -> R4` (or the added dependency).
- Phase B optimize: must fail/refuse, not hang, not return a garbage CP.
- Phase C: validate passes; plan identical to Phase A.

### Success Criteria

1. Phase A `validate` exit code 0.
2. Phase B `validate` exit code ≠ 0 AND stderr/output includes the tokens `cycle` and all three of `R1`, `R2`, `R4` (R3 is not in the cycle).
3. Phase B `optimize` does not segfault, does not hang >5s, returns a non-zero exit or an error JSON (`{"error": "..."}`).
4. Phase C `validate` exit code 0; `optimize` output identical to Phase A.
5. The cycle error is **actionable**: mentions which edge to remove, not just "graph has a cycle."

### Failure Modes to Watch

- **Silent success:** validate returns 0 despite cycle (DFS not run or bug).
- **Optimize hangs:** CPM algorithm loops forever on cyclic graph.
- **Wrong members reported:** error lists R3 or R5 (not in cycle).
- **Unactionable error:** `Error: cycle detected` with no node names.
- **State corruption:** after removing the edge in Phase C, validate still fails (stale cache).
- **Agent behaviour:** agent doesn't run validate after each add dep, plows ahead with a broken graph, Camshaft's downstream tools produce garbage. Success criteria: agent runs validate after structural changes.

---

## Scenario 5: What-If Impact Analysis

**One-line purpose:** Verify `query what-if` correctly propagates a duration change through the graph and that the agent uses the result to decide whether to accept a scope change.

### Setup

Project: same web app as Scenario 2 (15 tasks, 19h CP). Agent is asked mid-sprint: "The backend auth team says BE2 (implement auth endpoints) will actually take 10h, not 5h. How bad is that?"

### Expected Agent Workflow

```bash
# Assume Scenario 2 plan is already loaded.
camshaft query critical-path         # baseline
camshaft query what-if --task BE2 --new-duration 10
camshaft query what-if --task BE3 --new-duration 10
# agent compares the two impacts to answer follow-up question:
# "what if we doubled BE3 instead?"
```

### Known-Correct Answer

Baseline critical path = `[S1, S2, BE1, BE2, BE4, QA2, DEP1]` = **19h**, with BE2 = 5h.

**What-if BE2 := 10h:**
- New CP same path: 1+2+2+10+3+4+2 = **24h**. Delta = +5h. CP unchanged.
- Downstream tasks delayed by 5h: BE4, QA2, DEP1, and anything waiting on BE2 (FE2).

**What-if BE3 := 10h (comparison, BE2 stays at 5h):**
- New path through BE3: 1+2+2+10+3+4+2 = 24h.
- Path through BE2: still 19h.
- New CP = path through BE3 = 24h. **CP shifts** from BE2-chain to BE3-chain.
- Delta = +5h, but critical path changed — bottleneck moved.

### Success Criteria

1. `what-if BE2 new_duration=10`:
   - `new_makespan_hours == 24`.
   - `makespan_delta == 5` (or `+5`).
   - `critical_path` unchanged (still goes through BE2).
   - `affected_tasks` includes BE4, QA2, DEP1, and FE2.
2. `what-if BE3 new_duration=10`:
   - `new_makespan_hours == 24`.
   - `makespan_delta == 5`.
   - `critical_path` now goes through BE3 (not BE2).
   - Tool reports `critical_path_changed == true`.
3. What-if is **non-destructive**: running `query critical-path` after both what-ifs returns the original baseline (19h, BE2-chain).
4. Agent synthesizes: "Doubling BE2 costs 5h. Doubling BE3 also costs 5h but shifts the critical path — means BE2 no longer the bottleneck."

### Failure Modes to Watch

- **what-if mutates state:** subsequent `query critical-path` reflects the what-if instead of baseline.
- **delta miscounted:** tool reports +10 (full new duration) instead of +5 (delta).
- **critical path change missed:** BE3 what-if still reports BE2-chain as CP.
- **Affected-tasks set too narrow:** omits FE2, which waits on BE2.
- **Affected-tasks set too wide:** includes CI1 or FE1, which are upstream / orthogonal.
- **Agent behaviour:** agent reports "BE2 will take 5h more" without checking downstream impact; or worse, tells the user "no problem" when the critical path has shifted.
- **Edge case:** what-if with `new_duration == current` should return delta 0 and no-op, not error.

---

## Running the Battle Tests

Each scenario should be runnable as:

```bash
cargo test --test battle_test_<N>
```

or as a shell harness that:

1. Creates a temp project dir.
2. Runs the expected agent workflow commands.
3. Asserts each success criterion against Camshaft's JSON output (use `--format json` on each query).
4. Cleans up.

Failure output should name the specific success criterion that failed and print the offending JSON snippet for debugging.

### Coverage summary

| Scenario | Tests |
|----------|-------|
| 1. Diamond | CPM correctness, parallel detection, slack calculation |
| 2. Web app | Realistic multi-path CPM, bottleneck ranking, topological ordering |
| 3. Sprint overcommit | Capacity math, priority-aware descope, dep-respecting selection |
| 4. Cycle | Validation correctness, cycle reporting actionability, state recovery |
| 5. What-if | Non-destructive simulation, delta propagation, CP shift detection |

These five cover: pure graph correctness (1, 4), realistic scale (2), resource-constrained planning (3), and dynamic re-planning (5). Scenarios 6–8 from the design brief (linear, roadmap milestones, incremental refinement) are good candidates for a future second tier once these five are green.
