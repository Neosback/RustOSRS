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
            self.assertEqual(len([hit for hit in hits if hit.category == "legacy-history"]), 3)

    def test_scan_ignores_editstep_type_declaration_for_legacy_payload_check_but_marks_direct_construction(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/osrs-editor/src/app"
            source.mkdir(parents=True)
            (source / "history.rs").write_text(
                """
pub(in crate::app) struct EditStep {
    semantic: bool,
}
fn direct() {
    let _ = EditStep { semantic: true };
}
""",
                encoding="utf-8",
            )
            hits = [hit for hit in authority_audit.scan(root) if hit.category == "legacy-history"]
            self.assertEqual(
                [(hit.line, hit.text) for hit in hits],
                [(6, "let _ = EditStep { semantic: true };")],
            )

    def test_scan_marks_application_history_fields_and_retired_route_types(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            editing = root / "crates/osrs-editor/src/app/editing.rs"
            editing.parent.mkdir(parents=True)
            editing.write_text(
                """
struct TerrainController {
    history: Vec<EditStep>,
    redo_stack: Vec<EditStep>,
}
""",
                encoding="utf-8",
            )
            history = root / "crates/osrs-editor/src/app/editing/history.rs"
            history.parent.mkdir(parents=True)
            history.write_text(
                "struct SemanticPaintHistory;\nstruct SemanticHeightHistory;\nstruct SemanticPathHistory;\n",
                encoding="utf-8",
            )
            hits = [
                hit
                for hit in authority_audit.scan(root)
                if hit.category == "application-history"
            ]
            self.assertEqual(
                [(hit.path, hit.line) for hit in hits],
                [
                    ("crates/osrs-editor/src/app/editing.rs", 3),
                    ("crates/osrs-editor/src/app/editing.rs", 4),
                    ("crates/osrs-editor/src/app/editing/history.rs", 1),
                    ("crates/osrs-editor/src/app/editing/history.rs", 2),
                    ("crates/osrs-editor/src/app/editing/history.rs", 3),
                ],
            )

    def test_application_history_guard_rejects_retired_marker_and_sinks_but_ignores_session_history(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            app = root / "crates/osrs-editor/src/app/editing"
            app.mkdir(parents=True)
            (app / "history.rs").write_text(
                "pub(in crate::app) struct EditStep;\npush_step(EditStep);\nrecord_scenario();\n",
                encoding="utf-8",
            )
            session = root / "crates/osrs-editor/src/session"
            session.mkdir(parents=True)
            (session / "history.rs").write_text(
                "struct PatchHistory { history: Vec<u8>, redo_stack: Vec<u8> }\n",
                encoding="utf-8",
            )
            hits = [
                hit
                for hit in authority_audit.scan(root)
                if hit.category == "application-history"
            ]
            self.assertEqual(
                [(hit.path, hit.line) for hit in hits],
                [
                    ("crates/osrs-editor/src/app/editing/history.rs", 1),
                    ("crates/osrs-editor/src/app/editing/history.rs", 2),
                    ("crates/osrs-editor/src/app/editing/history.rs", 3),
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

    def test_legacy_history_allows_only_zero_hits(self):
        self.assertEqual(authority_audit.legacy_history_violations([]), [])

    def test_legacy_history_rejects_every_hit_regardless_of_file(self):
        former_scaffold = authority_audit.Hit(
            "legacy-history",
            "crates/osrs-editor/src/app/editing/tests.rs",
            1,
            "step.changes",
        )
        new_file = authority_audit.Hit(
            "legacy-history",
            "crates/osrs-editor/src/app/new_tool.rs",
            2,
            "EditStep {",
        )
        unrelated = authority_audit.Hit(
            "checkpoint-lifecycle",
            "crates/osrs-editor/src/app/edit_runtime.rs",
            3,
            "commit_checkpointed(",
        )
        self.assertEqual(
            authority_audit.legacy_history_violations([new_file, unrelated, former_scaffold]),
            sorted([former_scaffold, new_file]),
        )

    def test_application_history_rejects_every_application_history_hit(self):
        first = authority_audit.Hit(
            "application-history",
            "crates/osrs-editor/src/app/editing.rs",
            1,
            "redo_stack: Vec<EditStep>,",
        )
        second = authority_audit.Hit(
            "application-history",
            "crates/osrs-editor/src/app/editing/history.rs",
            2,
            "struct SemanticPaintHistory;",
        )
        unrelated = authority_audit.Hit(
            "legacy-history",
            "crates/osrs-editor/src/app/editing/tests.rs",
            3,
            "step.changes",
        )
        self.assertEqual(
            authority_audit.application_history_violations([second, unrelated, first]),
            sorted([first, second]),
        )


if __name__ == "__main__":
    unittest.main()
