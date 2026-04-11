# Camshaft — GanttML-Powered Planning Engine for Claude Code

**Date:** 2026-04-11
**Status:** Design
**Author:** Claude + Michael Wong

## Overview

Camshaft is a Rust CLI tool that supercharges Claude Code's planning capabilities by leveraging GanttML as its scheduling engine. It creates, optimizes, and battle-tests Gantt charts with full dependency resolution to find the optimal order of operations for both short-term sprint planning and long-term roadmap planning.

## Problem Statement

Claude Code plans linearly — sequential task lists without dependency awareness, critical path analysis, or parallelism optimization. Complex projects need:

- Dependency-aware ordering (can't test what isn't built)
- Critical path identification (what blocks everything?)
- Parallel execution detection (what subagents can run simultaneously?)
- Resource-constrained scheduling (limited developer time, API rate limits)
- What-if analysis (what happens if task X takes 3x longer?)

## Architecture

### Project Structure

```
~/Developer/Camshaft/
├── Cargo.toml              # depends on gantt_ml (path = "../GanttML")
├── src/
│   ├── main.rs             # clap CLI entry point
│   ├── commands/
│   │   ├── mod.rs          # command registry
│   │   ├── init.rs         # create new plan (sprint or roadmap mode)
│   │   ├── add.rs          # add-task, add-dep, add-milestone, add-resource
│   │   ├── remove.rs       # remove-task, remove-dep
│   │   ├── optimize.rs     # run CPM, find critical path, suggest reordering
│   │   ├── query.rs        # critical-path, status, what-if, bottlenecks
│   │   ├── sprint.rs       # agentic sprint planning (SmartTask integration)
│   │   ├── validate.rs     # check for cycles, missing deps, orphan tasks
│   │   └── export.rs       # JSON export (future: mermaid, ascii via separate script)
│   ├── plan.rs             # plan state management (load/save .camshaft/plan.json)
│   └── modes.rs            # sprint vs roadmap mode configuration
├── skill/
│   └── SKILL.md            # Claude Code skill file
├── scripts/
│   └── visualize.py        # optional: convert JSON to mermaid/ascii (not required)
├── tests/
│   ├── integration/
│   │   ├── cli_tests.rs    # end-to-end CLI tests
│   │   ├── sprint_tests.rs # sprint planning scenarios
│   │   └── roadmap_tests.rs# long-term planning scenarios
│   └── fixtures/           # sample plan JSON files
└── README.md
```

### Dependencies

```toml
[dependencies]
gantt_ml = { path = "../GanttML", features = ["agentic", "file-io"] }
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
thiserror = "2"
```

### State Management

Plan state persists as `.camshaft/plan.json` in the working directory. Uses GanttML's native `GanttMlFile` envelope format with additional Camshaft metadata:

```json
{
  "version": "0.1.0",
  "format": "camshaft",
  "mode": "sprint",
  "created_at": "2026-04-11T10:00:00Z",
  "project": { /* GanttML Project */ },
  "camshaft_meta": {
    "mode": "sprint",
    "last_optimized": "2026-04-11T10:05:00Z",
    "optimization_runs": 3,
    "parallelism_groups": [["A1", "A2"], ["A3", "A4", "A5"]]
  }
}
```

## CLI Commands

### `camshaft init`

Create a new plan.

```bash
camshaft init --mode sprint --name "Auth System Implementation"
camshaft init --mode roadmap --name "Q2 Product Roadmap" --start 2026-04-14
```

**Sprint mode defaults:** tasks as SmartTasks with priority/energy, hours-based durations, parallel subagent optimization.

**Roadmap mode defaults:** tasks as Activities with day-based durations, milestone support, CPM-focused.

### `camshaft add`

Add tasks, dependencies, milestones, resources.

```bash
# Tasks
camshaft add task "design-api" --name "Design API Schema" --duration 4 --priority high
camshaft add task "impl-auth" --name "Implement Auth" --duration 8 --priority critical
camshaft add task "write-tests" --name "Write Tests" --duration 6 --priority high

# Dependencies
camshaft add dep design-api impl-auth                    # finish-to-start (default)
camshaft add dep design-api write-tests                  # parallel with impl-auth
camshaft add dep impl-auth write-tests --type ss --lag 2 # start-to-start with 2-unit lag

# Milestones
camshaft add milestone "mvp-ready" --name "MVP Ready"
camshaft add dep write-tests mvp-ready
camshaft add dep impl-auth mvp-ready

# Resources (optional)
camshaft add resource "agent-1" --type labor --units 8
camshaft assign "impl-auth" "agent-1" --units 8
```

### `camshaft remove`

Remove tasks or dependencies.

```bash
camshaft remove task "design-api"     # removes task and all its dependencies
camshaft remove dep design-api impl-auth  # removes specific dependency
```

### `camshaft optimize`

Run CPM analysis and optimization. Core command.

```bash
camshaft optimize                           # full optimization
camshaft optimize --objective min-duration   # minimize total time
camshaft optimize --objective min-cost       # minimize resource cost
camshaft optimize --fast-track              # convert FS to SS where safe
camshaft optimize --crash                   # compress critical path durations
```

**Output (JSON to stdout):**

```json
{
  "project_duration": 18.0,
  "critical_path": ["design-api", "impl-auth", "mvp-ready"],
  "parallel_groups": [
    {"group": 1, "tasks": ["impl-auth", "write-tests"], "can_start_after": "design-api"},
    {"group": 2, "tasks": ["mvp-ready"], "can_start_after": ["impl-auth", "write-tests"]}
  ],
  "total_float": {"design-api": 0.0, "impl-auth": 0.0, "write-tests": 2.0, "mvp-ready": 0.0},
  "bottlenecks": ["design-api"],
  "suggested_order": ["design-api", "impl-auth || write-tests", "mvp-ready"],
  "optimization_moves": []
}
```

### `camshaft query`

Query plan state without modifying.

```bash
camshaft query critical-path    # show critical path
camshaft query status           # show all tasks with computed dates and float
camshaft query bottlenecks      # identify tasks with zero float
camshaft query parallel         # show parallelizable task groups
camshaft query what-if "impl-auth" --duration 16  # what if this task takes 2x?
camshaft query suggest-order    # optimal execution order for Claude
```

### `camshaft sprint`

Agentic sprint planning (uses GanttML's SmartTask/TaskScheduler).

```bash
camshaft sprint plan --capacity 40 --hours-per-day 8
camshaft sprint suggest                    # AI-friendly daily schedule
camshaft sprint overcommit-check           # detect overcommitment
```

### `camshaft validate`

Check plan integrity.

```bash
camshaft validate    # check for cycles, missing deps, orphan tasks, invalid refs
```

### `camshaft export`

Export the plan.

```bash
camshaft export                  # JSON to stdout (default)
camshaft export --file plan.json # JSON to file
```

## Plan Modes

### Sprint Mode (Short-Term)

For implementation planning within a single development cycle.

- **Duration units**: hours
- **Task model**: SmartTask (priority, energy, category, deadline)
- **Optimization focus**: parallel subagent dispatch, minimize wall-clock time
- **Output emphasis**: parallel groups, execution order, which tasks can run simultaneously
- **Typical use**: "Plan how to implement this feature" or "Break down this PR into parallelizable work"

### Roadmap Mode (Long-Term)

For project/product planning across weeks or months.

- **Duration units**: days
- **Task model**: Activity with milestones and WBS
- **Optimization focus**: critical path, resource leveling, timeline
- **Output emphasis**: critical path, milestones, float analysis, bottlenecks
- **Typical use**: "Plan Q2 deliverables" or "Map out the migration to the new architecture"

### Mode Detection

The skill instructs Claude to choose mode based on context:
- If planning implementation tasks for current session → sprint
- If planning across multiple sessions/weeks → roadmap
- User can override with `--mode` flag

## Skill Integration

The skill (`skill/SKILL.md`) teaches Claude Code:

1. **When to invoke**: before any multi-step implementation, after brainstorming produces a task list
2. **Workflow**:
   - `camshaft init` with appropriate mode
   - `camshaft add task` for each identified task
   - `camshaft add dep` for known dependencies
   - `camshaft validate` to catch issues
   - `camshaft optimize` to find optimal order
   - Parse JSON output to determine execution sequence
   - For sprint mode: dispatch parallel groups to subagents
   - For roadmap mode: present critical path and timeline to user
3. **Iterative refinement**: as Claude discovers new tasks/constraints during execution, `camshaft add` and re-optimize
4. **What-if analysis**: before making scope decisions, use `camshaft query what-if` to understand impact

## Error Handling

All errors output structured JSON to stderr:

```json
{
  "error": "cycle_detected",
  "message": "Circular dependency: A -> B -> C -> A",
  "affected_tasks": ["A", "B", "C"],
  "suggestion": "Remove one dependency to break the cycle"
}
```

Error types:
- `no_plan` — no .camshaft/plan.json found
- `cycle_detected` — circular dependency
- `task_not_found` — referenced task doesn't exist
- `duplicate_task` — task ID already exists
- `invalid_dependency` — dependency references nonexistent task
- `validation_failed` — plan has structural issues
- `optimization_failed` — optimizer couldn't converge

## Testing Strategy

### Unit Tests
- Each command module has unit tests for its core logic
- Plan state serialization/deserialization round-trips
- Mode configuration defaults

### Integration Tests
- Full CLI invocation via `assert_cmd` crate
- Sprint scenario: 10 tasks with dependencies → optimize → verify parallel groups
- Roadmap scenario: 20 tasks with milestones → CPM → verify critical path
- Error scenarios: cycles, missing deps, invalid inputs
- What-if queries with known expected outcomes

### Battle Testing
- Real-world planning scenarios (feature implementation, migration, refactor)
- Stress test with 100+ tasks
- Verify optimization actually improves over naive sequential ordering

## Success Criteria

1. `camshaft optimize` produces a valid execution order that respects all dependencies
2. Parallel groups are correctly identified — no task in a group depends on another in the same group
3. Critical path is accurate per CPM algorithm
4. What-if analysis correctly propagates duration changes through the dependency graph
5. Sprint mode produces actionable parallel dispatch instructions for subagents
6. Roadmap mode produces meaningful timeline and milestone analysis
7. Full round-trip: init → add tasks → add deps → validate → optimize → query → export
8. All JSON output is parseable and well-structured for Claude consumption
9. Error messages are actionable and suggest fixes
10. CLI responds in <100ms for plans with <100 tasks

## Agent Compatibility

### Claude Code (Primary)
The skill file (`skill/SKILL.md`) provides direct integration. Claude invokes `camshaft` CLI commands and parses JSON output to drive planning decisions.

### Hermes Agent (Planned)
Camshaft's JSON-in/JSON-out design makes it agent-agnostic. Hermes integration roadmap:
- JSON output format is already Hermes-consumable (structured, parseable)
- Future: dedicated Hermes skill/tool definition that maps Camshaft commands to Hermes tool calls
- Future: Hermes-specific output annotations (task routing, agent capabilities matching)
- The CLI's exit codes + structured JSON errors work for any agent's error handling

**Design principle:** Camshaft is agent-agnostic at the CLI layer. Agent-specific integration lives in skill files, not in the binary.

## Future Extensions (Not In Scope for v1)

- Hermes agent skill file (after v1 Claude Code skill is validated)
- Visual output (Mermaid, ASCII) — separate script, convert from JSON
- Monte Carlo simulation for duration estimation
- Resource leveling optimization
- Integration with git history for velocity estimation
- Web dashboard
- Multi-project portfolio planning
