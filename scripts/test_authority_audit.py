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

    def test_scan_marks_legacy_history_payload_fields(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/osrs-editor/src/app"
            source.mkdir(parents=True)
            (source / "history.rs").write_text(
                """
struct EditStep {
    pub(super) changes: Vec<Change>,
    pub(super) removed: Vec<Loc>,
    pub(super) added: Vec<Loc>,
}
""",
                encoding="utf-8",
            )
            hits = authority_audit.scan(root)
            self.assertEqual(len([hit for hit in hits if hit.category == "legacy-history"]), 4)

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

    def test_legacy_history_ratchet_allows_zero_baseline(self):
        hits = []
        line = 1
        for path, ceiling in authority_audit.LEGACY_HISTORY_BASELINE.items():
            for _ in range(ceiling):
                hits.append(authority_audit.Hit("legacy-history", path, line, "legacy"))
                line += 1
        self.assertEqual(authority_audit.legacy_history_violations(hits), [])

    def test_legacy_history_ratchet_rejects_reintroduction_in_former_scaffold_file(self):
        path = "crates/osrs-editor/src/app/editing/tests.rs"
        hit = authority_audit.Hit("legacy-history", path, 1, "legacy")
        self.assertEqual(authority_audit.legacy_history_violations([hit]), [hit])

    def test_legacy_history_ratchet_rejects_new_file(self):
        hit = authority_audit.Hit(
            "legacy-history",
            "crates/osrs-editor/src/app/new_tool.rs",
            1,
            "step.changes",
        )
        self.assertEqual(authority_audit.legacy_history_violations([hit]), [hit])

    def test_legacy_history_ratchet_rejects_growth_in_allowed_file(self):
        path = "crates/osrs-editor/src/app/editing/tests.rs"
        ceiling = authority_audit.LEGACY_HISTORY_BASELINE[path]
        hits = [
            authority_audit.Hit("legacy-history", path, line, "legacy")
            for line in range(1, ceiling + 2)
        ]
        self.assertEqual(authority_audit.legacy_history_violations(hits), [hits[-1]])


if __name__ == "__main__":
    unittest.main()
