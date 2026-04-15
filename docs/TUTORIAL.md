# Camshaft Tutorial

Learn to use Camshaft by walking through three real scenarios. This guide is written for AI coding agents (Claude Code, Hermes) and for human developers exploring Camshaft for the first time.

## Prerequisites

- Camshaft installed (`cargo install --path .` from the repo, or a prebuilt binary on `$PATH`)
- Basic familiarity with a terminal
- A one-paragraph primer on Gantt/CPM (below) is all you need

**Gantt/CPM in one paragraph.** A Gantt plan is a set of tasks, each with a duration and a list of predecessor dependencies. The Critical Path Method (CPM) walks that graph forward to compute each task's earliest start, then backward to compute the latest start that still hits the project end date. The difference is *float* — slack a task can absorb without slipping the deadline. Tasks with zero float form the *critical path*: they determine the total project duration. Any task can run in parallel with any other task it doesn't depend on, and Camshaft exposes those groupings for parallel subagent dispatch.

All Camshaft state lives in `.camshaft/` in the current directory. Every command emits JSON on stdout so agents can pipe through `jq` or parse directly.

## Scenario 1: Sprint Planning — Implement User Auth

You're adding JWT authentication to an Express API. The work breaks down into roughly ten hour-scale tasks with dependencies: schema before repo, repo before service, service before routes, and so on. The goal is to parallelize wherever the graph allows.

### Step 1 — initialize

```bash
camshaft init --name "JWT Auth" --mode sprint
```

`sprint` mode uses hours as the unit and optimizes for parallel subagent dispatch. `roadmap` is for day-scale planning — see Scenario 2.

### Step 2 — add tasks

```bash
camshaft add task schema      --name "User + session schema"    --duration 1
camshaft add task migrations  --name "Write migrations"         --duration 1
camshaft add task repo        --name "UserRepository"           --duration 2
camshaft add task hashing     --name "Password hashing utils"   --duration 1
camshaft add task jwt-utils   --name "JWT sign/verify helpers"  --duration 1
camshaft add task auth-svc    --name "AuthService (login/register)" --duration 3
camshaft add task middleware  --name "authenticate() middleware" --duration 2
camshaft add task routes      --name "POST /login, /register, /me" --duration 2
camshaft add task tests       --name "Integration tests"        --duration 3
camshaft add task docs        --name "README + API docs"        --duration 1
```

### Step 3 — add dependencies

```bash
camshaft add dep schema migrations
camshaft add dep schema repo
camshaft add dep migrations repo
camshaft add dep repo auth-svc
camshaft add dep hashing auth-svc
camshaft add dep jwt-utils auth-svc
camshaft add dep jwt-utils middleware
camshaft add dep auth-svc routes
camshaft add dep middleware routes
camshaft add dep routes tests
camshaft add dep routes docs
```

### Step 4 — validate, then optimize

```bash
camshaft validate
camshaft optimize
```

Abbreviated optimize output:

```json
{
  "project_duration": 12,
  "critical_path": ["schema", "repo", "auth-svc", "routes", "tests"],
  "parallel_groups": [
    { "group": 0, "tasks": ["schema", "hashing", "jwt-utils"] },
    { "group": 1, "tasks": ["migrations"] },
    { "group": 2, "tasks": ["repo"] },
    { "group": 3, "tasks": ["auth-svc", "middleware"] },
    { "group": 4, "tasks": ["routes"] },
    { "group": 5, "tasks": ["tests", "docs"] }
  ],
  "bottlenecks": ["schema", "repo", "auth-svc", "routes", "tests"],
  "next_ready_tasks": ["schema", "hashing", "jwt-utils"]
}
```

### Step 5 — how an agent reads this

1. `project_duration: 12` hours — with parallel dispatch, the work finishes in 12h even though the task total is 17h.
2. `parallel_groups[0] = [schema, hashing, jwt-utils]` — spawn three subagents *now*; none depend on each other.
3. `bottlenecks` equals `critical_path` — any slip on these tasks extends the project 1:1. Review their estimates first.

### Step 6 — dispatch and iterate

After group 0 finishes:

```bash
camshaft task complete schema
camshaft task complete hashing
camshaft task complete jwt-utils
camshaft query ready   # returns ["migrations"]
```

If a new task emerges mid-sprint (say, rate limiting on `/login`), add it and re-optimize:

```bash
camshaft add task rate-limit --name "Rate limit login" --duration 1
camshaft add dep jwt-utils rate-limit
camshaft add dep rate-limit routes
camshaft optimize
```

The critical path may shift. The agent keeps looping: complete → `query ready` → dispatch → optimize if the graph changed.

## Scenario 2: Roadmap Planning — Q2 Product Release

You're planning a 12-week release with four teams. Durations are in days, and milestones anchor cross-team coordination.

### Step 1 — initialize in roadmap mode

```bash
camshaft init --name "Q2 Release" --mode roadmap --start 2026-04-01 --force
```

### Step 2 — milestones and tasks

```bash
camshaft add milestone m-alpha --name "Alpha feature-complete"
camshaft add milestone m-beta  --name "Beta to customers"
camshaft add milestone m-ga    --name "GA launch"

camshaft add task api-refactor --name "API refactor"        --duration 10 --category backend
camshaft add task mobile-ui    --name "Mobile UI redesign"  --duration 15 --category mobile
camshaft add task web-ui       --name "Web UI update"       --duration 12 --category frontend
camshaft add task analytics    --name "Analytics pipeline"  --duration 8  --category data
camshaft add task perf         --name "Perf hardening"      --duration 6  --category backend
camshaft add task qa           --name "QA cycle"            --duration 10 --category qa
camshaft add task launch       --name "Marketing + launch"  --duration 5  --category growth
```

### Step 3 — cross-team dependencies

```bash
camshaft add dep api-refactor mobile-ui
camshaft add dep api-refactor web-ui
camshaft add dep api-refactor analytics
camshaft add dep mobile-ui m-alpha
camshaft add dep web-ui    m-alpha
camshaft add dep analytics m-alpha
camshaft add dep m-alpha   perf
camshaft add dep perf      qa
camshaft add dep qa        m-beta
camshaft add dep m-beta    launch
camshaft add dep launch    m-ga
```

### Step 4 — optimize and analyze

```bash
camshaft optimize
camshaft analyze
```

`analyze` returns a health score (0–100), a list of schedule quality issues (dangling tasks, over-long critical path, insufficient float), and suggested remediations.

### Step 5 — what-if for scope cuts

Exec wants GA two weeks earlier. Check whether trimming the mobile redesign helps:

```bash
camshaft query what-if mobile-ui --duration 10
```

Output shows the new `project_duration` and any shift in the critical path. If `mobile-ui` isn't critical, cutting it saves nothing — look elsewhere.

### Step 6 — baseline + diff

Save the approved plan as a baseline, then diff after each planning review:

```bash
camshaft export > baseline-q2.json
# ... plan changes happen ...
camshaft diff --baseline baseline-q2.json
```

The diff output lists added/removed/modified tasks and dependencies, plus the delta in project duration.

## Scenario 3: Risk-Aware Planning — Database Migration

You're migrating from Postgres 13 to 16 with schema changes in flight. Durations are uncertain; some tasks could easily double.

### Step 1 — build the plan

```bash
camshaft init --name "DB Migration" --mode sprint --force
camshaft add task snapshot       --name "Snapshot prod data"   --duration 2
camshaft add task upgrade-stage  --name "Upgrade staging PG"   --duration 4
camshaft add task test-staging   --name "Full app test suite"  --duration 6
camshaft add task migrate-schema --name "Apply schema changes" --duration 3
camshaft add task load-test      --name "Load test"            --duration 4
camshaft add task upgrade-prod   --name "Upgrade production"   --duration 3
camshaft add task verify-prod    --name "Post-migration verify" --duration 2

camshaft add dep snapshot upgrade-stage
camshaft add dep upgrade-stage test-staging
camshaft add dep test-staging migrate-schema
camshaft add dep migrate-schema load-test
camshaft add dep load-test upgrade-prod
camshaft add dep upgrade-prod verify-prod

camshaft optimize
```

### Step 2 — Monte Carlo risk analysis

```bash
camshaft risk-analysis
```

The simulator samples task durations from a distribution around each estimate (10k iterations by default) and returns:

- `p50_duration`, `p80_duration`, `p95_duration` — percentile outcomes
- `criticality_index` per task — fraction of iterations where that task sat on the critical path

Example reading: if `test-staging` has `criticality_index: 0.94`, it's *almost always* critical — invest in de-risking it. If `snapshot` is `0.12`, it rarely matters.

### Step 3 — use P80 for commitments

Quote the P80 duration to stakeholders. If deterministic optimize says 24h but P80 says 34h, the 10h gap is your risk buffer. P95 is the "what if everything slips" number for worst-case comms.

### Step 4 — calibrate with `velocity`

Historical git data tells you whether your estimates have been accurate:

```bash
camshaft velocity --days 90
```

Output includes median commit-to-merge time per category. If your team historically takes 1.6× longer than estimated on `backend` tasks, scale those durations before running risk-analysis.

## Common Patterns

### Pattern: Parallel Subagent Dispatch

```
optimize
→ parse parallel_groups[0]
→ spawn N subagents for those tasks
→ wait for all to finish
→ task complete <each>
→ query ready
→ repeat
```

### Pattern: Scope Negotiation

Optimize shows 40h but the budget is 30h:

1. `camshaft query bottlenecks` — find zero-float tasks.
2. For each, run `camshaft query what-if <id> --duration <smaller>` and read the new `project_duration`.
3. If no single cut gets under budget, `camshaft analyze` often suggests a task worth splitting or descoping.

### Pattern: Baseline + Diff

```bash
camshaft export > baseline.json            # after plan approval
# ...execution + changes...
camshaft diff --baseline baseline.json     # see drift
```

### Pattern: Bulk Plan Load

For large plans, skip the shell loop and load a YAML:

```bash
camshaft bulk --file docs/examples/auth-sprint.yaml
```

(`bulk` loads tasks, deps, milestones, and resources in one pass. It is the scripted equivalent of the per-task invocations in Scenario 1.)

## Command Cheat Sheet

```
init             --name <N> [--mode sprint|roadmap] [--start YYYY-MM-DD] [--force]
add task <id>    --name <N> --duration <H|D> [--priority low|medium|high] [--category <C>]
add dep <from> <to> [--type fs|ss|ff|sf] [--lag <N>]
add milestone <id> --name <N>
add resource <id> --name <N> [--type labor|material|equipment] [--units <N>]
add assign <task> <resource>
remove task|dep  <id...>
validate
optimize
analyze
query critical-path | status | bottlenecks | parallel | suggest-order | ready
query what-if <task> --duration <N>
task complete|reopen|status <id>
sprint plan --capacity <N> --hours-per-day <N>
sprint suggest
sprint overcommit-check --hours-per-day <N>
risk-analysis
evm
velocity [--repo <path>] [--days <N>]
level-resources
diff --baseline <file>
export
import --file <file> [--force]
bulk --file <yaml>            # load tasks/deps/milestones from YAML
```

## JSON Field Reference

### `optimize`
- `project_duration` — total duration (hours in sprint, days in roadmap)
- `critical_path` — ordered task IDs determining the minimum timeline
- `parallel_groups` — `[{group, tasks}]` groups dispatchable concurrently
- `bottlenecks` — zero-float task IDs
- `suggested_order` — priority-aware execution sequence
- `next_ready_tasks` — tasks with all predecessors satisfied
- `total_float` — per-task slack

### `query status`
Each task: `id`, `earliest_start`, `earliest_finish`, `latest_start`, `latest_finish`, `total_float`, `free_float`, `completed`.

### `risk-analysis`
`p50_duration`, `p80_duration`, `p95_duration`, `criticality_index` (per task), `iterations`.

### `analyze`
`health_score` (0–100), `issues[]` with `{severity, task, message}`, `recommendations[]`.

### `evm`
`planned_value`, `earned_value`, `actual_cost`, `spi`, `cpi`, `eac`, `etc`, `vac`.

### `diff`
`added[]`, `removed[]`, `modified[]`, `duration_delta`.

## Integration with AI Agents

### Claude Code
Drop `skill/SKILL.md` into your skills directory. It auto-loads and teaches Claude when to invoke Camshaft.

### Hermes
The same skill file is agentskills.io compatible. No extra setup.

### Generic Shell Agents
Every command emits JSON. Pipe through `jq`:

```bash
camshaft optimize | jq '.parallel_groups[0].tasks[]'
```

## Troubleshooting

- **Plan already exists** — `.camshaft/` from a prior run. Use `--force` on `init` or `rm -rf .camshaft/`.
- **Task not found** — IDs are case-sensitive. `camshaft query status` prints every known ID.
- **Cycle detected** — `camshaft validate` returns the offending edges; remove or reverse one.
- **Optimization empty** — ensure every task has `--duration > 0`. Zero-duration tasks should be milestones.
- **Subcommand name differs** — it is `add dep` (not `dependency`), `risk-analysis` (not `risk`), `query what-if` (not top-level `what-if`).
