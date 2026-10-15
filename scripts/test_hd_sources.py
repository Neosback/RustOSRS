"""Failure-path checks for pinned-source verification."""
import hashlib
from pathlib import Path
import tempfile
import unittest

from check_hd_sources import verify_files


class SourcePinTests(unittest.TestCase):
    def test_matching_content_is_accepted_but_tampering_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "shader.glsl"
            path.write_bytes(b"pinned shader")
            pin = {"shader.glsl": hashlib.sha256(path.read_bytes()).hexdigest()}
            self.assertEqual(verify_files(root, pin), [])
            path.write_bytes(b"altered shader")
            self.assertTrue(verify_files(root, pin))

    def test_missing_and_additional_files_both_break_the_pin(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertTrue(verify_files(root, {"missing": "unused"}))
            (root / "additional").write_text("new upstream file")
            self.assertTrue(verify_files(root, {}))

    def test_optional_java_check_ignores_non_java_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "README.md").write_text("external build metadata")
            self.assertEqual(verify_files(root, {}, "*.java"), [])


if __name__ == "__main__":
    unittest.main()
