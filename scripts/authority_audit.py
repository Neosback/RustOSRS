#!/usr/bin/env python3
"""Report semantic-authority hotspots without compiling the workspace.

This is an inventory tool, not an acceptance gate. It makes transitional edit/history ownership
visible to contributors and coding agents before CP2 turns stable findings into no-growth guards.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import re
from typing import Iterable

ROOT = Path(__file__).resolve().parents[1]
EDITOR_SRC = ROOT / "crates" / "osrs-editor" / "src"

CHECKPOINT_TOKENS = (
    "commit_checkpointed(",
    "commit_scenario_checkpointed(",
    "commit_appearance_checkpointed(",
    "undo_checkpointed(",
    "redo_checkpointed(",
    "finalize_checkpoint(",
    "restore_checkpoint(",
)

LEGACY_HISTORY_PATTERNS = (
    re.compile(r"\bstep\.(changes|removed|added)\b"),
    re.compile(r"\bstroke\.step\.(changes|removed|added)\b"),
    re.compile(r"\bEditStep\s*\{"),
)

AUTHORITY_WRITE_TOKENS = (
    ".apply_untracked(",
    ".load_appearance(",
    ".ingest_region(",
    ".unload_region(",
)


@dataclass(frozen=True, order=True)
class Hit:
    category: str
    path: str
    line: int
    text: str


def rust_files(root: Path = EDITOR_SRC) -> Iterable[Path]:
    if not root.exists():
        return ()
    return sorted(root.rglob("*.rs"))


def scan(root: Path = ROOT) -> list[Hit]:
    editor = root / "crates" / "osrs-editor" / "src"
    hits: list[Hit] = []
    for path in rust_files(editor):
        relative = path.relative_to(root).as_posix()
        for line_number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            text = raw.strip()
            if any(token in raw for token in CHECKPOINT_TOKENS):
                hits.append(Hit("checkpoint-lifecycle", relative, line_number, text))
            if any(pattern.search(raw) for pattern in LEGACY_HISTORY_PATTERNS):
                hits.append(Hit("legacy-history", relative, line_number, text))
            if any(token in raw for token in AUTHORITY_WRITE_TOKENS):
                hits.append(Hit("lifecycle-write", relative, line_number, text))
    return sorted(hits)


def print_report(hits: list[Hit], category: str | None = None) -> None:
    selected = [hit for hit in hits if category is None or hit.category == category]
    current: str | None = None
    for hit in selected:
        if hit.category != current:
            if current is not None:
                print()
            current = hit.category
            print(f"[{current}]")
        print(f"{hit.path}:{hit.line}: {hit.text}")
    if not selected:
        print("No matching authority hotspots.")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Inventory RustOSRS editor semantic-authority and transitional-history hotspots."
    )
    parser.add_argument(
        "--category",
        choices=("checkpoint-lifecycle", "legacy-history", "lifecycle-write"),
        help="show only one hotspot category",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    print_report(scan(), args.category)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
