"""Focused tests for the read-only feature navigation CLI."""
from __future__ import annotations

import contextlib
from io import StringIO
import unittest

import features


def sample_features():
    return {
        "Paint": {
            "id": "paint",
            "aliases": ["terrain paint", "tile paint"],
            "owner": {"crate": "osrs-edit", "area": "paint"},
            "status": "implemented",
            "mutates_document": True,
            "planner": {
                "file": "crates/osrs-edit/src/paint.rs",
                "symbol": "build_terrain_paint_patch",
            },
        },
        "Buildings": {
            "id": "buildings",
            "aliases": ["building generator", "procgen building"],
            "owner": {"crate": "osrs-edit", "area": "building_kit"},
            "status": "implemented",
            "kind": "workspace",
            "mutates_document": True,
            "planner": {
                "file": "crates/osrs-edit/src/building_kit/generate.rs",
                "symbol": "generate",
            },
        },
    }


class FeatureCliTests(unittest.TestCase):
    def test_exact_feature_resolves_stable_id_and_alias(self):
        registry = sample_features()
        self.assertEqual(features.exact_feature(registry, "paint")[0], "Paint")
        self.assertEqual(features.exact_feature(registry, "terrain paint")[0], "Paint")
        self.assertEqual(features.exact_feature(registry, "BUILDING GENERATOR")[0], "Buildings")

    def test_find_indexes_alias_and_owner_metadata(self):
        registry = sample_features()
        self.assertEqual(features.find_features(registry, "procgen building")[0][0], "Buildings")
        self.assertEqual(features.find_features(registry, "building_kit")[0][0], "Buildings")

    def test_show_by_id_surfaces_owner_status_and_planner(self):
        output = StringIO()
        with contextlib.redirect_stdout(output):
            code = features.command_show(sample_features(), "buildings")
        text = output.getvalue()
        self.assertEqual(code, 0)
        self.assertIn("id: buildings", text)
        self.assertIn("status: implemented", text)
        self.assertIn("owner: osrs-edit::building_kit", text)
        self.assertIn("planner: crates/osrs-edit/src/building_kit/generate.rs::generate", text)

    def test_missing_feature_returns_failure(self):
        error = StringIO()
        with contextlib.redirect_stderr(error):
            code = features.command_show(sample_features(), "water rendering")
        self.assertEqual(code, 1)
        self.assertIn("No registered feature matches", error.getvalue())


if __name__ == "__main__":
    unittest.main()
