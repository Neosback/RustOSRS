#!/usr/bin/env python3
"""Report semantic-authority hotspots without compiling the workspace.

The default mode is an inventory report. Check modes enforce already-established ownership rules
without compiling Rust:

- ``--check-gateway`` keeps semantic checkpoint lifecycle APIs inside ``EditorSession`` internals or
  the application ``EditRuntime`` gateway.
- ``--check-legacy-history`` rejects any reintroduction of legacy ``EditStep`` before/after payload
  fields or call-site usage now that CP2 removed that duplicate authority surface.
"""
from __future__ import annotations

import argparse
from collections import Counter
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
    re.compile(r"\bpub\(super\)\s+(changes|removed|added)\s*:"),
)

AUTHORITY_WRITE_TOKENS = (
    ".apply_untracked(",
    ".load_appearance(",
    ".ingest_region(",
    ".unload_region(",
)

GATEWAY_ALLOWED_FILE = "crates/osrs-editor/src/app/edit_runtime.rs"
SESSION_PREFIX = "crates/osrs-editor/src/session/"

# CP2 eliminated the legacy EditStep payload surface. Keep the former scaffold paths explicit with
# a zero ceiling so any reintroduction there, or in any new file, fails the cheap source-level gate.
LEGACY_HISTORY_BASELINE = {
    "crates/osrs-editor/src/app/editing/history.rs": 0,
    "crates/osrs-editor/src/app/editing/tests.rs": 0,
}


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


def gateway_violations(hits: Iterable[Hit]) -> list[Hit]:
    """Return checkpoint-lifecycle uses outside the session/gateway ownership boundary."""
    return [
        hit
        for hit in hits
        if hit.category == "checkpoint-lifecycle"
        and hit.path != GATEWAY_ALLOWED_FILE
        and not hit.path.startswith(SESSION_PREFIX)
    ]


def legacy_history_violations(hits: Iterable[Hit]) -> list[Hit]:
    """Return any legacy-history usage beyond the zero CP2 baseline."""
    legacy = [hit for hit in hits if hit.category == "legacy-history"]
    violations = [hit for hit in legacy if hit.path not in LEGACY_HISTORY_BASELINE]
    counts = Counter(hit.path for hit in legacy)
    for path, ceiling in LEGACY_HISTORY_BASELINE.items():
        excess = counts[path] - ceiling
        if excess <= 0:
            continue
        path_hits = [hit for hit in legacy if hit.path == path]
        violations.extend(path_hits[-excess:])
    return sorted(violations)


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
    parser.add_argument(
        "--check-gateway",
        action="store_true",
        help="fail if checkpoint lifecycle APIs are used outside EditorSession/EditRuntime",
    )
    parser.add_argument(
        "--check-legacy-history",
        action="store_true",
        help="fail if any legacy EditStep payload authority is present",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    hits = scan()
    failed = False
    if args.check_gateway:
        violations = gateway_violations(hits)
        if violations:
            print("Semantic checkpoint lifecycle escaped the EditRuntime gateway:")
            for hit in violations:
                print(f"{hit.path}:{hit.line}: {hit.text}")
            failed = True
        else:
            print("semantic checkpoint gateway: OK")
    if args.check_legacy_history:
        violations = legacy_history_violations(hits)
        if violations:
            print("Legacy EditStep payload authority is forbidden after CP2:")
            for hit in violations:
                print(f"{hit.path}:{hit.line}: {hit.text}")
            failed = True
        else:
            print("legacy history authority: zero")
    if args.check_gateway or args.check_legacy_history:
        return 1 if failed else 0
    print_report(hits, args.category)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
