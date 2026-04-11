Camshaft turns AI code agents into better planners. Instead of executing tasks in a flat sequential list, agents use Camshaft to build dependency-aware Gantt charts, identify which tasks can run in parallel, and find the critical path that determines total project duration. The result: faster execution through parallelism and smarter prioritization through CPM (Critical Path Method) analysis.

Built as the first downstream project on [GanttML](https://github.com/thekozugroup/GanttML), a high-performance Rust scheduling engine.

## Screenshots

![Camshaft optimize output showing critical path analysis and parallel task groups](./docs/screenshot.png)

## How it works

Camshaft is a Rust CLI that agents invoke during planning. An agent describes its tasks and their dependencies, then Camshaft runs CPM analysis to compute the critical path, float values, and parallelizable task groups. The output is structured JSON that the agent parses to decide execution order and which tasks to dispatch to parallel subagents.

Two modes adapt to different planning horizons. Sprint mode uses hour-based durations and optimizes for parallel subagent dispatch during implementation. Roadmap mode uses day-based durations with milestone support and optimizes for timeline and resource leveling across weeks or months.

What-if analysis lets agents simulate the impact of scope changes before committing. Changing a single task's duration propagates through the entire dependency graph, showing exactly how it affects the critical path and total project duration.

A bundled Claude Code skill file teaches agents when to invoke Camshaft, how to build plans incrementally, and how to interpret the optimization output. The JSON-in/JSON-out design is agent-agnostic, with planned Hermes agent support.

## Stack

- Rust (CLI binary, clap for argument parsing)
- GanttML crate (CPM engine, dependency graphs, agentic scheduling)
- serde/serde_json (structured JSON I/O for agent consumption)
- chrono (date handling for roadmap mode)
- thiserror (structured error output with actionable suggestions)

## Status

Active
