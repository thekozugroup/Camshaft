# Real Agent End-to-End Test

**Date**: 2026-04-14
**Scenario**: A Claude Code agent receives the task "Implement user authentication for a Node.js Express API (JWT + refresh, bcrypt, login/register/logout, auth middleware, integration tests)" and uses the Camshaft skill to plan it before dispatch.

Working directory: `/tmp/real-agent-test/`
Binary: `~/Developer/Camshaft/target/debug/camshaft`

---

## Transcript

### Step 0 — Read the skill

Agent reasoning: The task is multi-step implementation work with interdependent pieces (model → endpoints → middleware → tests). This is exactly the "invoke the skill" case. Read `/Users/michaelwong/Developer/Camshaft/skill/SKILL.md`.

### Step 1 — `init`

```bash
camshaft init --name "Node.js Express Auth Implementation" --mode sprint --force
```

Response:
```json
{ "mode": "sprint", "name": "Node.js Express Auth Implementation", "status": "created" }
```

Agent reasoning: Sprint mode — this is hours of work, not a multi-week roadmap.

### Step 2 — Add 12 tasks

Broke the auth implementation into 12 subtasks:

| ID  | Task                                                        | Duration | Priority | Category |
|-----|-------------------------------------------------------------|----------|----------|----------|
| T1  | Project scaffolding (npm init, install deps)                | 1h       | critical | chore    |
| T2  | Define User model and store                                 | 1h       | critical | feature  |
| T3  | Password hashing service (bcrypt wrapper)                   | 1h       | critical | feature  |
| T4  | JWT service (sign/verify access + refresh)                  | 2h       | critical | feature  |
| T5  | POST /register endpoint                                     | 2h       | high     | feature  |
| T6  | POST /login endpoint                                        | 2h       | critical | feature  |
| T7  | POST /refresh endpoint                                      | 1h       | high     | feature  |
| T8  | POST /logout endpoint (token revocation)                    | 1h       | high     | feature  |
| T9  | Auth middleware                                             | 2h       | critical | feature  |
| T10 | Sample protected route for verification                     | 1h       | medium   | feature  |
| T11 | Integration tests (supertest + jest)                        | 3h       | high     | chore    |
| T12 | Env config + README + error handling polish                 | 1h       | medium   | chore    |

Agent reasoning: 12 is a reasonable granularity — small enough to parallelize, large enough to capture real dependency structure.

### Step 3 — Add 19 dependencies

Key relationships:
- T1 (scaffolding) blocks T2/T3/T4
- T2+T3 block T5 (register needs user store + hashing)
- T2+T3+T4 block T6 (login needs store + hashing + JWT)
- T4 blocks T7, T8, T9 (refresh, logout, middleware all need JWT)
- T6+T9 block T10 (protected route needs a working login to test)
- T5, T6, T7, T8, T10 all block T11 (integration tests)
- T11 blocks T12 (polish last)

All `fs` (finish-to-start, default).

### Step 4 — `validate`

```json
{ "valid": true, "task_count": 12, "dependency_count": 19, "can_optimize": true }
```

### Step 5 — `optimize --fast-track`

Warning returned:
```
Warning: --fast-track is not yet implemented (future feature).
```

But the plain optimization still ran and produced a complete schedule:

- **project_duration**: 10.0 hours
- **critical_path**: T1 → T4 → T9 → T6 → T10 → T11 → T12
- **bottlenecks** (zero float): T1, T4, T9, T6, T10, T11, T12
- **parallel_groups**:
  - Group 1 (t=0): T1
  - Group 2 (t=1): T2 || T3 || T4
  - Group 3 (t=2): T5
  - Group 4 (t=3): T6 || T7 || T8 || T9
  - Group 5 (t=5): T10
  - Group 6 (t=6): T11
  - Group 7 (t=9): T12
- **total_float**: T1/T4/T6/T9/T10/T11/T12 = 0; T2/T3 = 1; T5 = 2; T7/T8 = 2

Agent reasoning: Scheduler put T5 in its own group between G2 and G4 because only T2+T3 are its predecessors, not T4. Technically correct but a bit quirky — a more aggressive parallelizer would lump T5 into G4 since both start after T2+T3 finish. With three subagents, I'd probably override and run T2/T3/T4 in parallel, then T5/T6/T7/T8/T9 in the next batch.

### Step 6 — `query ready`

```json
{ "ready_tasks": [{ "id": "T1", ...}], "total_completed": 0, "total_remaining": 12 }
```

Only T1 unblocked. Dispatch it.

### Step 7 — Simulate completion, re-query

```bash
camshaft task complete T1
camshaft query ready
```

Next ready batch: **T2, T3, T4** — three parallel tasks, perfect for 3-agent dispatch.

```json
{
  "ready_tasks": [
    { "id": "T2", "name": "Define User model...", "priority_hint": "medium", "duration": 1.0 },
    { "id": "T3", "name": "Implement password hashing...", "priority_hint": "medium", "duration": 1.0 },
    { "id": "T4", "name": "Implement JWT service...", "priority_hint": "critical", "duration": 2.0 }
  ],
  "total_completed": 1, "total_ready": 3, "total_remaining": 11
}
```

---

## Findings

### 1. Skill quality — **Good, minor gaps**

**What worked**:
- Clear when/when-not-to-use boundary at top.
- The mode distinction (sprint = hours, roadmap = days) landed immediately.
- Step-by-step sprint workflow mapped cleanly onto my task.
- "Forthcoming commands" section correctly flagged `task complete` / `query ready` — I checked first and they are shipped.
- Error recovery section was reassuring even though I didn't need it.

**Gaps I had to guess on**:
- **No guidance on task granularity.** How small is "a task"? 30 min? 4 hours? I defaulted to 1–3h chunks, which felt right but was a judgement call. A sentence like "aim for 1-4 hour tasks in sprint mode" would remove the guesswork.
- **No guidance on dependency density.** I added 19 deps for 12 tasks. Is that a lot? Over-constrained? Under-constrained? No rubric.
- **`--fast-track` flag is documented but prints "not yet implemented" warning.** The skill shows `camshaft optimize --fast-track` as a canonical invocation (Step 5 of sprint workflow). An agent following the skill verbatim will hit this warning every time. Either ship the flag or remove it from the example.
- **No note on reading `priority_hint` in `query ready` output.** I set T2 and T3 as `critical`, but `query ready` returned them with `priority_hint: "medium"`. Seems like priority gets re-computed by float/slack rather than reflecting what I set. Unexpected — the skill should explain this (or the tool should preserve user priority).

### 2. CLI UX — **Mostly smooth, a few rough edges**

**Worked well**:
- Every command returned clean, parseable JSON.
- `init --force` saved me when I re-ran the test.
- `validate` → `optimize` → `query ready` → `task complete` → `query ready` loop is exactly the ergonomic flow the skill promises.
- Error messages are nonexistent because nothing errored — can't evaluate error UX from this run.

**Rough edges**:
- **`--fast-track` warning.** Shown above. Flag appears in the help text / skill but isn't wired up. Either remove or implement.
- **Priority mutation.** Mentioned above. If the optimizer derives its own priority, output should use a different key (`schedule_priority`) rather than overwriting the user-set one.
- **Verbose JSON per add command.** Every `add task` / `add dep` returns a 5-line JSON object. For 12 tasks + 19 deps = 31 echoes of boilerplate. A `--quiet` mode or a bulk `add tasks --from file.yaml` would be far nicer for agent use.
- **No `camshaft status` summary.** After 19 `add dep` calls, I wanted a one-liner "12 tasks, 19 deps, 0 cycles" to sanity-check before validate. `validate` itself provides this, but calling it "status" in addition to "validate" would help.

### 3. Output quality — **Actionable and dispatchable**

The `optimize` output has everything an agent needs:
- `parallel_groups` → direct mapping to concurrent subagent dispatch.
- `critical_path` → tells me which slips extend the project.
- `bottlenecks` + `total_float` → tells me where to focus review effort.
- `execution_hint` → single-line natural-language guidance, nice touch.

`query ready` after `task complete` is the cleanest interface — this replaces static group-iteration (which the skill acknowledges) and handles mid-flight plan changes naturally. After completing T1 I got a clean 3-task ready batch that I can dispatch to three subagents without further thought.

The one thing missing: **no estimate of wall-clock time with N agents**. I can eyeball it (critical path = 10h, so with infinite agents min = 10h), but a `sprint plan --capacity 3` output would be welcome. The skill mentions this command but I didn't invoke it.

### 4. Would a real agent succeed — **Yes, with minor friction**

I could have dispatched this task without guidance. The flow worked:

1. Skill told me when to invoke — clear.
2. Init + add tasks + add deps — syntactically obvious from skill examples.
3. Validate confirmed no cycles.
4. Optimize gave me the schedule and parallel groups.
5. Query ready gave me the first dispatch batch.
6. After complete, re-query gave me the next batch.

**Friction points that an agent could hit**:
- The `--fast-track` warning would make a less confident agent pause and wonder if its plan is wrong. (It's not — optimizer still runs.)
- If the user edits the plan mid-flight (adds a T13), the skill says "re-validate, re-optimize" but doesn't explicitly mention whether `task complete` history survives re-optimization. I didn't test this.
- The priority mutation surprise: if an agent relies on `priority_hint` to pick which of three parallel tasks to label as "most critical" for the user, it'll get the wrong answer.

Overall: **the skill and binary are good enough for a real agent to use unsupervised on a medium-complexity task.** The gaps are polish items, not blockers.

---

## Recommendations (priority order)

1. **Fix or remove `--fast-track`.** Either wire it up or strip it from the skill example. A documented flag that prints "not implemented" is the worst of both worlds.
2. **Preserve user-set priority in output.** Rename the derived one to `schedule_priority` or `urgency`.
3. **Add a task-granularity sentence** to the skill: "Aim for 1-4 hour tasks in sprint mode; split anything larger."
4. **Add a bulk-add interface.** `camshaft add tasks --from plan.yaml` — huge quality-of-life for agent workflows generating 10+ tasks.
5. **Add a `--quiet` flag on add commands.** Cuts echoed boilerplate JSON by ~80%.
