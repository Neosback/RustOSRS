"""Tests for the complete performance-baseline orchestration (no editor, no GPU)."""
import json
import tempfile
import unittest
from pathlib import Path

import performance_baseline
from test_performance import metadata, report


def write_run(
    root: Path,
    workload: str,
    profile: str,
    sync: str,
    repetition: int,
    *,
    commit: str = "a" * 40,
    binary: str = "b" * 64,
) -> None:
    vsync = sync == "capped"
    payload = report(
        meta=metadata(
            workload=workload,
            profile=profile,
            vsync=vsync,
            commit=commit,
            binary=binary,
        )
    )
    (root / f"{workload}-{profile}-{sync}-{repetition}.json").write_text(json.dumps(payload))


class BaselineTests(unittest.TestCase):
    def test_expected_keys_covers_both_sync_modes(self):
        keys = performance_baseline.expected_keys(["water"], ["Reference", "Hd"])
        self.assertEqual(
            keys,
            {
                ("water", "Reference", "uncapped"),
                ("water", "Reference", "capped"),
                ("water", "Hd", "uncapped"),
                ("water", "Hd", "capped"),
            },
        )

    def test_freeze_binary_copies_and_verifies_exact_bytes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / "osrs-editor"
            source.write_bytes(b"frozen-editor-binary")
            source.chmod(0o755)
            frozen, digest = performance_baseline.freeze_binary(source, root / "evidence")
            self.assertTrue(frozen.is_file())
            self.assertEqual(frozen.read_bytes(), source.read_bytes())
            self.assertEqual(performance_baseline.sha256_file(frozen), digest)
            self.assertIn(digest[:16], frozen.name)

    def test_certify_directory_accepts_complete_clean_matrix(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for sync in ("uncapped", "capped"):
                write_run(root, "water", "Hd", sync, 0)
            groups, facts = performance_baseline.certify_directory(
                root,
                ["water"],
                ["Hd"],
                1,
                "b" * 64,
                "a" * 40,
            )
            self.assertEqual(
                set(groups),
                {
                    ("water", "Hd", "uncapped"),
                    ("water", "Hd", "capped"),
                },
            )
            self.assertEqual(facts["reports"], 2)
            self.assertEqual(facts["groups"], 2)
            self.assertEqual(facts["binary_sha256"], "b" * 64)

    def test_certify_directory_rejects_missing_matrix_cell(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_run(root, "water", "Hd", "uncapped", 0)
            with self.assertRaisesRegex(ValueError, "baseline matrix mismatch"):
                performance_baseline.certify_directory(
                    root,
                    ["water"],
                    ["Hd"],
                    1,
                    "b" * 64,
                    "a" * 40,
                )

    def test_certify_directory_rejects_cross_matrix_binary_drift(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_run(root, "water", "Hd", "uncapped", 0, binary="b" * 64)
            write_run(root, "water", "Hd", "capped", 0, binary="c" * 64)
            with self.assertRaisesRegex(ValueError, "binary_sha256"):
                performance_baseline.certify_directory(
                    root,
                    ["water"],
                    ["Hd"],
                    1,
                    "c" * 64,
                    "a" * 40,
                )

    def test_runner_argv_uses_frozen_binary_and_only_caps_when_requested(self):
        frozen = Path("/tmp/frozen/osrs-editor-deadbeef")
        common = dict(
            frozen_binary=frozen,
            cache=Path("/cache"),
            output=Path("/out"),
            workloads=["water"],
            profiles=["Hd"],
            repetitions=3,
            cooldown=0,
            reuse="on",
            retries=2,
            capture=False,
        )
        uncapped = performance_baseline.runner_argv(vsync=False, **common)
        capped = performance_baseline.runner_argv(vsync=True, **common)
        self.assertIn(str(frozen), uncapped)
        self.assertNotIn("--vsync", uncapped)
        self.assertIn("--vsync", capped)


if __name__ == "__main__":
    unittest.main()
