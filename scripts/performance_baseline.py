#!/usr/bin/env python3
"""Capture and certify a complete RustOSRS editor performance baseline.

This orchestration intentionally builds once, freezes that exact executable, then runs every
requested workload/profile in both uncapped and capped modes against the frozen copy. The
resulting directory is only certified when every expected repetition exists and provenance is
consistent across the whole matrix.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import platform
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable

import perf_report
import performance

ROOT = Path(__file__).resolve().parents[1]
SYNC_MODES = (("uncapped", False), ("capped", True))
TARGET_MEMORY_BYTES = 8 * 1024**3
GLOBAL_METADATA = (
    "commit",
    "working_tree_status",
    "tracked_diff_sha256",
    "binary_sha256",
    "toolchain",
    "hardware",
    "cache_path",
    "viewport_reuse",
    "window_logical_points",
    "base_settings",
)


def command(*args: str) -> str:
    try:
        return subprocess.check_output(args, cwd=ROOT, text=True, stderr=subprocess.DEVNULL).strip()
    except (OSError, subprocess.CalledProcessError):
        return ""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def target_machine_problems() -> list[str]:
    """Return why this host is not the authorized M2/8-GB baseline machine."""
    problems: list[str] = []
    if sys.platform != "darwin":
        problems.append(f"platform is {sys.platform!r}, expected 'darwin'")
    machine = platform.machine()
    if machine != "arm64":
        problems.append(f"machine is {machine!r}, expected 'arm64'")
    cpu = command("sysctl", "-n", "machdep.cpu.brand_string")
    if "Apple M2" not in cpu:
        problems.append(f"CPU is {cpu or 'unknown'!r}, expected Apple M2")
    memory_text = command("sysctl", "-n", "hw.memsize")
    memory = int(memory_text) if memory_text.isdigit() else 0
    if memory != TARGET_MEMORY_BYTES:
        problems.append(f"physical memory is {memory} bytes, expected {TARGET_MEMORY_BYTES}")
    return problems


def expected_keys(workloads: Iterable[str], profiles: Iterable[str]) -> set[tuple[str, str, str]]:
    return {
        (workload, profile, sync)
        for workload in workloads
        for profile in profiles
        for sync, _ in SYNC_MODES
    }


def freeze_binary(source: Path, output: Path) -> tuple[Path, str]:
    """Copy one built executable into the evidence directory and verify its bytes."""
    digest = sha256_file(source)
    frozen_dir = output / "frozen"
    frozen_dir.mkdir(parents=True, exist_ok=True)
    destination = frozen_dir / f"osrs-editor-{digest[:16]}{source.suffix}"
    if destination.exists():
        if sha256_file(destination) != digest:
            raise ValueError(f"existing frozen binary has wrong hash: {destination}")
    else:
        shutil.copy2(source, destination)
        destination.chmod(source.stat().st_mode)
    if sha256_file(destination) != digest:
        raise ValueError("frozen binary hash changed during copy")
    return destination, digest


def runner_argv(
    frozen_binary: Path,
    cache: Path,
    output: Path,
    workloads: list[str],
    profiles: list[str],
    repetitions: int,
    cooldown: float,
    reuse: str,
    retries: int,
    vsync: bool,
    capture: bool,
) -> list[str]:
    argv = [
        sys.executable,
        str(ROOT / "scripts/performance.py"),
        "--binary",
        str(frozen_binary),
        "--cache",
        str(cache),
        "--output",
        str(output),
        "--profiles",
        *profiles,
        "--workloads",
        *workloads,
        "--repetitions",
        str(repetitions),
        "--cooldown",
        str(cooldown),
        "--reuse",
        reuse,
        "--retries",
        str(retries),
    ]
    if vsync:
        argv.append("--vsync")
    if capture:
        argv.append("--capture")
    return argv


def _all_reports(groups: dict[tuple[str, str, str], list[dict]]) -> list[dict]:
    return [report for reports in groups.values() for report in reports]


def certify_directory(
    output: Path,
    workloads: list[str],
    profiles: list[str],
    repetitions: int,
    frozen_sha256: str,
    expected_commit: str,
) -> tuple[dict[tuple[str, str, str], list[dict]], dict]:
    """Strictly certify matrix completeness and provenance; return groups and manifest facts."""
    groups = perf_report.load_runs(output)
    expected = expected_keys(workloads, profiles)
    actual = set(groups)
    missing = sorted(expected - actual)
    unexpected = sorted(actual - expected)
    if missing or unexpected:
        details = []
        if missing:
            details.append("missing " + ", ".join("/".join(key) for key in missing))
        if unexpected:
            details.append("unexpected " + ", ".join("/".join(key) for key in unexpected))
        raise ValueError("baseline matrix mismatch: " + "; ".join(details))
    wrong_counts = {key: len(groups[key]) for key in sorted(groups) if len(groups[key]) != repetitions}
    if wrong_counts:
        raise ValueError(f"baseline repetition counts differ from {repetitions}: {wrong_counts}")

    reports = _all_reports(groups)
    if not reports:
        raise ValueError("baseline contains no reports")
    reference = reports[0]["metadata"]
    for report in reports:
        metadata = report["metadata"]
        mismatches = [
            field for field in GLOBAL_METADATA if metadata.get(field) != reference.get(field)
        ]
        if mismatches:
            raise ValueError("baseline has cross-matrix provenance drift: " + ", ".join(mismatches))
        if metadata["commit"] != expected_commit:
            raise ValueError(
                f"report commit {metadata['commit']} differs from expected {expected_commit}"
            )
        if metadata["working_tree_status"]:
            raise ValueError("baseline report was captured from a dirty working tree")
        if metadata["binary_sha256"] != frozen_sha256:
            raise ValueError("baseline report did not use the frozen binary")
        stalls = perf_report.stalls(report)
        if stalls:
            raise ValueError(f"certified report still contains frame stalls: {max(stalls):.1f} ms")
        drift = perf_report.camera_drift(report)
        if drift:
            raise ValueError(f"certified report contains camera drift: {drift}")

    return groups, {
        "commit": reference["commit"],
        "tracked_diff_sha256": reference["tracked_diff_sha256"],
        "binary_sha256": reference["binary_sha256"],
        "toolchain": reference["toolchain"],
        "hardware": reference["hardware"],
        "cache_path": reference["cache_path"],
        "viewport_reuse": reference["viewport_reuse"],
        "window_logical_points": reference["window_logical_points"],
        "base_settings": reference["base_settings"],
        "reports": len(reports),
        "groups": len(groups),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cache", type=Path, default=Path.home() / "RustOSRS-cache/b241/cache")
    parser.add_argument(
        "--profiles", nargs="+", choices=["Reference", "Hd"], default=["Reference", "Hd"]
    )
    parser.add_argument(
        "--workloads",
        nargs="+",
        choices=list(performance.WORKLOADS),
        default=list(performance.WORKLOADS),
    )
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--cooldown", type=float, default=10)
    parser.add_argument("--reuse", choices=["on", "off"], default="on")
    parser.add_argument("--retries", type=int, default=2)
    parser.add_argument("--capture", action="store_true")
    parser.add_argument(
        "--allow-non-target-machine",
        action="store_true",
        help="development escape hatch; a release baseline should never use this",
    )
    args = parser.parse_args()

    if args.repetitions < 1:
        parser.error("repetitions must be positive")
    if args.cooldown < 0:
        parser.error("cooldown must be nonnegative")
    if not args.cache.is_dir():
        parser.error(f"cache directory does not exist: {args.cache}")

    problems = target_machine_problems()
    if problems and not args.allow_non_target_machine:
        parser.error("not the authorized M2/8-GB baseline machine: " + "; ".join(problems))

    commit = command("git", "rev-parse", "HEAD")
    status = command("git", "status", "--porcelain")
    if not commit:
        parser.error("could not resolve Git HEAD")
    if status:
        parser.error("working tree must be clean before a baseline capture")

    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    existing_reports = [
        path
        for path in output.glob("*.json")
        if not path.name.endswith((".metadata.json", ".settings.json"))
    ]
    if existing_reports:
        parser.error(
            f"output already contains performance reports; use a fresh directory: {existing_reports[0]}"
        )

    subprocess.run(
        ["cargo", "build", "--release", "--locked", "-p", "osrs-editor"],
        cwd=ROOT,
        check=True,
    )
    built = ROOT / "target/release/osrs-editor"
    if not built.is_file():
        parser.error(f"release binary was not produced: {built}")
    frozen, frozen_sha = freeze_binary(built, output)

    for sync, vsync in SYNC_MODES:
        print(f"== capturing {sync} matrix against {frozen.name}", flush=True)
        subprocess.run(
            runner_argv(
                frozen,
                args.cache.resolve(),
                output,
                args.workloads,
                args.profiles,
                args.repetitions,
                args.cooldown,
                args.reuse,
                args.retries,
                vsync,
                args.capture,
            ),
            cwd=ROOT,
            check=True,
        )

    end_commit = command("git", "rev-parse", "HEAD")
    end_status = command("git", "status", "--porcelain")
    if end_commit != commit or end_status:
        raise SystemExit("source tree changed during baseline capture; refusing certification")

    groups, facts = certify_directory(
        output,
        args.workloads,
        args.profiles,
        args.repetitions,
        frozen_sha,
        commit,
    )
    summary_name = "baseline-summary.md"
    (output / summary_name).write_text(perf_report.markdown_summary(groups) + "\n")
    manifest = {
        "schema_version": 1,
        "kind": "rustosrs-performance-baseline",
        "certified": True,
        "created_utc": datetime.now(timezone.utc).isoformat(),
        **facts,
        "target_machine_problems": problems,
        "frozen_binary": str(frozen.relative_to(output)),
        "profiles": args.profiles,
        "workloads": args.workloads,
        "sync_modes": [sync for sync, _ in SYNC_MODES],
        "repetitions": args.repetitions,
        "expected_reports": len(expected_keys(args.workloads, args.profiles)) * args.repetitions,
        "summary": summary_name,
    }
    (output / "baseline-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps(manifest, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
