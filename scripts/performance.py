#!/usr/bin/env python3
"""Repeat optimized editor workloads without changing user settings or visual quality.

Example: python3 scripts/performance.py --output target/performance/baseline --repetitions 3
Reports retain raw samples; no unstable timing threshold is imposed on shared CI.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
WORKLOADS = {
    "stationary": {"settings": {}},
    "static": {"settings": {"animate_objects": False}, "fixed_time": "10"},
    "distance": {"settings": {"draw_distance_tiles": 200, "load_radius": 3}},
    "travel": {"settings": {}, "script": "sleep 8; teleport 3300 3219; sleep 8; teleport 3228 3219"},
    "editing": {"settings": {}, "script": "sleep 8; tool paint; overlay 1; paint 3228 3219; sleep 2; undo; sleep 2; redo"},
    "spline": {"settings": {}, "script": "sleep 8; tool spline; overlay 1; spline 3223,3215 3228,3219 3234,3225; sleep 3; lay; sleep 2; undo"},
}


def command(*args: str) -> str:
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def metadata(binary: Path, workload: str, profile: str, vsync: bool) -> dict:
    hardware = {"platform": platform.platform(), "machine": platform.machine()}
    if sys.platform == "darwin":
        hardware["cpu"] = command("sysctl", "-n", "machdep.cpu.brand_string")
        hardware["physical_memory_bytes"] = int(command("sysctl", "-n", "hw.memsize"))
    return {
        "commit": command("git", "rev-parse", "HEAD"),
        "working_tree_status": command("git", "status", "--porcelain"),
        "tracked_diff_sha256": hashlib.sha256(subprocess.check_output(
            ["git", "diff", "HEAD", "--binary"], cwd=ROOT)).hexdigest(),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "toolchain": command("rustc", "--version"),
        "hardware": hardware,
        "workload": workload, "profile": profile, "vsync": vsync,
        "window_logical_points": [1360, 860],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/osrs-editor")
    parser.add_argument("--cache", type=Path, default=Path.home() / "RustOSRS-cache/b241/cache")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--profiles", nargs="+", choices=["Reference", "Hd"], default=["Reference", "Hd"])
    parser.add_argument("--workloads", nargs="+", choices=list(WORKLOADS), default=["stationary"])
    parser.add_argument("--seconds", type=float, default=30)
    parser.add_argument("--warmup", type=float, default=8)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--reuse", choices=["on", "off"], default="on")
    parser.add_argument("--vsync", action="store_true")
    parser.add_argument("--capture", action="store_true", help="Capture a deterministic static frame per run")
    args = parser.parse_args()
    if args.seconds <= args.warmup or args.warmup < 0 or args.repetitions < 1:
        parser.error("seconds must exceed nonnegative warmup; repetitions must be positive")
    binary = args.binary.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    for workload in args.workloads:
        spec = WORKLOADS[workload]
        for profile in args.profiles:
            for repetition in range(args.repetitions):
                name = f"{workload}-{profile}-{repetition}"
                settings = output / f"{name}.settings.json"
                settings.write_text(json.dumps({"render_profile": profile, **spec["settings"]}))
                meta = metadata(binary, workload, profile, args.vsync)
                meta["viewport_reuse"] = args.reuse
                meta["cache_path"] = str(args.cache.resolve())
                (output / f"{name}.metadata.json").write_text(json.dumps(meta, indent=2))
                env = os.environ.copy()
                # Never inherit unrelated smoke hooks or persist changes to the user's settings.
                env = {key: value for key, value in env.items() if not key.startswith("RUSTOSRS_")}
                env.update({
                    "RUSTOSRS_VIEWPORT_REUSE": "1" if args.reuse == "on" else "0",
                    "RUSTOSRS_SETTINGS": str(settings), "RUSTOSRS_TRACE_EDITS": "0",
                    "RUSTOSRS_EXIT_AFTER_SECONDS": str(args.seconds),
                    "RUSTOSRS_VSYNC": "1" if args.vsync else "0",
                    "RUSTOSRS_WINDOW": "1360,860", "RUSTOSRS_SCRIPT": spec.get("script", ""),
                    "RUSTOSRS_PERF_REPORT": str(output / f"{name}.json"),
                    "RUSTOSRS_PERF_WARMUP_SECONDS": str(args.warmup),
                    "RUSTOSRS_PERF_METADATA": json.dumps(meta),
                })
                if "fixed_time" in spec:
                    env["RUSTOSRS_FIXED_TIME_SECONDS"] = spec["fixed_time"]
                if args.capture:
                    if workload != "static":
                        parser.error("--capture requires the static workload for deterministic visual inputs")
                    env["RUSTOSRS_SCREENSHOT"] = str(output / f"{name}.png")
                    env["RUSTOSRS_SCREENSHOT_FRAME"] = "600"
                with (output / f"{name}.log").open("w") as log:
                    result = subprocess.run([str(binary), str(args.cache.resolve())], cwd=ROOT,
                                            env=env, stdout=log, stderr=log, timeout=args.seconds + 120)
                report_path = output / f"{name}.json"
                if result.returncode or not report_path.exists():
                    print(f"{name}: failed or missing report; see {name}.log", file=sys.stderr)
                    return 1
                if args.capture and not (output / f"{name}.png").exists():
                    print(f"{name}: requested capture did not arrive", file=sys.stderr)
                    return 1
                report = json.loads(report_path.read_text())
                if not report["samples"]:
                    print(f"{name}: no post-warmup samples", file=sys.stderr)
                    return 1
                print(name, json.dumps(report["summary"]), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
