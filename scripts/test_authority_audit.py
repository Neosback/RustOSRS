"""Focused tests for the source-level semantic authority inventory."""
from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

import authority_audit


class AuthorityAuditTests(unittest.TestCase):
    def test_scan_classifies_checkpoint_legacy_and_lifecycle_hotspots(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/osrs-editor/src/app"
            source.mkdir(parents=True)
            (source / "sample.rs").write_text(
                """
fn example() {
    session.commit_checkpointed(patch);
    let _ = step.changes.len();
    session.apply_untracked(&patch);
}
""",
                encoding="utf-8",
            )
            hits = authority_audit.scan(root)
            self.assertEqual(
                [(hit.category, hit.line) for hit in hits],
                [
                    ("checkpoint-lifecycle", 3),
                    ("legacy-history", 4),
                    ("lifecycle-write", 5),
                ],
            )

    def test_scan_ignores_unrelated_history_words(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/osrs-editor/src"
            source.mkdir(parents=True)
            (source / "other.rs").write_text(
                "let changes = values.changed();\nlet removed_count = 2;\n",
                encoding="utf-8",
            )
            self.assertEqual(authority_audit.scan(root), [])


if __name__ == "__main__":
    unittest.main()
