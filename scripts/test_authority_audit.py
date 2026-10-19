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

    def test_gateway_allows_session_and_edit_runtime_only(self):
        hits = [
            authority_audit.Hit(
                "checkpoint-lifecycle",
                "crates/osrs-editor/src/session/edit_session.rs",
                10,
                "fn commit_checkpointed(",
            ),
            authority_audit.Hit(
                "checkpoint-lifecycle",
                "crates/osrs-editor/src/session/edit_session/tests.rs",
                20,
                "session.commit_checkpointed(patch);",
            ),
            authority_audit.Hit(
                "checkpoint-lifecycle",
                "crates/osrs-editor/src/app/edit_runtime.rs",
                30,
                "self.session.commit_checkpointed(patch);",
            ),
        ]
        self.assertEqual(authority_audit.gateway_violations(hits), [])

    def test_gateway_rejects_checkpoint_calls_from_feature_modules(self):
        hit = authority_audit.Hit(
            "checkpoint-lifecycle",
            "crates/osrs-editor/src/app/object_tool.rs",
            40,
            "self.session.commit_checkpointed(patch);",
        )
        self.assertEqual(authority_audit.gateway_violations([hit]), [hit])


if __name__ == "__main__":
    unittest.main()
