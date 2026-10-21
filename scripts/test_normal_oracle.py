#!/usr/bin/env python3
"""Offline safety checks for exact-method normal-oracle regeneration."""

import importlib.util
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "normal_oracle", ROOT / "scripts/normal_oracle/regenerate.py",
)
ORACLE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ORACLE)


class NormalOracleTests(unittest.TestCase):
    def test_new_manifest_revisions_pin_the_complete_adapter(self):
        files = ("regenerate.py", "NormalOracle.java", "model_fields.java.txt", "sources.json")
        digest = hashlib.sha256(b"".join((ORACLE.HERE / name).read_bytes() for name in files)).hexdigest()
        manifests = [path for path in (ROOT / "reference-fixtures/manifest").glob("normals-*.yaml")
                     if "melxin-deob-extracted-methods" in path.read_text()]
        self.assertEqual(len(manifests), 5)
        for path in manifests:
            self.assertIn(f"  revision: sha256:{digest}\n", path.read_text(), str(path))

    def test_rejects_accepted_tree_and_symlink_alias(self):
        with self.assertRaisesRegex(ValueError, "inside reference-fixtures"):
            ORACLE.candidate_directory(ROOT / "reference-fixtures/normals/candidate")
        with tempfile.TemporaryDirectory() as temporary:
            alias = Path(temporary) / "alias"
            alias.symlink_to(ROOT / "reference-fixtures", target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "inside reference-fixtures"):
                ORACLE.candidate_directory(alias / "candidate")

    def test_rejects_existing_output_instead_of_overwriting(self):
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(ValueError, "refusing overwrite"):
                ORACLE.candidate_directory(Path(temporary))
            path = Path(temporary) / "new-candidate"
            self.assertEqual(ORACLE.candidate_directory(path), path.resolve())
            self.assertFalse(path.exists())

    def test_rejects_source_pin_mismatch_before_invoking_java(self):
        pins = json.loads((ORACLE.HERE / "sources.json").read_text())
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / pins["sources"][0]["path"]
            source.parent.mkdir(parents=True)
            source.write_text("changed source")
            with patch.object(ORACLE.subprocess, "run") as run:
                with self.assertRaisesRegex(ValueError, "Source pin mismatch"):
                    ORACLE.generate(Path(temporary), Path(temporary))
                run.assert_not_called()

    def test_extraction_preserves_body_and_rejects_ambiguous_or_truncated_source(self):
        method = "void normals() { if (true) { count++; } }"
        self.assertEqual(ORACLE.extract_method("prefix\n" + method + "\nsuffix", "void normals()"), method)
        for source in (method * 2, "void other() {}", "void normals() { if (true) {"):
            with self.assertRaises(ValueError):
                ORACLE.extract_method(source, "void normals()")

    def test_rejects_inputs_beyond_original_marker_capacity(self):
        with self.assertRaisesRegex(ValueError, "marker capacity"):
            ORACLE.encode_model({"vertices": [None] * 10001, "faces": []})


if __name__ == "__main__":
    unittest.main()
