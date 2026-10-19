#!/usr/bin/env python3
"""Generate or verify the human-readable feature index from the canonical registry."""
from __future__ import annotations

import argparse
from pathlib import Path
import sys
from typing import Any

from feature_registry import ROOT, RegistryError, load_registry

OUTPUT_PATH = ROOT / "docs" / "architecture" / "FEATURE-INDEX.md"
REFERENCE_ROLES = ("controller", "planner", "gateway")
TEST_ROLES = ("planning_test", "cancellation_test", "commit_test", "undo_test")


def reference(value: Any) -> str:
    if not isinstance(value, dict):
        return "-"
    file = value.get("file")
    symbol = value.get("symbol")
    if not isinstance(file, str) or not isinstance(symbol, str):
        return "-"
    return f"`{file}::{symbol}`"


def owner(feature: dict[str, Any]) -> str:
    value = feature["owner"]
    return f"`{value['crate']}::{value['area']}`"


def render(features: dict[str, dict[str, Any]]) -> str:
    lines = [
        "# Generated feature index",
        "",
        "<!-- GENERATED FILE: edit scripts/editor_features.json, not this file. -->",
        "",
        "Canonical source: `scripts/editor_features.json`. Regenerate with ",
        "`python3 scripts/feature_index.py --write`.",
        "",
        "Use `python3 scripts/features.py find \"<concept>\"` before adding a feature, planner, ",
        "gateway, or history path.",
        "",
        "| ID | Feature | Status | Owner | Kind | Mutation |",
        "|---|---|---|---|---|---|",
    ]
    for name, feature in sorted(features.items(), key=lambda item: item[1]["id"]):
        kind = feature.get("kind", "tool")
        mutation = "mutating" if feature.get("mutates_document") else "read-only"
        lines.append(
            f"| `{feature['id']}` | {name} | {feature['status']} | {owner(feature)} | {kind} | {mutation} |"
        )

    lines.extend(["", "## Navigation", ""])
    for name, feature in sorted(features.items(), key=lambda item: item[1]["id"]):
        lines.extend(
            [
                f"### {name}",
                "",
                f"- **ID:** `{feature['id']}`",
                f"- **Aliases:** {', '.join(f'`{alias}`' for alias in feature['aliases'])}",
                f"- **Owner:** {owner(feature)}",
                f"- **Status:** `{feature['status']}`",
            ]
        )
        for role in REFERENCE_ROLES:
            lines.append(f"- **{role.replace('_', ' ').title()}:** {reference(feature.get(role))}")
        if feature.get("mutates_document"):
            tests = ", ".join(reference(feature.get(role)) for role in TEST_ROLES)
            lines.append(f"- **Behavior tests:** {tests}")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true", help="regenerate FEATURE-INDEX.md")
    mode.add_argument("--check", action="store_true", help="fail when FEATURE-INDEX.md has drifted")
    args = parser.parse_args(argv)

    try:
        expected = render(load_registry())
    except RegistryError as exc:
        print(f"feature registry error: {exc}", file=sys.stderr)
        return 2

    if args.write:
        OUTPUT_PATH.write_text(expected, encoding="utf-8")
        print(f"wrote {OUTPUT_PATH.relative_to(ROOT)}")
        return 0

    try:
        actual = OUTPUT_PATH.read_text(encoding="utf-8")
    except OSError as exc:
        print(f"feature index missing: {exc}", file=sys.stderr)
        return 1
    if actual != expected:
        print(
            "FEATURE-INDEX.md is stale; run python3 scripts/feature_index.py --write",
            file=sys.stderr,
        )
        return 1
    print("feature index: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
