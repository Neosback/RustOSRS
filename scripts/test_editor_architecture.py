"""Regression tests for editor ownership and feature-navigation contracts."""
from __future__ import annotations
import copy
from pathlib import Path
import tempfile
import unittest
import check_architecture
import editor_architecture as guard
from feature_registry import load_registry

ROOT = Path(__file__).resolve().parents[1]


class EditorArchitectureTests(unittest.TestCase):
    def test_workspace_features_keep_behavior_checks_without_hiding_tools(self):
        features = load_registry(ROOT / "scripts/editor_features.json")
        self.assertEqual(guard.check_features(ROOT, features), [])
        features["Paint"]["kind"] = "workspace"
        self.assertTrue(any("Tool variants" in error for error in guard.check_features(ROOT, features)))
        self.assertTrue(any("invalid feature kind" in error for error in guard.check_features(ROOT, features)))

    def test_current_editor_contracts_match_the_source(self):
        self.assertEqual(guard.check_editor_contracts(ROOT), [])

    def test_rust_literals_and_nested_comments_cannot_hide_or_invent_app_methods(self):
        source = '''
// impl EditorApp { fn comment() {} }
/* outer /* impl EditorApp { fn nested() {} } */ end */
const TEXT: &str = r##"impl EditorApp { fn raw_string() {} }"##;
impl crate::app::EditorApp {
    fn real(&self) { let _ = '}'; let _ = "fn fake() {}"; fn local() {} }
}
impl eframe::App for EditorApp { fn ui(&mut self) {} }
'''
        self.assertEqual(guard.app_methods(source), ["real", "ui"])

    def test_lifetimes_are_code_but_char_and_byte_literals_are_not(self):
        source = "impl EditorApp { fn view<'a>(&'a self) { let _ = b'{'; let _ = '\\n'; } }"
        self.assertEqual(guard.app_methods(source), ["view"])

    def fixture(self, directory):
        source = Path(directory)
        (source / "app.rs").write_text("pub struct EditorApp { world: WorldRuntime }\nimpl EditorApp { fn boot() {} }\n")
        baseline = {"fields": ["world"], "methods": {"app.rs": ["boot"]}}
        return source, baseline

    def test_new_app_methods_in_existing_and_new_modules_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            source, baseline = self.fixture(directory)
            (source / "app.rs").write_text("pub struct EditorApp { world: WorldRuntime }\nimpl EditorApp { fn boot() {} fn paint() {} }")
            (source / "feature.rs").write_text("impl EditorApp { fn another_feature() {} }")
            errors = guard.check_app_surface(source, baseline)
            self.assertTrue(any("paint" in error for error in errors))
            self.assertTrue(any("another_feature" in error for error in errors))

    def test_removing_app_behavior_requires_lowering_its_baseline(self):
        with tempfile.TemporaryDirectory() as directory:
            source, baseline = self.fixture(directory)
            (source / "app.rs").write_text("pub struct EditorApp { world: WorldRuntime }")
            self.assertTrue(any("lower the ownership ratchet" in error for error in guard.check_app_surface(source, baseline)))
            baseline["methods"] = {}
            self.assertEqual(guard.check_app_surface(source, baseline), [])

    def test_duplicate_cfg_implementations_cannot_grow_unnoticed(self):
        with tempfile.TemporaryDirectory() as directory:
            source, baseline = self.fixture(directory)
            with (source / "app.rs").open("a") as handle:
                handle.write("impl EditorApp { fn boot() {} }")
            self.assertTrue(any("new broad app methods" in error for error in guard.check_app_surface(source, baseline)))

    def test_new_direct_app_state_requires_ownership_review(self):
        with tempfile.TemporaryDirectory() as directory:
            source, baseline = self.fixture(directory)
            (source / "app.rs").write_text("pub struct EditorApp {\nworld: WorldRuntime,\nbrushes: Vec<Brush>,\n}\nimpl EditorApp { fn boot() {} }")
            self.assertTrue(any("canonical ownership" in error for error in guard.check_app_surface(source, baseline)))

    def test_new_tool_requires_its_feature_contract(self):
        features = load_registry(ROOT / "scripts/editor_features.json")
        del features["Height"]
        self.assertTrue(any("Tool variants" in error for error in guard.check_features(ROOT, features)))

    def test_mutating_tool_requires_all_four_behavior_tests(self):
        features = load_registry(ROOT / "scripts/editor_features.json")
        for phase in ["planning_test", "cancellation_test", "commit_test", "undo_test"]:
            candidate = copy.deepcopy(features)
            del candidate["Paint"][phase]
            self.assertTrue(any(f"missing {phase}" in error for error in guard.check_features(ROOT, candidate)))

    def test_feature_references_must_resolve_to_actual_tests(self):
        features = load_registry(ROOT / "scripts/editor_features.json")
        features["Paint"]["undo_test"] = features["Paint"]["planner"]
        self.assertTrue(any("not a #[test]" in error for error in guard.check_features(ROOT, features)))
        features["Paint"]["undo_test"]["file"] = "../outside.rs"
        self.assertTrue(any("repository-relative" in error for error in guard.check_features(ROOT, features)))

    def test_headless_domain_rejects_aliased_target_specific_platform_dependencies(self):
        for package in ["wgpu", "egui", "eframe", "rfd", "web-sys", "wasm-bindgen", "tokio"]:
            with self.subTest(package=package), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                target = root / "osrs-edit"
                target.mkdir()
                (target / "Cargo.toml").write_text(f'''[package]
name = "osrs-edit"
version = "0.0.0"
[target.'cfg(target_arch = "wasm32")'.dependencies]
alias = {{ package = "{package}", version = "1" }}
''')
                self.assertTrue(any("UI/GPU/platform dependency leakage" in error for error in check_architecture.check_workspace(root)))


if __name__ == "__main__":
    unittest.main()
