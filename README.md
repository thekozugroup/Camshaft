# Camshaft

GanttML-powered planning engine for AI code agents. Creates, optimizes, and battle-tests Gantt charts with full dependency resolution to find the optimal order of operations.

Built on [GanttML](https://github.com/thekozugroup/GanttML) - a high-performance scheduling engine with CPM (Critical Path Method), resource optimization, and agentic planning capabilities.

## Features

- **Critical Path Analysis** - identify bottleneck tasks that determine project duration
- **Parallel Execution Detection** - find tasks that can run simultaneously for subagent dispatch
- **What-If Analysis** - simulate duration changes and see impact on the entire plan
- **Sprint Planning** - agentic task scheduling with priority-aware ordering
- **Dependency Validation** - cycle detection, orphan checks, missing reference detection
- **Dual Mode** - sprint mode (hours, parallel subagents) and roadmap mode (days, milestones)

## Installation

```bash
cargo install --path .
```

Requires [GanttML](https://github.com/thekozugroup/GanttML) as a sibling directory (`../GanttML`).

## Quick Start

```bash
# Create a sprint plan
camshaft init --name "Auth System" --mode sprint

# Add tasks
camshaft add task design-api --name "Design API Schema" --duration 4 --priority high
camshaft add task impl-auth --name "Implement Auth" --duration 8 --priority critical
camshaft add task write-tests --name "Write Tests" --duration 6 --priority high
camshaft add task setup-ci --name "Setup CI" --duration 2 --priority medium

# Add dependencies
camshaft add dep design-api impl-auth
camshaft add dep design-api write-tests
camshaft add dep impl-auth setup-ci
camshaft add dep write-tests setup-ci

# Validate and optimize
camshaft validate
camshaft optimize
```

**Output:**
```json
{
  "project_duration": 14.0,
  "critical_path": ["design-api", "impl-auth", "setup-ci"],
  "parallel_groups": [
    {"group": 1, "tasks": ["design-api"]},
    {"group": 2, "tasks": ["impl-auth", "write-tests"]},
    {"group": 3, "tasks": ["setup-ci"]}
  ],
  "suggested_order": ["group1: design-api", "group2: impl-auth || write-tests", "group3: setup-ci"]
}
```

## Commands

| Command | Description |
|---------|-------------|
| `camshaft init` | Create a new plan (sprint or roadmap mode) |
| `camshaft add task` | Add a task with duration and priority |
| `camshaft add dep` | Add a dependency between tasks |
| `camshaft add milestone` | Add a zero-duration milestone |
| `camshaft add resource` | Add a resource (labor, material, equipment) |
| `camshaft add assign` | Assign a resource to a task |
| `camshaft remove task` | Remove a task and its dependencies |
| `camshaft remove dep` | Remove a specific dependency |
| `camshaft optimize` | Run CPM analysis and find optimal order |
| `camshaft query critical-path` | Show the critical path |
| `camshaft query status` | Show all tasks with dates and float |
| `camshaft query bottlenecks` | Identify zero-float tasks |
| `camshaft query parallel` | Show parallelizable task groups |
| `camshaft query suggest-order` | Get optimal execution order |
| `camshaft query what-if` | Simulate duration changes |
| `camshaft sprint plan` | Generate a sprint plan |
| `camshaft sprint suggest` | Get daily schedule suggestion |
| `camshaft sprint overcommit-check` | Check for overcommitment |
| `camshaft validate` | Check plan integrity (cycles, missing deps) |
| `camshaft export` | Export plan as JSON |

## Modes

### Sprint Mode
For short-term implementation planning. Durations in hours. Optimizes for parallel subagent dispatch.

```bash
camshaft init --name "Feature Sprint" --mode sprint
```

### Roadmap Mode
For long-term project planning. Durations in days. Optimizes for critical path and timeline.

```bash
camshaft init --name "Q2 Roadmap" --mode roadmap --start 2026-05-01
```

## Agent Integration

Camshaft includes a Claude Code skill file (`skill/SKILL.md`) that teaches AI agents when and how to use Gantt-based planning. The JSON-in/JSON-out design is agent-agnostic, with planned support for Hermes and other agent frameworks.

## License

MIT
