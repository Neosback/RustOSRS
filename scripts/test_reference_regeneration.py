#!/usr/bin/env python3
"""Offline tests for the M5 reference-regeneration boundary."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "tools" / "reference-fixtures" / "regenerate.py"
SPEC = importlib.util.spec_from_file_location("rustosrs_regenerate", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
REGENERATE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REGENERATE)


class ReferenceRegenerationTests(unittest.TestCase):
    def test_pin_is_exact_expected_public_deob_commit(self) -> None:
        self.assertEqual(
            REGENERATE.PINNED_MELXIN_DEOB_COMMIT,
            "1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc",
        )

    def test_candidate_inside_reference_fixtures_is_rejected(self) -> None:
        candidate = ROOT / "reference-fixtures" / "candidate.txt"
        with self.assertRaisesRegex(ValueError, "may not be written inside"):
            REGENERATE.reject_checked_in_candidate(candidate, ROOT)

    def test_external_candidate_is_allowed(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            candidate = Path(temp_dir) / "candidate.txt"
            self.assertEqual(
                REGENERATE.reject_checked_in_candidate(candidate, ROOT),
                candidate.resolve(strict=False),
            )

    def test_adapter_command_carries_exact_pin_and_candidate(self) -> None:
        command = REGENERATE.build_melxin_command(
            checkout=Path("/tmp/deob"),
            bcprov=Path("/tmp/bcprov.jar"),
            candidate=Path("/tmp/candidate.txt"),
            work_dir=Path("/tmp/work"),
        )
        self.assertEqual(command[0], "sh")
        self.assertIn(str(ROOT / "tools" / "deob-harness" / "run.sh"), command)
        self.assertIn("--expected-commit", command)
        pin_index = command.index("--expected-commit") + 1
        self.assertEqual(command[pin_index], REGENERATE.PINNED_MELXIN_DEOB_COMMIT)
        output_index = command.index("--output") + 1
        self.assertEqual(command[output_index], "/tmp/candidate.txt")

    def test_legacy_harness_has_no_machine_specific_path_or_direct_fixture_write(self) -> None:
        harness = (ROOT / "tools" / "deob-harness" / "run.sh").read_text(encoding="utf-8")
        self.assertNotIn("/Users/tylercovalt", harness)
        self.assertNotIn("reference-fixtures/deob_golden.txt", harness)
        self.assertNotIn("curl ", harness)
        self.assertIn("candidate output may not be written inside reference-fixtures", harness)


if __name__ == "__main__":
    unittest.main()
