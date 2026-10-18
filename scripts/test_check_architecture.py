#!/usr/bin/env python3
"""Tests for the RustOSRS architecture dependency and debt-ratchet guard."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import check_architecture


class ArchitectureGuardTests(unittest.TestCase):
    def test_lab_rejects_editor_and_gpu_dependencies(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            self.write_manifest(root,"osrs-lab",'[dependencies]\nosrs-editor = "0.0.0"\nwgpu = "30"\n')
            errors=check_architecture.check_workspace(root)
            self.assertTrue(any("illegal internal dependencies: osrs-editor" in e for e in errors))
            self.assertTrue(any("UI/GPU/platform dependency leakage: wgpu" in e for e in errors))

    def test_filestore_boundary_is_enforced(self) -> None:
        for owner, dependency in [
            ("osrs-filestore", "osrs-core"),
            ("osrs-filestore", "osrs-cache"),
            ("osrs-editor", "osrs-filestore"),
            ("osrs-edit", "osrs-filestore"),
        ]:
            with (
                self.subTest(owner=owner, dependency=dependency),
                tempfile.TemporaryDirectory() as directory,
            ):
                root = Path(directory)
                self.write_manifest(root, owner, f'[dependencies]\n{dependency} = "0.0.0"\n')
                errors = check_architecture.check_workspace(root)
                self.assertTrue(any(
                    f"illegal internal dependencies: {dependency}" in error for error in errors
                ))

    def test_filestore_rejects_ui_and_gpu_dependencies(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_manifest(root, "osrs-filestore", '[dependencies]\nwgpu = "30"\negui = "0.36"\n')
            errors = check_architecture.check_workspace(root)
            self.assertTrue(any("osrs-filestore: UI/GPU/platform dependency leakage: egui, wgpu" in error for error in errors))

    def write_manifest(self, root: Path, crate: str, body: str) -> None:
        crate_dir = root / crate
        crate_dir.mkdir(parents=True, exist_ok=True)
        (crate_dir / "Cargo.toml").write_text(body, encoding="utf-8")

    def test_current_workspace_is_legal(self) -> None:
        errors = check_architecture.check_workspace(check_architecture.CRATES)
        self.assertEqual(errors, [])

    def test_current_editor_modules_respect_debt_ratchet(self) -> None:
        errors = check_architecture.check_editor_module_sizes(check_architecture.EDITOR_SOURCE)
        self.assertEqual(errors, [])

    def test_current_debt_allowlist_only_holds_modules_over_the_cap(self) -> None:
        debt = check_architecture.load_editor_debt()
        for relative, limits in debt.items():
            self.assertTrue(
                limits["max_lines"] > check_architecture.EDITOR_MAX_RUST_LINES
                or limits["max_bytes"] > check_architecture.EDITOR_MAX_RUST_BYTES,
                f"{relative} fits the normal cap; its exception must be removed",
            )

    def test_production_module_line_cap_is_enforced(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "app" / "god_file.rs"
            source.parent.mkdir(parents=True)
            source.write_text("one\ntwo\nthree\nfour\n", encoding="utf-8")

            errors = check_architecture.check_editor_module_sizes(
                root,
                max_lines=3,
                max_bytes=1024,
                debt={},
            )
            self.assertTrue(
                any("4 lines exceeds the 3-line production module cap" in error for error in errors)
            )

    def test_production_module_byte_cap_is_enforced(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "app" / "god_file.rs"
            source.parent.mkdir(parents=True)
            source.write_text("0123456789ABCDEF", encoding="utf-8")

            errors = check_architecture.check_editor_module_sizes(
                root,
                max_lines=10,
                max_bytes=8,
                debt={},
            )
            self.assertTrue(
                any("16 bytes exceeds the 8-byte production module cap" in error for error in errors)
            )

    def test_dedicated_test_module_uses_test_cap(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "app" / "tests.rs"
            source.parent.mkdir(parents=True)
            source.write_text("one\ntwo\nthree\nfour\n", encoding="utf-8")

            errors = check_architecture.check_editor_module_sizes(
                root,
                max_lines=3,
                max_bytes=1024,
                test_max_lines=4,
                test_max_bytes=1024,
                debt={},
            )
            self.assertEqual(errors, [])

    def test_dedicated_test_module_test_cap_is_enforced(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "tests" / "large_tests.rs"
            source.parent.mkdir(parents=True)
            source.write_text("one\ntwo\nthree\nfour\nfive\n", encoding="utf-8")

            errors = check_architecture.check_editor_module_sizes(
                root,
                max_lines=3,
                max_bytes=1024,
                test_max_lines=4,
                test_max_bytes=1024,
                debt={},
            )
            self.assertTrue(
                any("5 lines exceeds the 4-line test module cap" in error for error in errors)
            )

    def test_legacy_debt_at_recorded_baseline_is_allowed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "app" / "legacy.rs"
            source.parent.mkdir(parents=True)
            source.write_text("one\ntwo\nthree\nfour\n", encoding="utf-8")
            debt = {
                "app/legacy.rs": {
                    "max_lines": 4,
                    "max_bytes": len(source.read_bytes()),
                }
            }

            errors = check_architecture.check_editor_module_sizes(
                root,
                max_lines=3,
                max_bytes=1024,
                debt=debt,
            )
            self.assertEqual(errors, [])

    def test_legacy_debt_cannot_grow_in_lines(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "app" / "legacy.rs"
            source.parent.mkdir(parents=True)
            source.write_text("one\ntwo\nthree\nfour\n", encoding="utf-8")
            debt = {
                "app/legacy.rs": {
                    "max_lines": 3,
                    "max_bytes": len(source.read_bytes()),
                }
            }

            errors = check_architecture.check_editor_module_sizes(
                root,
                max_lines=3,
                max_bytes=1024,
                debt=debt,
            )
            self.assertTrue(any("legacy debt grew from 3 to 4 lines" in error for error in errors))

    def test_legacy_debt_cannot_grow_in_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "app" / "legacy.rs"
            source.parent.mkdir(parents=True)
            source.write_text("0123456789ABCDEF", encoding="utf-8")
            debt = {"app/legacy.rs": {"max_lines": 10, "max_bytes": 15}}

            errors = check_architecture.check_editor_module_sizes(
                root,
                max_lines=10,
                max_bytes=8,
                debt=debt,
            )
            self.assertTrue(any("legacy debt grew from 15 to 16 bytes" in error for error in errors))

    def test_debt_entry_must_be_removed_after_file_reaches_normal_cap(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "app" / "legacy.rs"
            source.parent.mkdir(parents=True)
            source.write_text("one\ntwo\n", encoding="utf-8")
            debt = {
                "app/legacy.rs": {
                    "max_lines": 4,
                    "max_bytes": len(source.read_bytes()) + 100,
                }
            }

            errors = check_architecture.check_editor_module_sizes(
                root,
                max_lines=3,
                max_bytes=1024,
                debt=debt,
            )
            self.assertTrue(any("legacy debt exception is stale" in error for error in errors))

    def test_orphaned_debt_entry_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            root.mkdir(exist_ok=True)
            debt = {"app/missing.rs": {"max_lines": 600, "max_bytes": 20000}}

            errors = check_architecture.check_editor_module_sizes(root, debt=debt)
            self.assertTrue(any("has no matching Rust module" in error for error in errors))

    def test_malformed_debt_allowlist_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            debt_path = Path(directory) / "architecture_debt.json"
            debt_path.write_text(
                json.dumps(
                    {
                        "version": 1,
                        "editor_modules": {
                            "app/legacy.rs": {"max_lines": 600, "max_bytes": "not-an-int"}
                        },
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "positive integer max_lines/max_bytes"):
                check_architecture.load_editor_debt(debt_path)

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

    def test_aliased_reference_dependency_is_still_rejected(self) -> None:
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
reference-tool = { package = "osrs-reference", path = "../osrs-reference" }
""",
            )

            errors = check_architecture.check_workspace(root)
            self.assertTrue(any("production crates must never depend" in error for error in errors))

    def test_edit_domain_may_depend_on_core(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_manifest(
                root,
                "osrs-edit",
                """
[package]
name = "osrs-edit"
version = "0.0.0"

[dependencies]
osrs-core = { path = "../osrs-core" }
""",
            )

            errors = check_architecture.check_workspace(root)
            self.assertEqual(errors, [])

    def test_edit_domain_cannot_depend_on_world_or_ui_runtime(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_manifest(
                root,
                "osrs-edit",
                """
[package]
name = "osrs-edit"
version = "0.0.0"

[dependencies]
osrs-core = { path = "../osrs-core" }
osrs-world = { path = "../osrs-world" }
eframe = "0.33"
""",
            )

            errors = check_architecture.check_workspace(root)
            self.assertTrue(any("illegal internal dependencies: osrs-world" in error for error in errors))

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
            self.assertTrue(
                any("renderer/UI/cache-transport dependency leakage: wgpu" in error for error in errors)
            )

    def test_aliased_renderer_dependency_is_still_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_manifest(
                root,
                "osrs-core",
                """
[package]
name = "osrs-core"
version = "0.0.0"

[dependencies]
gpu = { package = "wgpu", version = "28" }
""",
            )

            errors = check_architecture.check_workspace(root)
            self.assertTrue(
                any("renderer/UI/cache-transport dependency leakage: wgpu" in error for error in errors)
            )

    def test_world_may_compose_reusable_runtime_layers(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_manifest(
                root,
                "osrs-world",
                """
[package]
name = "osrs-world"
version = "0.0.0"

[dependencies]
osrs-core = { path = "../osrs-core" }
osrs-cache = { path = "../osrs-cache" }
osrs-scene = { path = "../osrs-scene" }
osrs-render = { path = "../osrs-render" }
""",
            )

            errors = check_architecture.check_workspace(root)
            self.assertEqual(errors, [])

    def test_world_cannot_depend_on_editor(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_manifest(
                root,
                "osrs-world",
                """
[package]
name = "osrs-world"
version = "0.0.0"

[dependencies]
osrs-core = { path = "../osrs-core" }
osrs-editor = { path = "../osrs-editor" }
""",
            )

            errors = check_architecture.check_workspace(root)
            self.assertTrue(any("illegal internal dependencies: osrs-editor" in error for error in errors))

    def test_editor_may_depend_on_edit_domain(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_manifest(
                root,
                "osrs-editor",
                """
[package]
name = "osrs-editor"
version = "0.0.0"

[dependencies]
osrs-core = { path = "../osrs-core" }
osrs-edit = { path = "../osrs-edit" }
osrs-world = { path = "../osrs-world" }
""",
            )

            errors = check_architecture.check_workspace(root)
            self.assertEqual(errors, [])

    def test_reference_may_depend_on_semantic_cache_and_world_crates(self) -> None:
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
osrs-world = { path = "../osrs-world" }
""",
            )

            errors = check_architecture.check_workspace(root)
            self.assertEqual(errors, [])


if __name__ == "__main__":
    unittest.main()
