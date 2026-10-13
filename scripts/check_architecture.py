#!/usr/bin/env python3
"""Enforce RustOSRS architecture boundaries and editor module size limits."""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"
EDITOR_SOURCE = CRATES / "osrs-editor" / "src"

# Architecture Contract v2 baseline. Legacy files above these limits will move to an explicit
# measured debt allowlist. This temporary commit intentionally reports the exact live offenders so
# the ratchet is based on repository state rather than guessed exceptions.
EDITOR_MAX_RUST_LINES = 500
EDITOR_MAX_RUST_BYTES = 32 * 1024

ALLOWED_INTERNAL: dict[str, set[str]] = {
    "osrs-core": set(),
    # Headless editable-map semantics may reuse only the canonical value vocabulary.
    "osrs-edit": {"osrs-core"},
    "osrs-cache": {"osrs-core"},
    "osrs-scene": {"osrs-core"},
    # 117HD data/settings are reusable renderer inputs and do not depend on renderer/world/editor.
    "osrs-hd": set(),
    "osrs-render": {"osrs-core", "osrs-scene", "osrs-hd"},
    # Runtime world composition/orchestration may consume the reusable decode,
    # semantic-scene, renderer, and HD-data layers, but none of those layers may depend back on it.
    "osrs-world": {"osrs-core", "osrs-cache", "osrs-scene", "osrs-render", "osrs-hd"},
    # Reference infrastructure may exercise the composed world layer for integration parity.
    "osrs-reference": {"osrs-core", "osrs-cache", "osrs-scene", "osrs-world"},
    # The editor is the product composition root and may consume world orchestration directly.
    "osrs-editor": {
        "osrs-core",
        "osrs-edit",
        "osrs-cache",
        "osrs-scene",
        "osrs-render",
        "osrs-world",
        "osrs-hd",
    },
}

DEPENDENCY_SECTIONS = {"dependencies", "dev-dependencies", "build-dependencies"}
CORE_FORBIDDEN = {
    "osrs-edit",
    "osrs-cache",
    "osrs-scene",
    "osrs-hd",
    "osrs-render",
    "osrs-world",
    "osrs-reference",
    "osrs-editor",
    "wgpu",
    "egui",
    "eframe",
    "rs-cache",
}


def dependency_names(node: object) -> set[str]:
    """Collect dependency identities, including aliases and target-specific sections."""
    found: set[str] = set()

    if isinstance(node, dict):
        for key, value in node.items():
            if key in DEPENDENCY_SECTIONS and isinstance(value, dict):
                for dependency_name, dependency_spec in value.items():
                    found.add(dependency_name)
                    if isinstance(dependency_spec, dict):
                        package = dependency_spec.get("package")
                        if isinstance(package, str):
                            found.add(package)
            found.update(dependency_names(value))
    elif isinstance(node, list):
        for value in node:
            found.update(dependency_names(value))

    return found


def load_manifest(crates_root: Path, crate: str) -> dict[str, object] | None:
    """Load a crate manifest from an explicit workspace crate root."""
    manifest_path = crates_root / crate / "Cargo.toml"
    if not manifest_path.exists():
        return None

    with manifest_path.open("rb") as handle:
        return tomllib.load(handle)


def check_workspace(crates_root: Path = CRATES) -> list[str]:
    """Return dependency-boundary violations for the supplied crates directory."""
    errors: list[str] = []

    for crate, allowed in ALLOWED_INTERNAL.items():
        manifest = load_manifest(crates_root, crate)
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

    return errors


def check_editor_module_sizes(
    editor_root: Path = EDITOR_SOURCE,
    *,
    max_lines: int = EDITOR_MAX_RUST_LINES,
    max_bytes: int = EDITOR_MAX_RUST_BYTES,
) -> list[str]:
    """Reject editor Rust modules that are too large for safe whole-file maintenance."""
    if not editor_root.exists():
        return []

    errors: list[str] = []
    for path in sorted(editor_root.rglob("*.rs")):
        data = path.read_bytes()
        line_count = len(data.splitlines())
        relative = path.relative_to(editor_root)

        if line_count > max_lines:
            errors.append(
                f"osrs-editor/{relative}: {line_count} lines exceeds the {max_lines}-line "
                "module cap; split by responsibility before adding more code"
            )
        if len(data) > max_bytes:
            errors.append(
                f"osrs-editor/{relative}: {len(data)} bytes exceeds the {max_bytes}-byte "
                "API-safe module cap; split the module before whole-file tooling becomes fragile"
            )

    return errors


def main() -> int:
    errors = check_workspace()
    errors.extend(check_editor_module_sizes())
    if errors:
        print("Architecture check failed:", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1

    print("Architecture checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())