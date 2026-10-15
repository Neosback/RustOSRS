#!/usr/bin/env python3
"""Repeat optimized editor workloads without changing user settings or visual quality.

Example: python3 scripts/performance.py --build --output target/performance/baseline --repetitions 3
Reports (schema 2) retain raw samples; no timing threshold is imposed. On macOS the process
physical footprint is sampled externally with `footprint` (separate from the in-process RSS and
from owned GPU bytes; the three are never added together).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
import time
from pathlib import Path

import perf_report

ROOT = Path(__file__).resolve().parents[1]
# Settings every measured run uses: HUD chrome that redraws every frame is hidden so reports and
# captures are comparable. Visual quality, viewing distance and resolution are untouched.
BASE_SETTINGS = {"stats_overlay": False, "show_minimap": False, "tile_info_hud": False}
SETTLE = "sleep 8"
WORKLOADS = {
    "stationary": {"settings": {}},
    "static": {"settings": {"animate_objects": False}, "fixed_time": "10"},
    "distance": {"settings": {"draw_distance_tiles": 200, "load_radius": 3}},
    # Dense town centre, high angle, then a low grazing view down a street.
    "dense": {"settings": {}, "seconds": 40,
              "script": "teleport 3212 3428 0; pose 20 45 1500; sleep 14; mark street; pose 20 15 400; sleep 14"},
    "water": {"settings": {}, "seconds": 36,
              "script": "teleport 3240 3226 0; pose 0 40 600; sleep 14; mark low; pose 60 20 300; sleep 14"},
    "shadows": {"settings": {}, "seconds": 36,
                "script": "teleport 3222 3218 0; pose 300 35 900; sleep 14; mark low_sun; pose 300 12 300; sleep 14"},
    "interior": {"settings": {}, "seconds": 36,
                 "script": "teleport 3209 3214 0; pose 0 60 600; sleep 14; mark corner; pose 135 40 350; sleep 14"},
    "travel": {"settings": {}, "seconds": 70,
               "script": f"{SETTLE}; mark outbound; teleport 3300 3219; sleep 10; mark far; "
                         "teleport 3228 3219; sleep 10; mark return; teleport 3300 3219; sleep 10; "
                         "mark revisit; teleport 3228 3219; sleep 10"},
    "editing": {"settings": {}, "seconds": 40,
                "script": f"{SETTLE}; tool paint; overlay 1; mark paint; paint 3228 3219; sleep 2; "
                          "mark undo; undo; sleep 2; mark redo; redo; sleep 2; mark settle; sleep 4"},
    "spline": {"settings": {}, "seconds": 40,
               "script": f"{SETTLE}; tool spline; overlay 1; mark preview; "
                         "spline 3223,3215 3228,3219 3234,3225; sleep 3; mark lay; lay; sleep 2; mark undo; undo; sleep 3"},
}


def command(*args: str) -> str:
    try:
        return subprocess.check_output(args, cwd=ROOT, text=True, stderr=subprocess.DEVNULL).strip()
    except (OSError, subprocess.CalledProcessError):
        return ""


def machine_state() -> dict:
    """Thermal, power and display facts that explain run-to-run differences."""
    state: dict = {}
    if sys.platform == "darwin":
        state["thermal"] = command("pmset", "-g", "therm")
        state["power"] = command("pmset", "-g", "batt").splitlines()[:2]
        display = command("system_profiler", "SPDisplaysDataType")
        state["display"] = [line.strip() for line in display.splitlines()
                            if re.search(r"Resolution|Refresh|Main Display|UI Looks like|Chipset", line)]
    return state


def metadata(binary: Path, workload: str, profile: str, vsync: bool) -> dict:
    hardware = {"platform": platform.platform(), "machine": platform.machine()}
    if sys.platform == "darwin":
        hardware["cpu"] = command("sysctl", "-n", "machdep.cpu.brand_string")
        hardware["physical_memory_bytes"] = int(command("sysctl", "-n", "hw.memsize") or 0)
    return {
        "commit": command("git", "rev-parse", "HEAD"),
        "working_tree_status": command("git", "status", "--porcelain"),
        "tracked_diff_sha256": hashlib.sha256(subprocess.check_output(
            ["git", "diff", "HEAD", "--binary"], cwd=ROOT)).hexdigest(),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "toolchain": command("rustc", "--version"),
        "hardware": hardware, "machine_state": machine_state(),
        "workload": workload, "profile": profile, "vsync": vsync,
        "window_logical_points": [1360, 860],
        "base_settings": BASE_SETTINGS,
    }


def footprint_mib(pid: int) -> float | None:
    """Physical footprint of a running process via the macOS `footprint` tool (None elsewhere)."""
    if sys.platform != "darwin":
        return None
    match = re.search(r"Footprint:\s+([\d.]+)\s*(KB|MB|GB)", command("footprint", "-p", str(pid)))
    if not match:
        return None
    return float(match.group(1)) * {"KB": 1 / 1024, "MB": 1.0, "GB": 1024.0}[match.group(2)]


def run_process(argv: list[str], env: dict, log, timeout: float, interval: float = 2.0):
    """Run the editor, sampling its footprint every `interval` seconds. Returns (code, samples)."""
    # `caffeinate` keeps the display and system awake for the life of the editor so an unattended
    # window is not throttled or slept mid-run (observed as multi-second frame gaps otherwise).
    if sys.platform == "darwin" and Path("/usr/bin/caffeinate").exists():
        argv = ["/usr/bin/caffeinate", "-dimsu", *argv]
    process = subprocess.Popen(argv, cwd=ROOT, env=env, stdout=log, stderr=log)
    samples: list[dict] = []
    started = time.monotonic()
    while process.poll() is None:
        elapsed = time.monotonic() - started
        if elapsed > timeout:
            process.kill()
            process.wait()
            return -9, samples
        mib = footprint_mib(process.pid)
        if mib is not None:
            samples.append({"seconds": round(elapsed, 2), "mib": mib})
        time.sleep(interval)
    return process.returncode, samples


def attach_external(report_path: Path, samples: list[dict], repetition_notes: dict) -> None:
    report = json.loads(report_path.read_text())
    values = [s["mib"] for s in samples]
    report["external"] = {
        "footprint_mib": {"samples": samples, "max": max(values) if values else None,
                          "unavailable_reason": None if values else "footprint tool unavailable on this platform"},
        **repetition_notes,
    }
    report_path.write_text(json.dumps(report))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/osrs-editor")
    parser.add_argument("--cache", type=Path, default=Path.home() / "RustOSRS-cache/b241/cache")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--profiles", nargs="+", choices=["Reference", "Hd"], default=["Reference", "Hd"])
    parser.add_argument("--workloads", nargs="+", choices=list(WORKLOADS), default=["stationary"])
    parser.add_argument("--seconds", type=float, default=30, help="default run length when a workload sets none")
    parser.add_argument("--warmup", type=float, default=8)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--cooldown", type=float, default=10, help="seconds idle between runs")
    parser.add_argument("--reuse", choices=["on", "off"], default="on")
    parser.add_argument("--vsync", action="store_true", help="capped (vsync) instead of uncapped runs")
    parser.add_argument("--retries", type=int, default=2,
                        help="repeat a run that failed or contained a >1 s frame stall this many times")
    parser.add_argument("--build", action="store_true", help="cargo build --release --locked first")
    parser.add_argument("--capture", action="store_true", help="Capture one window screenshot per run")
    parser.add_argument("--capture-frame", type=int, default=600,
                        help="frame number to capture; only the static workload is deterministic")
    args = parser.parse_args()
    if args.warmup < 0 or args.repetitions < 1:
        parser.error("warmup must be nonnegative; repetitions must be positive")
    if args.build:
        subprocess.run(["cargo", "build", "--release", "--locked", "-p", "osrs-editor"], cwd=ROOT, check=True)
    binary = args.binary.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    first = True
    failures: list[str] = []
    for workload in args.workloads:
        spec = WORKLOADS[workload]
        seconds = float(spec.get("seconds", args.seconds))
        if seconds <= args.warmup:
            parser.error(f"{workload}: seconds ({seconds}) must exceed warmup ({args.warmup})")
        for profile in args.profiles:
            for repetition in range(args.repetitions):
                if not first and args.cooldown > 0:
                    time.sleep(args.cooldown)
                first = False
                name = f"{workload}-{profile}-{repetition}"
                settings = output / f"{name}.settings.json"
                settings.write_text(json.dumps({"render_profile": profile, **BASE_SETTINGS, **spec["settings"]}))
                meta = metadata(binary, workload, profile, args.vsync)
                meta["viewport_reuse"] = args.reuse
                meta["cache_path"] = str(args.cache.resolve())
                meta["run_seconds"] = seconds
                (output / f"{name}.metadata.json").write_text(json.dumps(meta, indent=2))
                # Never inherit unrelated smoke hooks or persist changes to the user's settings.
                env = {key: value for key, value in os.environ.items() if not key.startswith("RUSTOSRS_")}
                env.update({
                    "RUSTOSRS_VIEWPORT_REUSE": "1" if args.reuse == "on" else "0",
                    "RUSTOSRS_SETTINGS": str(settings), "RUSTOSRS_TRACE_EDITS": "0",
                    "RUSTOSRS_EXIT_AFTER_SECONDS": str(seconds),
                    "RUSTOSRS_VSYNC": "1" if args.vsync else "0",
                    "RUSTOSRS_WINDOW": "1360,860", "RUSTOSRS_SCRIPT": spec.get("script", ""),
                    "RUSTOSRS_PERF_REPORT": str(output / f"{name}.json"),
                    "RUSTOSRS_PERF_WARMUP_SECONDS": str(args.warmup),
                    "RUSTOSRS_PERF_METADATA": json.dumps(meta),
                })
                if "fixed_time" in spec:
                    env["RUSTOSRS_FIXED_TIME_SECONDS"] = spec["fixed_time"]
                if args.capture:
                    env["RUSTOSRS_SCREENSHOT"] = str(output / f"{name}.png")
                    env["RUSTOSRS_SCREENSHOT_FRAME"] = str(args.capture_frame)
                outcome = None
                for attempt in range(args.retries + 1):
                    report_path = output / f"{name}.json"
                    report_path.unlink(missing_ok=True)
                    with (output / f"{name}.log").open("w") as log:
                        code, footprint = run_process(
                            [str(binary), str(args.cache.resolve())], env, log, seconds + 120)
                    if code or not report_path.exists():
                        outcome = f"failed (exit {code}); see {name}.log"
                        continue
                    attach_external(report_path, footprint, {"thermal_after": machine_state().get("thermal")})
                    report = json.loads(report_path.read_text())
                    problems = perf_report.validate(report)
                    if problems:
                        outcome = f"invalid report: {'; '.join(problems)}"
                        continue
                    gaps = perf_report.stalls(report)
                    if gaps:
                        outcome = f"contaminated by {len(gaps)} stall(s), longest {max(gaps) / 1000:.1f} s"
                        report_path.rename(output / f"{name}.stalled-{attempt}.json.txt")
                        continue
                    outcome = None
                    break
                if outcome is not None:
                    print(f"{name}: giving up after {args.retries + 1} attempts: {outcome}", file=sys.stderr)
                    failures.append(name)
                    continue
                if args.capture and not (output / f"{name}.png").exists():
                    print(f"{name}: requested capture did not arrive", file=sys.stderr)
                    failures.append(name)
                    continue
                drift = perf_report.camera_drift(report)
                note = f" camera segments changed {drift}x" if drift else ""
                retried = f" (attempt {attempt + 1})" if attempt else ""
                print(name, json.dumps(report["summary"]["frame_interval_ms"]), note, retried, flush=True)
    if failures:
        print("runs without a clean report:", ", ".join(failures), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
