#!/usr/bin/env python3
"""Focused tests for the reproducible architecture dashboard."""
from __future__ import annotations

import json
import unittest

import architecture_dashboard
from feature_registry import ROOT


class ArchitectureDashboardTests(unittest.TestCase):
    def test_live_dashboard_uses_canonical_sources_and_has_no_cp2_violations(self) -> None:
        metrics = architecture_dashboard.collect(ROOT)

        self.assertGreater(metrics["registered_features"], 0)
        self.assertGreater(metrics["registered_mutating_features"], 0)
        self.assertLessEqual(
            metrics["registered_mutating_features"], metrics["registered_features"]
        )
        self.assertGreater(metrics["editor_app_fields"], 0)
        self.assertGreater(metrics["editor_app_methods"], 0)
        for name in architecture_dashboard.VIOLATION_METRICS:
            self.assertEqual(metrics[name], 0, name)

    def test_json_shape_is_versioned_and_metric_names_are_stable(self) -> None:
        metrics = {
            "registered_features": 1,
            "registered_mutating_features": 1,
            "feature_contract_violations": 0,
            "authority_gateway_violations": 0,
            "legacy_history_violations": 0,
            "application_history_violations": 0,
            "tracked_lifecycle_write_sites": 2,
            "explicit_editor_debt_modules": 3,
            "editor_app_fields": 4,
            "editor_app_methods": 5,
        }
        payload = json.dumps({"version": 1, "metrics": metrics}, sort_keys=True)
        decoded = json.loads(payload)

        self.assertEqual(decoded["version"], 1)
        self.assertEqual(decoded["metrics"], metrics)
        rendered = architecture_dashboard.render(metrics)
        self.assertIn("Architecture dashboard", rendered)
        self.assertIn("application history violations", rendered)


if __name__ == "__main__":
    unittest.main()
