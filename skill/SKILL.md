---
name: camshaft-planner
description: Optimize planning with dependency-aware Gantt charts, critical path analysis, and parallel execution detection. Use before any multi-step implementation or project planning.
version: 0.1.0
platforms: [macos, linux]
metadata:
  hermes:
    tags: [planning, gantt, rust, cli, scheduling]
    category: productivity
    requires_toolsets: [terminal]
---

# Camshaft Planning Skill

Camshaft is a Rust CLI that wraps GanttML to create, optimize, and analyze Gantt charts. It determines optimal execution order, identifies parallelizable work, and finds critical paths.

## When to Invoke

Activate this skill when:

- You have a multi-step implementation plan and need to determine execution order before coding
- Planning parallel subagent dispatch (which tasks can run simultaneously)
- The user asks to plan a project, roadmap, or sprint
- Optimizing execution order matters (dependencies, critical path, bottlenecks)
- After brainstorming/design phase, before execution phase

## When NOT to Use

Skip this skill for:

- **Single-step tasks.** One-off edits, bug fixes, or a single file change — overhead outweighs benefit.
- **Fewer than 3 tasks.** A 2-task sequence doesn't need CPM analysis; just run them in order.
- **No dependencies between tasks.** If every task is independent and non-ordered, parallel dispatch logic trivially degenerates — skip the planner.
- **Pure exploration or research work.** Open-ended investigation, reading docs, or debugging sessions where the task graph is discovered, not planned.
- **User wants raw plan output, not optimized order.** If the user just asked you to enumerate steps or draft a checklist, don't impose a Gantt schedule on them.
- **Real-time conversational tasks.** Answering questions, writing prose, reviewing a PR.

## Modes

### Sprint Mode (Implementation Planning)

For short-term work measured in **hours**. Optimizes for parallel subagent dispatch.

Use when: implementing a feature, fixing a set of bugs, executing a technical plan.

### Roadmap Mode (Project Planning)

For long-term work measured in **days**. Optimizes for critical path and timeline.

Use when: planning a multi-week project, creating a release plan, scheduling milestones.

## CLI Reference

### Initialize a Plan

```bash
camshaft init --name "Plan Name" --mode sprint
camshaft init --name "Plan Name" --mode roadmap --start 2025-03-15
camshaft init --name "Plan Name" --force        # Overwrite existing plan
```

### Add Tasks

```bash
camshaft add task T1 --name "Set up database schema" --duration 2 --priority critical --category feature
camshaft add task T2 --name "Write API endpoints" --duration 3 --priority high --category feature
camshaft add task T3 --name "Add input validation" --duration 1 --priority medium --category chore
```

Duration is hours (sprint) or days (roadmap). Priority: critical, high, medium, low. Category is a free-form label (feature, bug, chore, research are conventional).

### Add Dependencies

```bash
camshaft add dep T1 T2                          # T1 must finish before T2 starts (finish-to-start)
camshaft add dep T1 T3 --type ss                # T1 and T3 start together (start-to-start)
camshaft add dep T2 T4 --type fs --lag 1        # T4 starts 1 unit after T2 finishes
```

Dependency types: fs (finish-to-start, default), ss (start-to-start), ff (finish-to-finish), sf (start-to-finish).

### Add Milestones and Resources

```bash
camshaft add milestone M1 --name "MVP Complete"
camshaft add resource R1 --name "Backend Dev" --type labor --units 1
camshaft add assign T1 R1 --units 1
```

Resource types: labor, material, equipment.

### Remove Items

```bash
camshaft remove task T3
camshaft remove dep T1 T2
```

### Validate

```bash
camshaft validate
```

Always validate before optimizing. Catches circular dependencies and missing references.

### Optimize

```bash
camshaft optimize                                        # Default: minimize duration
camshaft optimize --objective min-duration --fast-track  # Aggressive parallelization
camshaft optimize --objective min-cost --crash           # Reduce cost with crashing
```

### Query

```bash
camshaft query critical-path           # Tasks that determine the minimum timeline
camshaft query status                  # All tasks with computed dates and float
camshaft query bottlenecks             # Zero-float tasks
camshaft query parallel                # Tasks that can execute simultaneously
camshaft query suggest-order           # Recommended execution sequence
camshaft query what-if T3 --duration 5 # What happens if T3 takes 5 instead
```

### Sprint-Specific Commands

```bash
camshaft sprint plan --capacity 3 --hours-per-day 6    # Plan with 3 parallel agents, 6h/day
camshaft sprint suggest                                 # Daily task scheduling suggestion
camshaft sprint overcommit-check --hours-per-day 8      # Check for overcommitted days
```

### Export

```bash
camshaft export --file plan.gantt.json
camshaft export                          # Print JSON to stdout
```

## Forthcoming Commands

These commands are being added to Camshaft. Check `camshaft --help` before using; if missing, the feature has not shipped in the installed build yet.

### Task Completion Tracking

```bash
camshaft task complete T1        # Mark T1 as complete
camshaft query ready             # List unblocked, uncompleted tasks (next batch)
```

### Plan Import

```bash
camshaft import --file path.json   # Load an external plan JSON
```

### Schedule Analytics

```bash
camshaft risk-analysis            # Monte Carlo simulation over duration uncertainty
camshaft analyze                  # Schedule health scoring + anomaly detection
camshaft evm                      # Earned Value Management metrics (CV, SV, CPI, SPI)
camshaft diff --baseline plan.json  # Compare current plan to a saved baseline
```

## Sprint Mode Workflow

Follow this sequence when planning implementation work:

### Step 1: Initialize

```bash
camshaft init --name "Implement Auth System" --mode sprint
```

### Step 2: Add Tasks from Implementation Plan

Convert each implementation step into a task with an estimated duration in hours:

```bash
camshaft add task T1 --name "Define user model and migrations" --duration 1 --priority critical --category feature
camshaft add task T2 --name "Implement password hashing service" --duration 1 --priority critical --category feature
camshaft add task T3 --name "Build login endpoint" --duration 2 --priority critical --category feature
camshaft add task T4 --name "Build registration endpoint" --duration 2 --priority high --category feature
camshaft add task T5 --name "Add JWT token generation" --duration 1 --priority critical --category feature
camshaft add task T6 --name "Write auth middleware" --duration 2 --priority high --category feature
camshaft add task T7 --name "Add rate limiting" --duration 1 --priority medium --category chore
camshaft add task T8 --name "Write integration tests" --duration 2 --priority high --category chore
```

### Step 3: Add Dependencies

```bash
camshaft add dep T1 T3
camshaft add dep T1 T4
camshaft add dep T2 T3
camshaft add dep T2 T4
camshaft add dep T3 T5
camshaft add dep T5 T6
camshaft add dep T6 T7
camshaft add dep T3 T8
camshaft add dep T4 T8
```

### Step 4: Validate

```bash
camshaft validate
```

### Step 5: Optimize

```bash
camshaft optimize --fast-track
```

### Step 6: Dispatch Parallel Groups

Parse the `parallel_groups` field from the JSON output. Dispatch each group as concurrent subagents; wait for a group to finish before starting the next. Tasks within a group have no mutual dependencies.

### Step 7: Track Completion and Pull Next Batch

After a subagent finishes a task, mark it done and query the next ready batch:

```bash
camshaft task complete T1
camshaft task complete T2
camshaft query ready      # Returns tasks whose predecessors are all complete
```

This replaces static parallel-group iteration with a dynamic ready-queue loop: complete → query ready → dispatch → repeat until empty.

## Roadmap Mode Workflow

### Step 1: Initialize with Start Date

```bash
camshaft init --name "Q2 Platform Rewrite" --mode roadmap --start 2025-04-01
```

### Step 2: Add Milestones, Tasks, Dependencies

```bash
camshaft add milestone M1 --name "Core API Complete"
camshaft add task T1 --name "Design new API schema" --duration 5 --priority critical --category research
camshaft add task T2 --name "Implement core endpoints" --duration 10 --priority critical --category feature
camshaft add dep T1 T2
camshaft add dep T2 T4 --type ss --lag 3
```

### Step 3: Optimize, Query, and Analyze

```bash
camshaft validate
camshaft optimize
camshaft query critical-path
camshaft query bottlenecks
camshaft query what-if T2 --duration 15
camshaft risk-analysis                        # If available
camshaft analyze                              # Schedule health check
```

Present the critical path, total timeline, and risk scenarios to the user.

## Error Recovery

### `validate` returns `{ "valid": false }`

Do not call `optimize`. The errors array names the problem. Common cases:

- **Cycle detected (e.g. T1 → T2 → T1):** remove one of the offending deps with `camshaft remove dep <FROM> <TO>`, then re-validate.
- **Missing task reference:** either add the referenced task, or remove the dangling dependency.
- **Duplicate task ID:** remove the old one or pick a different ID on the next `add task`.

Re-run `camshaft validate` until it returns `{ "valid": true }` before proceeding to `optimize`.

### `optimize` fails or returns an empty schedule

- **Empty plan:** no tasks added yet — return to Step 2 and add tasks.
- **All tasks blocked:** every task has an unresolvable predecessor. Run `camshaft query status` to inspect; likely a missing root task. Add an unblocked starting task.
- **Optimizer error:** re-run `camshaft validate`; a subtle graph issue may have slipped through. If validation passes but optimize still fails, export the plan (`camshaft export --file debug.json`) and report the JSON to the user rather than guessing.

### `query ready` returns an empty list but tasks remain

Either every remaining task is blocked by an incomplete predecessor (check `camshaft query status`), or the plan is complete. If blocked, the most recent `task complete` call may have missed a task — verify with `query status` and mark the predecessor complete.

## Iterative Refinement

Plans evolve during execution:

1. Add newly discovered tasks: `camshaft add task T9 --name "Fix auth bug" --duration 1 --priority critical --category bug`
2. Add their dependencies: `camshaft add dep T3 T9`
3. Remove tasks no longer needed: `camshaft remove task T7`
4. Re-validate and re-optimize: `camshaft validate && camshaft optimize --fast-track`
5. Re-query `ready` to adjust dispatch

## JSON Output Handling

All camshaft commands output JSON. Parse these responses to make planning decisions. Never present raw JSON to the user — interpret and summarize.

### `optimize` output schema

Key fields agents should expect from `camshaft optimize`:

- `project_duration` — total duration in mode units (hours or days)
- `critical_path` — ordered array of task IDs that determine the minimum timeline; any slip extends the project
- `parallel_groups` — array of `{ group: N, tasks: [IDs] }`; each group can be dispatched concurrently
- `bottlenecks` — task IDs with zero float (cannot slip without extending timeline)
- `suggested_order` — optimal execution sequence accounting for priority and dependency
- `next_ready_tasks` — tasks whose predecessors are already satisfied (initial ready queue)
- `total_float` — per-task slack; tasks with larger float can absorb delay without affecting the critical path

### Other command outputs

- `query critical-path` → ordered list of task IDs on the critical path
- `query parallel` → groups of concurrently-runnable tasks
- `query ready` → tasks currently unblocked and uncompleted
- `query what-if` → changed duration and affected downstream tasks
- `validate` → `{ "valid": true }` or `{ "valid": false, "errors": [...] }`
- `sprint suggest` → day-by-day schedule with task assignments
- `risk-analysis` → probability distribution of completion dates
- `evm` → `{ pv, ev, ac, cv, sv, cpi, spi }`

## Key Principles

1. **Always validate before optimizing.** Catch cycles and errors early.
2. **Use sprint mode for implementation, roadmap mode for project planning.** Do not mix concerns.
3. **Task IDs should be short and meaningful.** Use T1-Tn for quick plans, or descriptive IDs like `auth-model`, `api-login` for complex plans.
4. **Dependencies flow forward.** `camshaft add dep A B` means A must complete before B starts.
5. **Critical path tasks cannot slip** without extending the total timeline. Focus attention here.
6. **Parallel groups are dispatch groups.** Each group can be safely executed by concurrent subagents.
7. **Prefer the complete → ready loop over static group iteration** once those commands ship — it handles mid-flight plan changes gracefully.
8. **Re-optimize after changes.** Any add/remove invalidates previous optimization results.
9. **Overcommit check prevents burnout.** Run `sprint overcommit-check` before committing to a schedule.
