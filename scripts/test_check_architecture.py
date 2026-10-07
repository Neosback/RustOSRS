#!/usr/bin/env python3
"""Tests for the RustOSRS architecture dependency guard."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import check_architecture


class ArchitectureGuardTests(unittest.TestCase):
    def write_manifest(self, root: Path, crate: str, body: str) -> None:
        crate_dir = root / crate
        crate_dir.mkdir(parents=True, exist_ok=True)
        (crate_dir / "Cargo.toml").write_text(body, encoding="utf-8")

    def test_current_workspace_is_legal(self) -> None:
        errors = check_architecture.check_workspace(check_architecture.CRATES)
        self.assertEqual(errors, [])

    def test_production_crate_cannot_depend_on_reference(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_manifest(
                root,
                "osrs-cache",
                """
[package]
name = "osrs-cache"
version = "0.0.0"

[dependencies]
osrs-core = { path = "../osrs-core" }
osrs-reference = { path = "../osrs-reference" }
""",
            )

            errors = check_architecture.check_workspace(root)
            self.assertTrue(any("production crates must never depend" in error for error in errors))

    def test_scene_cannot_depend_on_cache(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_manifest(
                root,
                "osrs-scene",
                """
[package]
name = "osrs-scene"
version = "0.0.0"

[dependencies]
osrs-core = { path = "../osrs-core" }
osrs-cache = { path = "../osrs-cache" }
""",
            )

            errors = check_architecture.check_workspace(root)
            self.assertTrue(any("illegal internal dependencies: osrs-cache" in error for error in errors))

    def test_core_rejects_target_specific_renderer_dependency(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_manifest(
                root,
                "osrs-core",
                """
[package]
name = "osrs-core"
version = "0.0.0"

[target.'cfg(target_os = "windows")'.dependencies]
wgpu = "28"
""",
            )

            errors = check_architecture.check_workspace(root)
            self.assertTrue(any("renderer/UI/cache-transport dependency leakage: wgpu" in error for error in errors))

    def test_reference_may_depend_on_semantic_and_cache_crates(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_manifest(
                root,
                "osrs-reference",
                """
[package]
name = "osrs-reference"
version = "0.0.0"

[dependencies]
osrs-core = { path = "../osrs-core" }
osrs-cache = { path = "../osrs-cache" }
osrs-scene = { path = "../osrs-scene" }
""",
            )

            errors = check_architecture.check_workspace(root)
            self.assertEqual(errors, [])


if __name__ == "__main__":
    unittest.main()
