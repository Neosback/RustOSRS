#!/usr/bin/env python3
"""Enforce RustOSRS workspace dependency boundaries from ADR-0001."""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"

ALLOWED_INTERNAL: dict[str, set[str]] = {
    "osrs-core": set(),
    "osrs-cache": {"osrs-core"},
    "osrs-scene": {"osrs-core"},
    "osrs-render": {"osrs-core", "osrs-scene"},
    "osrs-reference": {"osrs-core", "osrs-cache", "osrs-scene"},
    "osrs-editor": {"osrs-core", "osrs-cache", "osrs-scene", "osrs-render"},
}

DEPENDENCY_SECTIONS = {"dependencies", "dev-dependencies", "build-dependencies"}
CORE_FORBIDDEN = {
    "osrs-cache",
    "osrs-scene",
    "osrs-render",
    "osrs-reference",
    "osrs-editor",
    "wgpu",
    "egui",
    "eframe",
    "rs-cache",
}


def dependency_names(node: object) -> set[str]:
    """Collect dependencies, including target-specific dependency sections."""
    found: set[str] = set()

    if isinstance(node, dict):
        for key, value in node.items():
            if key in DEPENDENCY_SECTIONS and isinstance(value, dict):
                found.update(value.keys())
            found.update(dependency_names(value))
    elif isinstance(node, list):
        for value in node:
            found.update(dependency_names(value))

    return found


def load_manifest(crate: str) -> dict[str, object] | None:
    manifest_path = CRATES / crate / "Cargo.toml"
    if not manifest_path.exists():
        return None

    with manifest_path.open("rb") as handle:
        return tomllib.load(handle)


def main() -> int:
    errors: list[str] = []

    for crate, allowed in ALLOWED_INTERNAL.items():
        manifest = load_manifest(crate)
        if manifest is None:
            continue

        dependencies = dependency_names(manifest)
        internal = {name for name in dependencies if name.startswith("osrs-")}
        illegal = sorted(internal - allowed)
        if illegal:
            errors.append(
                f"{crate}: illegal internal dependencies: {', '.join(illegal)}; "
                f"allowed: {', '.join(sorted(allowed)) or '<none>'}"
            )

        if crate != "osrs-reference" and "osrs-reference" in dependencies:
            errors.append(f"{crate}: production crates must never depend on osrs-reference")

        if crate == "osrs-core":
            forbidden = sorted(dependencies & CORE_FORBIDDEN)
            if forbidden:
                errors.append(
                    "osrs-core: renderer/UI/cache-transport dependency leakage: "
                    + ", ".join(forbidden)
                )

    if errors:
        print("Architecture dependency check failed:", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1

    print("Architecture dependency check passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
