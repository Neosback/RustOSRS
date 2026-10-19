"""Focused tests for the generated human feature index."""
from __future__ import annotations

import unittest

import feature_index


class FeatureIndexTests(unittest.TestCase):
    def test_render_is_stable_and_contains_ai_navigation_metadata(self):
        features = {
            "Paint": {
                "id": "paint",
                "aliases": ["terrain paint"],
                "owner": {"crate": "osrs-edit", "area": "paint"},
                "status": "implemented",
                "mutates_document": True,
                "controller": {"file": "controller.rs", "symbol": "paint_at"},
                "planner": {"file": "planner.rs", "symbol": "plan"},
                "gateway": {"file": "gateway.rs", "symbol": "commit"},
                "planning_test": {"file": "tests.rs", "symbol": "plans"},
                "cancellation_test": {"file": "tests.rs", "symbol": "cancels"},
                "commit_test": {"file": "tests.rs", "symbol": "commits"},
                "undo_test": {"file": "tests.rs", "symbol": "undoes"},
            }
        }
        rendered = feature_index.render(features)
        self.assertIn("| `paint` | Paint | implemented | `osrs-edit::paint` | tool | mutating |", rendered)
        self.assertIn("- **Aliases:** `terrain paint`", rendered)
        self.assertIn("- **Planner:** `planner.rs::plan`", rendered)
        self.assertIn("- **Behavior tests:** `tests.rs::plans`", rendered)
        self.assertEqual(rendered, feature_index.render(features))

    def test_read_only_feature_does_not_invent_behavior_tests(self):
        rendered = feature_index.render(
            {
                "Select": {
                    "id": "select",
                    "aliases": ["selection"],
                    "owner": {"crate": "osrs-editor", "area": "selection"},
                    "status": "implemented",
                    "mutates_document": False,
                    "controller": {"file": "selection.rs", "symbol": "begin"},
                }
            }
        )
        self.assertIn("| `select` | Select | implemented", rendered)
        self.assertNotIn("Behavior tests", rendered)


if __name__ == "__main__":
    unittest.main()
