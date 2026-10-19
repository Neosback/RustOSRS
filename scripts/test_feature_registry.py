"""Regression tests for the canonical editor feature registry loader."""
from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from feature_registry import (
    CURRENT_SCHEMA_VERSION,
    REGISTRY_PATH,
    RegistryError,
    load_registry,
    normalize_registry,
)


class FeatureRegistryTests(unittest.TestCase):
    def test_flat_registry_is_rejected_after_schema_migration(self):
        with self.assertRaisesRegex(RegistryError, "unsupported feature registry version"):
            normalize_registry({"Paint": {"mutates_document": True}})

    def test_versioned_registry_returns_only_feature_entries(self):
        payload = {
            "version": CURRENT_SCHEMA_VERSION,
            "features": {"Paint": {"mutates_document": True}},
        }
        self.assertEqual(normalize_registry(payload), payload["features"])

    def test_versioned_registry_rejects_unknown_versions(self):
        with self.assertRaisesRegex(RegistryError, "unsupported feature registry version"):
            normalize_registry({"version": 999, "features": {}})

    def test_feature_names_and_values_are_validated_once(self):
        for payload in (
            {"version": 1, "features": {"": {}}},
            {"version": 1, "features": {"Paint": []}},
            {"version": 1, "features": []},
        ):
            with self.subTest(payload=payload), self.assertRaises(RegistryError):
                normalize_registry(payload)

    def test_loader_reports_invalid_json_as_registry_error(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "features.json"
            path.write_text("{not-json", encoding="utf-8")
            with self.assertRaisesRegex(RegistryError, "invalid JSON"):
                load_registry(path)

    def test_loader_reads_versioned_file(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "features.json"
            path.write_text(
                json.dumps({"version": 1, "features": {"Select": {"mutates_document": False}}}),
                encoding="utf-8",
            )
            self.assertIn("Select", load_registry(path))

    def test_repository_registry_uses_current_schema(self):
        payload = json.loads(REGISTRY_PATH.read_text(encoding="utf-8"))
        self.assertEqual(payload.get("version"), CURRENT_SCHEMA_VERSION)
        self.assertEqual(load_registry(), payload["features"])


if __name__ == "__main__":
    unittest.main()
