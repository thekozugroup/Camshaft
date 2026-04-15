#!/usr/bin/env python3
"""
Camshaft plan visualizer.

Reads a Camshaft plan JSON (from stdin, a --file argument, or the default
.camshaft/plan.json) and emits one of three visual representations:

  * mermaid  -- Mermaid gantt chart syntax (default)
  * ascii    -- Plain-text Gantt chart with critical-path markers
  * summary  -- Human-readable text summary

If the activities do not carry pre-computed CPM dates (early_start /
early_finish) the script reconstructs them from the dependency graph using a
topological forward pass.

Only the Python standard library is used. Python 3.9+.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict, deque
from typing import Any, Dict, Iterable, List, Optional, Sequence, Tuple


# --------------------------------------------------------------------------- #
# Data model helpers
# --------------------------------------------------------------------------- #

DEFAULT_PLAN_PATH = ".camshaft/plan.json"

# Dependency types -> (predecessor_anchor, successor_anchor)
# Anchors: "start" or "finish".  The successor's start (or finish) must be at
# least (predecessor anchor + lag) time units.
_DEP_ANCHORS: Dict[str, Tuple[str, str]] = {
    "FinishToStart": ("finish", "start"),
    "StartToStart": ("start", "start"),
    "FinishToFinish": ("finish", "finish"),
    "StartToFinish": ("start", "finish"),
}


def _duration_days(activity: Dict[str, Any]) -> float:
    """Extract duration (in days) from an activity record."""
    dur = activity.get("original_duration") or activity.get("remaining_duration") or {}
    if isinstance(dur, dict):
        if "days" in dur and dur["days"] is not None:
            return float(dur["days"])
        if "hours" in dur and dur["hours"] is not None:
            return float(dur["hours"]) / 24.0
        if "minutes" in dur and dur["minutes"] is not None:
            return float(dur["minutes"]) / (24.0 * 60.0)
    if isinstance(dur, (int, float)):
        return float(dur)
    return 0.0


def _load_plan(source: Optional[str]) -> Dict[str, Any]:
    """Load plan JSON from a file path or stdin."""
    if source is None or source == "-":
        raw = sys.stdin.read()
        if not raw.strip():
            raise ValueError("No plan data received on stdin.")
        return json.loads(raw)
    try:
        with open(source, "r", encoding="utf-8") as fh:
            return json.load(fh)
    except FileNotFoundError as exc:
        raise FileNotFoundError(
            f"Plan file not found: {source!r}. "
            f"Pass --file or pipe JSON via stdin."
        ) from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"Malformed JSON in {source!r}: {exc.msg} (line {exc.lineno})") from exc


def _project(plan: Dict[str, Any]) -> Dict[str, Any]:
    proj = plan.get("project")
    if not isinstance(proj, dict):
        raise ValueError("Plan is missing the 'project' object.")
    return proj


def _activities(plan: Dict[str, Any]) -> Dict[str, Dict[str, Any]]:
    acts = _project(plan).get("activities")
    if not isinstance(acts, dict):
        raise ValueError("Plan is missing project.activities.")
    return acts


def _dependencies(plan: Dict[str, Any]) -> List[Dict[str, Any]]:
    deps = _project(plan).get("dependencies") or []
    if not isinstance(deps, list):
        raise ValueError("project.dependencies must be a list.")
    return deps


# --------------------------------------------------------------------------- #
# CPM forward pass
# --------------------------------------------------------------------------- #


def _topological_order(
    activities: Dict[str, Dict[str, Any]],
    dependencies: Sequence[Dict[str, Any]],
) -> List[str]:
    """Kahn's algorithm; raises ValueError if the graph has a cycle."""
    indegree: Dict[str, int] = {aid: 0 for aid in activities}
    succ: Dict[str, List[str]] = defaultdict(list)

    for dep in dependencies:
        pred = dep.get("predecessor_id")
        succ_id = dep.get("successor_id")
        if pred not in activities or succ_id not in activities:
            # Unknown activity ids: silently skip -- don't break the whole chart.
            continue
        succ[pred].append(succ_id)
        indegree[succ_id] += 1

    queue: deque[str] = deque(sorted(aid for aid, deg in indegree.items() if deg == 0))
    ordered: List[str] = []

    while queue:
        node = queue.popleft()
        ordered.append(node)
        for nxt in succ[node]:
            indegree[nxt] -= 1
            if indegree[nxt] == 0:
                queue.append(nxt)

    if len(ordered) != len(activities):
        remaining = [aid for aid, deg in indegree.items() if deg > 0]
        raise ValueError(
            "Dependency graph contains a cycle involving: "
            + ", ".join(sorted(remaining))
        )
    return ordered


def _ensure_cpm(
    activities: Dict[str, Dict[str, Any]],
    dependencies: Sequence[Dict[str, Any]],
) -> Dict[str, Tuple[float, float]]:
    """
    Return a mapping of activity_id -> (early_start, early_finish) in days.

    Uses pre-populated early_start / early_finish when available; otherwise
    performs a forward pass honoring dependency types and lags.
    """
    schedule: Dict[str, Tuple[float, float]] = {}
    need_compute = False

    for aid, act in activities.items():
        es = act.get("early_start")
        ef = act.get("early_finish")
        if isinstance(es, (int, float)) and isinstance(ef, (int, float)):
            schedule[aid] = (float(es), float(ef))
        else:
            need_compute = True
            break

    if not need_compute and len(schedule) == len(activities):
        return schedule

    # Forward pass.
    order = _topological_order(activities, dependencies)
    preds: Dict[str, List[Dict[str, Any]]] = defaultdict(list)
    for dep in dependencies:
        if dep.get("predecessor_id") in activities and dep.get("successor_id") in activities:
            preds[dep["successor_id"]].append(dep)

    schedule = {}
    for aid in order:
        duration = _duration_days(activities[aid])
        if not preds[aid]:
            es = 0.0
        else:
            candidates: List[float] = []
            for dep in preds[aid]:
                p_es, p_ef = schedule.get(dep["predecessor_id"], (0.0, 0.0))
                dep_type = dep.get("dependency_type", "FinishToStart")
                pred_anchor, succ_anchor = _DEP_ANCHORS.get(
                    dep_type, _DEP_ANCHORS["FinishToStart"]
                )
                lag = float(dep.get("lag_days") or 0.0)
                anchor_val = p_ef if pred_anchor == "finish" else p_es
                # anchor_val is when the successor's anchor must be >= to.
                if succ_anchor == "start":
                    candidates.append(anchor_val + lag)
                else:  # succ_anchor == "finish"
                    candidates.append(anchor_val + lag - duration)
            es = max([0.0] + candidates)
        ef = es + duration
        schedule[aid] = (es, ef)

    return schedule


# --------------------------------------------------------------------------- #
# Critical path
# --------------------------------------------------------------------------- #


def _critical_path(
    activities: Dict[str, Dict[str, Any]],
    dependencies: Sequence[Dict[str, Any]],
    schedule: Dict[str, Tuple[float, float]],
) -> List[str]:
    """
    Return a single longest path through the dependency DAG by early_finish.

    If every early_finish is 0 (degenerate) an empty list is returned.
    """
    if not schedule:
        return []

    preds: Dict[str, List[str]] = defaultdict(list)
    for dep in dependencies:
        if dep.get("predecessor_id") in activities and dep.get("successor_id") in activities:
            preds[dep["successor_id"]].append(dep["predecessor_id"])

    # Terminal activity = largest early_finish (ties broken alphabetically).
    end_id = max(schedule, key=lambda a: (schedule[a][1], -ord(a[0]) if a else 0))
    total = schedule[end_id][1]
    if total <= 0:
        return []

    path: List[str] = [end_id]
    current = end_id
    while preds[current]:
        # Walk back to whichever predecessor finishes latest (that's the
        # binding constraint on the current activity's early start).
        best = max(preds[current], key=lambda p: schedule.get(p, (0.0, 0.0))[1])
        path.append(best)
        current = best
    path.reverse()
    return path


# --------------------------------------------------------------------------- #
# Renderers
# --------------------------------------------------------------------------- #


def _sorted_activities(
    activities: Dict[str, Dict[str, Any]],
    schedule: Dict[str, Tuple[float, float]],
) -> List[str]:
    return sorted(
        activities.keys(),
        key=lambda aid: (schedule[aid][0], schedule[aid][1], aid),
    )


def render_mermaid(plan: Dict[str, Any]) -> str:
    activities = _activities(plan)
    dependencies = _dependencies(plan)
    schedule = _ensure_cpm(activities, dependencies)

    project_name = _project(plan).get("name") or "Camshaft Plan"
    mode = plan.get("mode") or "sprint"

    lines: List[str] = [
        "gantt",
        f"    title {project_name}",
        "    dateFormat  X",
        "    axisFormat  %s",
        "",
        f"    section {str(mode).capitalize()}",
    ]

    # Assign short mermaid-safe tags ("a1", "a2", ...) keyed by activity id so
    # that "after" references are stable.
    tags: Dict[str, str] = {}
    for idx, aid in enumerate(_sorted_activities(activities, schedule), start=1):
        tags[aid] = f"a{idx}"

    preds: Dict[str, List[Dict[str, Any]]] = defaultdict(list)
    for dep in dependencies:
        if dep.get("predecessor_id") in activities and dep.get("successor_id") in activities:
            preds[dep["successor_id"]].append(dep)

    for aid in _sorted_activities(activities, schedule):
        act = activities[aid]
        name = act.get("name") or aid
        tag = tags[aid]
        es, ef = schedule[aid]
        duration = max(ef - es, 0.0)
        dur_str = _fmt_num(duration)

        task_preds = [p for p in preds[aid] if p.get("predecessor_id") in tags]
        if task_preds:
            after = " ".join(tags[p["predecessor_id"]] for p in task_preds)
            lines.append(f"    {name} :{tag}, after {after}, {dur_str}")
        else:
            lines.append(f"    {name} :{tag}, {_fmt_num(es)}, {dur_str}")

    return "\n".join(lines) + "\n"


def render_ascii(plan: Dict[str, Any], width: int = 60) -> str:
    activities = _activities(plan)
    dependencies = _dependencies(plan)
    schedule = _ensure_cpm(activities, dependencies)
    critical = set(_critical_path(activities, dependencies, schedule))

    if not activities:
        return "(no activities)\n"

    total = max((ef for _, ef in schedule.values()), default=0.0)
    if total <= 0:
        total = 1.0
    scale = width / total

    label_width = max(len(aid) for aid in activities)
    label_width = min(max(label_width, 8), 32)

    lines: List[str] = []
    project_name = _project(plan).get("name") or "Camshaft Plan"
    lines.append(f"Project: {project_name}")
    lines.append(f"Total duration: {_fmt_num(total)} days  (scale: {width} cols)")
    lines.append("")

    for aid in _sorted_activities(activities, schedule):
        es, ef = schedule[aid]
        start_col = int(round(es * scale))
        end_col = int(round(ef * scale))
        if end_col <= start_col and ef > es:
            end_col = start_col + 1
        bar_len = max(end_col - start_col, 0)

        bar_chars = ["░"] * width
        for i in range(start_col, min(start_col + bar_len, width)):
            bar_chars[i] = "█"
        bar = "".join(bar_chars)

        mark = " *" if aid in critical else "  "
        label = aid[:label_width].ljust(label_width)
        lines.append(
            f"{label} | {bar} | {_fmt_num(es)}-{_fmt_num(ef)}{mark}"
        )

    lines.append("")
    lines.append("Legend: █ active  ░ idle  * critical path")
    return "\n".join(lines) + "\n"


def render_summary(plan: Dict[str, Any]) -> str:
    activities = _activities(plan)
    dependencies = _dependencies(plan)
    schedule = _ensure_cpm(activities, dependencies)
    critical = _critical_path(activities, dependencies, schedule)

    project = _project(plan)
    meta = plan.get("meta") or {}
    groups = meta.get("parallelism_groups") or []

    total = max((ef for _, ef in schedule.values()), default=0.0)
    sequential = sum(_duration_days(a) for a in activities.values())

    lines: List[str] = []
    lines.append("=" * 60)
    lines.append(f"Camshaft Plan: {project.get('name') or project.get('id') or '(unnamed)'}")
    lines.append("=" * 60)
    lines.append(f"Mode:        {plan.get('mode', 'sprint')}")
    lines.append(f"Format:      {plan.get('format', 'camshaft')} v{plan.get('version', '?')}")
    if plan.get("created_at"):
        lines.append(f"Created:     {plan['created_at']}")
    lines.append(f"Activities:  {len(activities)}")
    lines.append(f"Dependencies:{len(dependencies):>4}")
    lines.append("")
    lines.append(f"Total duration (critical path): {_fmt_num(total)} days")
    lines.append(f"Sequential duration (sum):      {_fmt_num(sequential)} days")
    if total > 0:
        speedup = sequential / total
        lines.append(f"Parallel speedup:               {speedup:.2f}x")
    lines.append("")

    lines.append("Critical path:")
    if critical:
        for aid in critical:
            act = activities[aid]
            es, ef = schedule[aid]
            lines.append(
                f"  -> {aid}  [{_fmt_num(es)}-{_fmt_num(ef)}]  {act.get('name') or ''}"
            )
    else:
        lines.append("  (none)")
    lines.append("")

    lines.append("Parallelism groups:")
    if groups:
        for i, group in enumerate(groups, start=1):
            if not isinstance(group, list):
                continue
            label = ", ".join(str(g) for g in group) if group else "(empty)"
            lines.append(f"  wave {i}: {label}")
    else:
        lines.append("  (none provided)")
    lines.append("")

    lines.append("Activities (by early start):")
    for aid in _sorted_activities(activities, schedule):
        es, ef = schedule[aid]
        name = activities[aid].get("name") or ""
        marker = "*" if aid in set(critical) else " "
        lines.append(
            f"  {marker} {aid:<24} {name:<32} "
            f"start={_fmt_num(es):>6}  finish={_fmt_num(ef):>6}"
        )

    return "\n".join(lines) + "\n"


# --------------------------------------------------------------------------- #
# Misc
# --------------------------------------------------------------------------- #


def _fmt_num(value: float) -> str:
    if value is None:
        return "0"
    if abs(value - round(value)) < 1e-9:
        return str(int(round(value)))
    return f"{value:.2f}".rstrip("0").rstrip(".")


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="visualize.py",
        description=(
            "Render a Camshaft plan JSON as a Mermaid gantt chart, ASCII "
            "gantt chart, or a text summary."
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  python3 visualize.py\n"
            "  python3 visualize.py --file plan.json --format ascii\n"
            "  cat plan.json | python3 visualize.py --file - --format summary\n"
        ),
    )
    parser.add_argument(
        "--file",
        "-f",
        default=DEFAULT_PLAN_PATH,
        help=(
            f"Path to plan JSON (default: {DEFAULT_PLAN_PATH}). "
            "Use '-' to read from stdin."
        ),
    )
    parser.add_argument(
        "--format",
        choices=("mermaid", "ascii", "summary"),
        default="mermaid",
        help="Output format (default: mermaid).",
    )
    parser.add_argument(
        "--width",
        type=int,
        default=60,
        help="Column width for the ASCII gantt chart (default: 60).",
    )
    return parser


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)

    # If stdin has data and --file was left at its default, read stdin.
    source: Optional[str] = args.file
    if args.file == DEFAULT_PLAN_PATH and not sys.stdin.isatty():
        source = "-"

    try:
        plan = _load_plan(source)
    except (FileNotFoundError, ValueError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    try:
        if args.format == "mermaid":
            sys.stdout.write(render_mermaid(plan))
        elif args.format == "ascii":
            sys.stdout.write(render_ascii(plan, width=max(10, args.width)))
        else:
            sys.stdout.write(render_summary(plan))
    except ValueError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    except KeyError as exc:
        print(f"error: missing field {exc}", file=sys.stderr)
        return 2

    return 0


if __name__ == "__main__":
    sys.exit(main())
