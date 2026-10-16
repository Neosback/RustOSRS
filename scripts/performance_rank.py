#!/usr/bin/env python3
"""Generate bottleneck rankings from a certified RustOSRS editor performance baseline."""
from __future__ import annotations

import argparse
from pathlib import Path

import perf_rank


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("--top", type=int, default=5)
    parser.add_argument("--output-prefix", default="baseline-bottlenecks")
    parser.add_argument(
        "--allow-non-target-machine",
        action="store_true",
        help="development only; release analysis should use the authorized M2 baseline",
    )
    args = parser.parse_args()
    if args.top < 1:
        parser.error("--top must be positive")
    manifest, groups = perf_rank.load_certified_baseline(
        args.baseline, allow_non_target_machine=args.allow_non_target_machine
    )
    analysis = perf_rank.analyze(manifest, groups, args.top)
    json_path, md_path = perf_rank.write_analysis(
        args.baseline.resolve(), analysis, args.output_prefix
    )
    print(md_path)
    print(json_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
