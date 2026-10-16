#!/usr/bin/env python3
"""Compare two directories of performance reports (medians across repetitions).

Example: python3 scripts/performance_compare.py target/performance/before target/performance/after
Prints a table, writes optional JSON, and lists regressions as warnings. The exit status is 0
unless an input directory is unusable: timings are never a pass/fail gate.

With one directory (`--summary`) it prints a markdown table of per-workload medians instead.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import perf_report


def fmt(value: float | None) -> str:
    return "n/a" if value is None else f"{value:.3f}"


def load(directory: Path) -> dict:
    try:
        return perf_report.load_runs(directory)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        raise ValueError(f"{directory}: {error}") from error


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path, nargs="?")
    parser.add_argument("--json", type=Path, help="write the comparison rows here")
    parser.add_argument("--summary", action="store_true", help="markdown medians of one directory")
    args = parser.parse_args()
    try:
        if args.summary or args.after is None:
            runs = load(args.before)
            if not runs:
                print("no schema-2 certification reports found", file=sys.stderr)
                return 2
            print(perf_report.markdown_summary(runs))
            return 0
        before, after = load(args.before), load(args.after)
        if not before or not after:
            print("no schema-2 certification reports found in one of the directories", file=sys.stderr)
            return 2
        rows = perf_report.compare(before, after)
    except ValueError as error:
        print(f"invalid performance report set: {error}", file=sys.stderr)
        return 2
    shared = {(r["workload"], r["profile"], r["sync"]) for r in rows}
    for workload, profile, sync in sorted(shared):
        key = (workload, profile, sync)
        print(f"\n== {workload} / {profile} / {sync} (runs: before {len(before[key])},"
              f" after {len(after[key])})")
        for row in (r for r in rows
                    if (r["workload"], r["profile"], r["sync"]) == key):
            delta = "" if row["delta_pct"] is None else f"{row['delta_pct']:+7.1f}%"
            flag = "  REGRESSION?" if row["regression"] else ""
            print(f"  {row['metric']:<26} {fmt(row['before']):>12} -> {fmt(row['after']):>12} {delta}{flag}")
    only = sorted(set(before) ^ set(after))
    if only:
        print("\nnot compared (present on one side only):", ", ".join("/".join(k) for k in only))
    if args.json:
        args.json.write_text(json.dumps(rows, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
