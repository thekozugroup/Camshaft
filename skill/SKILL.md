---
name: camshaft-planner
description: Optimize planning with dependency-aware Gantt charts, critical path analysis, and parallel execution detection. Use before any multi-step implementation or project planning.
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

Do NOT use for single-step tasks or tasks with no dependencies.

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
```

### Add Tasks

```bash
camshaft add task T1 --name "Set up database schema" --duration 2 --priority critical --category feature
camshaft add task T2 --name "Write API endpoints" --duration 3 --priority high --category feature
camshaft add task T3 --name "Add input validation" --duration 1 --priority medium --category chore
```

Duration is hours (sprint) or days (roadmap). Priority: critical, high, medium, low. Category: feature, bug, chore, research.

### Add Dependencies

```bash
camshaft add dep T1 T2                          # T1 must finish before T2 starts (finish-to-start)
camshaft add dep T1 T3 --type ss               # T1 and T3 start together (start-to-start)
camshaft add dep T2 T4 --type fs --lag 1       # T4 starts 1 unit after T2 finishes
```

Dependency types: fs (finish-to-start, default), ss (start-to-start), ff (finish-to-finish), sf (start-to-finish).

### Add Milestones and Resources

```bash
camshaft add milestone M1 --name "MVP Complete"
camshaft add resource R1 --name "Backend Dev" --type labor --units 1
camshaft add assign T1 R1 --units 1
```

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
camshaft optimize                                       # Default: minimize duration
camshaft optimize --objective min-duration --fast-track  # Aggressive parallelization
camshaft optimize --objective min-cost --crash           # Reduce cost with crashing
```

### Query

```bash
camshaft query critical-path       # Which tasks determine the minimum timeline
camshaft query status              # Current state of the plan
camshaft query bottlenecks         # Resource and dependency bottlenecks
camshaft query parallel            # Tasks that can execute simultaneously
camshaft query suggest-order       # Recommended execution sequence
camshaft query what-if T3 --duration 5   # What happens if T3 takes 5 instead
```

### Sprint-Specific Commands

```bash
camshaft sprint plan --capacity 3 --hours-per-day 6    # Plan with 3 parallel agents, 6h/day
camshaft sprint suggest                                 # Get daily task scheduling suggestion
camshaft sprint overcommit-check --hours-per-day 8     # Check for overcommitted days
```

### Export

```bash
camshaft export --file plan.gantt.json
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

Determine what blocks what:

```bash
camshaft add dep T1 T3    # Model needed before login endpoint
camshaft add dep T1 T4    # Model needed before registration endpoint
camshaft add dep T2 T3    # Hashing needed before login
camshaft add dep T2 T4    # Hashing needed before registration
camshaft add dep T3 T5    # Login done before JWT
camshaft add dep T5 T6    # JWT done before middleware
camshaft add dep T6 T7    # Middleware done before rate limiting
camshaft add dep T3 T8    # Endpoints done before integration tests
camshaft add dep T4 T8    # Endpoints done before integration tests
```

### Step 4: Validate

```bash
camshaft validate
```

Fix any issues (cycles, missing references) before proceeding.

### Step 5: Optimize

```bash
camshaft optimize --fast-track
```

### Step 6: Parse Output and Dispatch

The optimize command returns JSON. Parse the `parallel_groups` field to determine which tasks can run as simultaneous subagents:

```json
{
  "parallel_groups": [
    {"group": 1, "tasks": ["T1", "T2"]},
    {"group": 2, "tasks": ["T3", "T4"]},
    {"group": 3, "tasks": ["T5"]},
    {"group": 4, "tasks": ["T6"]},
    {"group": 5, "tasks": ["T7", "T8"]}
  ],
  "critical_path": ["T1", "T3", "T5", "T6", "T7"],
  "estimated_duration": 8
}
```

Dispatch each group as parallel subagents, waiting for a group to complete before starting the next. Tasks within a group have no mutual dependencies and can safely run concurrently.

### Step 7: Use Sprint Suggest for Scheduling

```bash
camshaft sprint plan --capacity 3 --hours-per-day 6
camshaft sprint suggest
```

This provides a day-by-day schedule factoring in parallel capacity.

## Roadmap Mode Workflow

Follow this sequence for project-level planning:

### Step 1: Initialize with Start Date

```bash
camshaft init --name "Q2 Platform Rewrite" --mode roadmap --start 2025-04-01
```

### Step 2: Add Milestones

```bash
camshaft add milestone M1 --name "Core API Complete"
camshaft add milestone M2 --name "Frontend Migration Done"
camshaft add milestone M3 --name "Launch Ready"
```

### Step 3: Add Tasks with Day-Based Durations

```bash
camshaft add task T1 --name "Design new API schema" --duration 5 --priority critical --category research
camshaft add task T2 --name "Implement core endpoints" --duration 10 --priority critical --category feature
camshaft add task T3 --name "Migrate frontend to new API" --duration 8 --priority high --category feature
camshaft add task T4 --name "Performance testing" --duration 3 --priority high --category chore
camshaft add task T5 --name "Security audit" --duration 5 --priority critical --category chore
```

### Step 4: Add Dependencies

```bash
camshaft add dep T1 T2
camshaft add dep T2 T3
camshaft add dep T2 T4 --type ss --lag 3   # Perf testing starts 3 days into endpoint work
camshaft add dep T3 T5
camshaft add dep T4 T5
```

### Step 5: Optimize and Query

```bash
camshaft validate
camshaft optimize
camshaft query critical-path
camshaft query bottlenecks
```

### Step 6: Scenario Analysis

```bash
camshaft query what-if T2 --duration 15    # What if core endpoints take 15 days?
camshaft query what-if T3 --duration 12    # What if migration is harder than expected?
```

Present the critical path, total timeline, and risk scenarios to the user.

## Iterative Refinement

Plans evolve during execution. When new information emerges:

1. Add newly discovered tasks: `camshaft add task T9 --name "Fix auth bug" --duration 1 --priority critical --category bug`
2. Add their dependencies: `camshaft add dep T3 T9`
3. Remove tasks that are no longer needed: `camshaft remove task T7`
4. Re-optimize: `camshaft optimize --fast-track`
5. Re-query parallel groups to adjust subagent dispatch

## JSON Output Handling

All camshaft commands output JSON. Key patterns:

- **optimize** returns `parallel_groups`, `critical_path`, `estimated_duration`
- **query critical-path** returns the ordered list of tasks on the critical path
- **query parallel** returns groups of tasks that can execute concurrently
- **query what-if** returns the changed duration and affected tasks
- **validate** returns `{ "valid": true }` or `{ "valid": false, "errors": [...] }`
- **sprint suggest** returns a day-by-day schedule with task assignments

Parse these JSON responses to make planning decisions. Never present raw JSON to the user -- interpret and summarize the results.

## Key Principles

1. **Always validate before optimizing.** Catch cycles and errors early.
2. **Use sprint mode for implementation, roadmap mode for project planning.** Do not mix concerns.
3. **Task IDs should be short and meaningful.** Use T1-Tn for quick plans, or descriptive IDs like `auth-model`, `api-login` for complex plans.
4. **Dependencies flow forward.** `camshaft add dep A B` means A must complete before B starts.
5. **Critical path tasks cannot slip** without extending the total timeline. Focus attention here.
6. **Parallel groups are dispatch groups.** Each group can be safely executed by concurrent subagents.
7. **Re-optimize after changes.** Any add/remove invalidates previous optimization results.
8. **Overcommit check prevents burnout.** Run `sprint overcommit-check` before committing to a schedule.
