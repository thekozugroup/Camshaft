Camshaft turns AI code agents into better planners. Instead of executing tasks in a flat sequential list, agents use Camshaft to build dependency-aware Gantt charts, identify which tasks can run in parallel, and find the critical path that determines total project duration. The result: faster execution through parallelism and smarter prioritization through CPM (Critical Path Method) analysis. The CLI ships 17 commands spanning plan construction, optimization, risk, and progress tracking.

Built as the first downstream project on [GanttML](https://github.com/thekozugroup/GanttML), a high-performance Rust scheduling engine.

## Screenshots

![Camshaft optimize output showing critical path analysis and parallel task groups](./docs/screenshot.png)

## How it works

Camshaft is a Rust CLI that agents invoke during planning. An agent describes its tasks and their dependencies, then Camshaft runs CPM analysis to compute the critical path, float values, and parallelizable task groups. The output is structured JSON the agent parses to decide execution order and which tasks to dispatch to parallel subagents.

Agents rarely want to fire 30 `add` calls. A single `bulk-import` command accepts a YAML or JSON file describing the entire plan — tasks, dependencies, durations, milestones — and builds it in one shot. A `progress` loop (`complete` then `query`) lets the agent mark tasks done mid-execution and re-query the critical path as reality diverges from the plan.

For risk-aware planning, a Monte Carlo command samples thousands of duration scenarios and returns P50/P90 completion estimates. When a deadline slips, `fast-track` and `crash` invoke GanttML's GeneticOptimizer to suggest which dependencies to parallelize or which tasks to throw more resources at. A `git-velocity` command reads recent commit history to calibrate duration estimates from the agent's actual pace.

Two modes adapt to different planning horizons: sprint mode (hour-based, parallel subagent dispatch) and roadmap mode (day-based, milestone support, resource leveling). A bundled Claude Code skill file teaches agents when and how to invoke Camshaft. The JSON-in/JSON-out design is agent-agnostic, with Hermes agent compatibility built in. See [docs/TUTORIAL.md](./docs/TUTORIAL.md) for a deeper walkthrough.

## Example

A bulk plan for a five-task auth feature:

```yaml
mode: sprint
tasks:
  - id: schema
    name: Design user schema
    hours: 2
  - id: migration
    name: Write DB migration
    hours: 3
    depends_on: [schema]
  - id: jwt
    name: JWT signing service
    hours: 4
    depends_on: [schema]
  - id: login
    name: Login endpoint
    hours: 3
    depends_on: [migration, jwt]
  - id: tests
    name: Integration tests
    hours: 4
    depends_on: [login]
```

`camshaft bulk-import auth.yaml && camshaft optimize` identifies `schema → jwt → login → tests` as the critical path (13h), flags `migration` as parallelizable with `jwt` (1h of float), and recommends dispatching both to subagents once `schema` completes.

## Stack

- Rust (CLI binary, clap for argument parsing)
- GanttML crate (CPM engine, agentic scheduling, Monte Carlo, GeneticOptimizer)
- serde, serde_json, serde_yaml (structured I/O for agent consumption and bulk import)
- chrono (date handling for roadmap mode)
- thiserror (structured error output with actionable suggestions)

## Status

Active
