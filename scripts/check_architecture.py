#!/usr/bin/env python3
"""Enforce RustOSRS architecture boundaries and editor module debt ratchets."""

from __future__ import annotations

import json
import sys
import tomllib
from pathlib import Path
from editor_architecture import check_editor_contracts

ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"
EDITOR_SOURCE = CRATES / "osrs-editor" / "src"
ARCHITECTURE_DEBT_FILE = ROOT / "scripts" / "architecture_debt.json"

# Architecture Contract v3 normal limits. Dedicated test modules receive the larger test-only cap.
# Existing production modules above a normal limit must be listed in architecture_debt.json at an
# exact measured baseline and may never grow beyond that baseline.
EDITOR_MAX_RUST_LINES = 500
EDITOR_MAX_RUST_BYTES = 32 * 1024
EDITOR_TEST_MAX_RUST_LINES = 700
EDITOR_TEST_MAX_RUST_BYTES = 40 * 1024

ALLOWED_INTERNAL: dict[str, set[str]] = {
    "osrs-core": set(),
    # Headless editable-map semantics may reuse only the canonical value vocabulary.
    "osrs-edit": {"osrs-core"},
    "osrs-filestore": set(),
    "osrs-playtest": {"osrs-core"},
    "osrs-cache": {"osrs-core", "osrs-filestore"},
    "osrs-scene": {"osrs-core"},
    # 117HD data/settings are reusable renderer inputs and do not depend on renderer/world/editor.
    "osrs-hd": set(),
    "osrs-render": {"osrs-core", "osrs-scene", "osrs-hd"},
    # Runtime world composition/orchestration may consume the reusable decode,
    # semantic-scene, renderer, and HD-data layers, but none of those layers may depend back on it.
    "osrs-world": {"osrs-core", "osrs-cache", "osrs-scene", "osrs-render", "osrs-hd", "osrs-playtest"},
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
        "osrs-playtest",
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

EDIT_FORBIDDEN = {
    "wgpu", "egui", "eframe", "winit", "rfd", "web-sys", "js-sys",
    "wasm-bindgen", "tokio", "reqwest", "rune-fs", "rs-cache",
}

EditorDebt = dict[str, dict[str, int]]


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

        if crate in {"osrs-edit", "osrs-filestore", "osrs-playtest"}:
            forbidden = sorted(dependencies & EDIT_FORBIDDEN)
            if forbidden:
                errors.append(f"{crate}: UI/GPU/platform dependency leakage: " + ", ".join(forbidden))

        if crate == "osrs-core":
            forbidden = sorted(dependencies & CORE_FORBIDDEN)
            if forbidden:
                errors.append(
                    "osrs-core: renderer/UI/cache-transport dependency leakage: "
                    + ", ".join(forbidden)
                )

    return errors


def is_dedicated_test_module(relative: Path) -> bool:
    """Return whether a Rust source file is a dedicated test-only module."""
    return (
        relative.name in {"test.rs", "tests.rs"}
        or relative.name.endswith("_tests.rs")
        or "tests" in relative.parts
    )


def load_editor_debt(debt_path: Path = ARCHITECTURE_DEBT_FILE) -> EditorDebt:
    """Load and validate the explicit editor module debt baselines."""
    if not debt_path.exists():
        raise ValueError(f"missing debt allowlist: {debt_path}")

    try:
        payload = json.loads(debt_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(f"cannot read debt allowlist {debt_path}: {exc}") from exc

    if not isinstance(payload, dict) or payload.get("version") != 1:
        raise ValueError("architecture debt allowlist must be an object with version = 1")

    modules = payload.get("editor_modules")
    if not isinstance(modules, dict):
        raise ValueError("architecture debt allowlist must contain an editor_modules object")

    debt: EditorDebt = {}
    for relative, baseline in modules.items():
        if not isinstance(relative, str) or not relative or relative.startswith("/"):
            raise ValueError(f"invalid editor debt path: {relative!r}")
        path = Path(relative)
        if ".." in path.parts or path.suffix != ".rs":
            raise ValueError(f"invalid editor debt path: {relative!r}")
        if not isinstance(baseline, dict):
            raise ValueError(f"editor debt entry {relative!r} must be an object")

        max_lines = baseline.get("max_lines")
        max_bytes = baseline.get("max_bytes")
        if (
            not isinstance(max_lines, int)
            or isinstance(max_lines, bool)
            or max_lines <= 0
            or not isinstance(max_bytes, int)
            or isinstance(max_bytes, bool)
            or max_bytes <= 0
        ):
            raise ValueError(
                f"editor debt entry {relative!r} requires positive integer max_lines/max_bytes"
            )

        debt[path.as_posix()] = {"max_lines": max_lines, "max_bytes": max_bytes}

    return debt


def check_editor_module_sizes(
    editor_root: Path = EDITOR_SOURCE,
    *,
    max_lines: int = EDITOR_MAX_RUST_LINES,
    max_bytes: int = EDITOR_MAX_RUST_BYTES,
    test_max_lines: int = EDITOR_TEST_MAX_RUST_LINES,
    test_max_bytes: int = EDITOR_TEST_MAX_RUST_BYTES,
    debt: EditorDebt | None = None,
    debt_path: Path = ARCHITECTURE_DEBT_FILE,
) -> list[str]:
    """Enforce normal module caps and no-growth baselines for explicit legacy debt."""
    if not editor_root.exists():
        return []

    if debt is None:
        try:
            debt = load_editor_debt(debt_path)
        except ValueError as exc:
            return [f"architecture debt: {exc}"]

    errors: list[str] = []
    seen: set[str] = set()

    for path in sorted(editor_root.rglob("*.rs")):
        data = path.read_bytes()
        line_count = len(data.splitlines())
        byte_count = len(data)
        relative = path.relative_to(editor_root)
        key = relative.as_posix()
        seen.add(key)

        if is_dedicated_test_module(relative):
            normal_lines = test_max_lines
            normal_bytes = test_max_bytes
            kind = "test module"
        else:
            normal_lines = max_lines
            normal_bytes = max_bytes
            kind = "production module"

        over_normal = line_count > normal_lines or byte_count > normal_bytes
        baseline = debt.get(key)

        if baseline is None:
            if line_count > normal_lines:
                errors.append(
                    f"osrs-editor/{key}: {line_count} lines exceeds the {normal_lines}-line "
                    f"{kind} cap; split by responsibility before adding more code"
                )
            if byte_count > normal_bytes:
                errors.append(
                    f"osrs-editor/{key}: {byte_count} bytes exceeds the {normal_bytes}-byte "
                    f"{kind} cap; split before whole-file tooling becomes fragile"
                )
            continue

        baseline_lines = baseline["max_lines"]
        baseline_bytes = baseline["max_bytes"]

        if not over_normal:
            errors.append(
                f"osrs-editor/{key}: legacy debt exception is stale because the file is now "
                f"within the normal {normal_lines}-line/{normal_bytes}-byte cap; remove it"
            )
            continue

        if line_count > baseline_lines:
            errors.append(
                f"osrs-editor/{key}: legacy debt grew from {baseline_lines} to {line_count} lines; "
                "an allowlisted file may never exceed its recorded baseline"
            )
        if byte_count > baseline_bytes:
            errors.append(
                f"osrs-editor/{key}: legacy debt grew from {baseline_bytes} to {byte_count} bytes; "
                "an allowlisted file may never exceed its recorded baseline"
            )

    orphaned = sorted(set(debt) - seen)
    for key in orphaned:
        errors.append(
            f"osrs-editor/{key}: debt allowlist entry has no matching Rust module; remove it"
        )

    return errors


def main() -> int:
    errors = check_workspace()
    errors.extend(check_editor_module_sizes())
    errors.extend(check_editor_contracts(ROOT))
    if errors:
        print("Architecture check failed:", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1

    print("Architecture checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
