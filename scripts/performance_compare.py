#!/usr/bin/env python3
"""Compare two directories of performance reports (medians across repetitions).

Example: python3 scripts/performance_compare.py target/performance/before target/performance/after
Prints a table, writes optional JSON, and lists regressions as warnings. The exit status is 0
unless an input directory is unusable: timings are never a pass/fail gate.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import perf_report


def fmt(value: float | None) -> str:
    return "n/a" if value is None else f"{value:.3f}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("--json", type=Path, help="write the comparison rows here")
    args = parser.parse_args()
    before, after = perf_report.load_runs(args.before), perf_report.load_runs(args.after)
    if not before or not after:
        print("no schema-2 reports found in one of the directories", file=sys.stderr)
        return 2
    rows = perf_report.compare(before, after)
    shared = {(r["workload"], r["profile"]) for r in rows}
    for workload, profile in sorted(shared):
        print(f"\n== {workload} / {profile} (runs: before {len(before[(workload, profile)])},"
              f" after {len(after[(workload, profile)])})")
        for row in (r for r in rows if (r["workload"], r["profile"]) == (workload, profile)):
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
