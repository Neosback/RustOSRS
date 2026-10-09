#!/usr/bin/env python3
"""Explicit, candidate-only reference regeneration entry point.

This command deliberately does not update checked-in expected outputs or manifests.
It stages oracle output outside reference-fixtures for manual review and an explicit
follow-up source/manifest change.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys

PINNED_MELXIN_DEOB_COMMIT = "1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc"
ADAPTER_MELXIN_DEOB_GOLDEN = "melxin-deob-golden"


def repository_root() -> Path:
    return Path(__file__).resolve().parents[2]


def reject_checked_in_candidate(path: Path, root: Path | None = None) -> Path:
    repo_root = (root or repository_root()).resolve()
    fixture_root = (repo_root / "reference-fixtures").resolve()
    candidate = path.expanduser().resolve(strict=False)
    try:
        candidate.relative_to(fixture_root)
    except ValueError:
        return candidate
    raise ValueError("candidate output may not be written inside reference-fixtures")


def build_melxin_command(
    *, checkout: Path, bcprov: Path, candidate: Path, work_dir: Path | None = None
) -> list[str]:
    harness = repository_root() / "tools" / "deob-harness" / "run.sh"
    command = [
        "sh",
        str(harness),
        "--checkout",
        str(checkout),
        "--expected-commit",
        PINNED_MELXIN_DEOB_COMMIT,
        "--bcprov",
        str(bcprov),
        "--output",
        str(candidate),
    ]
    if work_dir is not None:
        command.extend(["--work-dir", str(work_dir)])
    return command


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Regenerate a reference oracle artifact into a candidate path only. "
            "Checked-in reference-fixtures are never rewritten by this command."
        )
    )
    parser.add_argument("--adapter", choices=[ADAPTER_MELXIN_DEOB_GOLDEN], required=True)
    parser.add_argument("--checkout", type=Path, required=True)
    parser.add_argument("--bcprov", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument(
        "--print-command",
        action="store_true",
        help="print the pinned adapter command without executing it",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(list(sys.argv[1:] if argv is None else argv))
    candidate = reject_checked_in_candidate(args.candidate)
    if candidate.exists():
        raise SystemExit(f"candidate output already exists: {candidate}")

    command = build_melxin_command(
        checkout=args.checkout.expanduser().resolve(strict=False),
        bcprov=args.bcprov.expanduser().resolve(strict=False),
        candidate=candidate,
        work_dir=(
            args.work_dir.expanduser().resolve(strict=False)
            if args.work_dir is not None
            else None
        ),
    )

    if args.print_command:
        print(" ".join(command))
        return 0

    completed = subprocess.run(command, check=False)
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
