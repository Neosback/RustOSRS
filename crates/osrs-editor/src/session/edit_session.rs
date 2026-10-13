use osrs_edit::{ChangeImpact, EditHistory, MapDocument, MapPatch, PatchApplyError};

/// One successful semantic state transition in the editor session.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditCommit {
    pub revision: u64,
    pub impact: ChangeImpact,
}

/// Persistent authoritative semantic editing state for one editor session.
///
/// `MapDocument` is the source of truth. `EditHistory` records reversible semantic patches.
/// `revision` advances only after successful state-changing commit/undo/redo operations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditorSession {
    document: MapDocument,
    history: EditHistory,
    revision: u64,
}

// The app owns this session now, but most mutation/query methods are intentionally staged one
// checkpoint before individual editor tools cut over. Keep tests unsuppressed so this allowance
// disappears naturally as the runtime begins using the API.
#[cfg_attr(not(test), allow(dead_code))]
impl EditorSession {
    pub fn new(document: MapDocument) -> Self {
        Self {
            document,
            history: EditHistory::new(),
            revision: 0,
        }
    }

    pub fn document(&self) -> &MapDocument {
        &self.document
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Apply one logical semantic edit.
    ///
    /// Empty patches remain true no-ops: they do not advance the session revision and do not
    /// disturb redo history.
    pub fn commit(&mut self, patch: MapPatch) -> Result<Option<EditCommit>, PatchApplyError> {
        let is_empty = patch.is_empty();
        let impact = self.history.commit(&mut self.document, patch)?;
        if is_empty {
            return Ok(None);
        }
        Ok(Some(self.record_change(impact)))
    }

    /// Undo the most recent semantic edit.
    pub fn undo(&mut self) -> Result<Option<EditCommit>, PatchApplyError> {
        self.history
            .undo(&mut self.document)
            .map(|impact| impact.map(|impact| self.record_change(impact)))
    }

    /// Redo the most recently undone semantic edit.
    pub fn redo(&mut self) -> Result<Option<EditCommit>, PatchApplyError> {
        self.history
            .redo(&mut self.document)
            .map(|impact| impact.map(|impact| self.record_change(impact)))
    }

    fn record_change(&mut self, impact: ChangeImpact) -> EditCommit {
        self.revision += 1;
        EditCommit {
            revision: self.revision,
            impact,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_edit::{HeightVertexCoord, MapOp};

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn vertex() -> HeightVertexCoord {
        HeightVertexCoord::new(MapTile::new(3200, 3200), plane())
    }

    fn session(height: i32) -> EditorSession {
        EditorSession::new(MapDocument::from_parts([], [(vertex(), height)], []))
    }

    fn height_patch(before: i32, after: i32) -> MapPatch {
        MapPatch::new([MapOp::SetHeight {
            coord: vertex(),
            before,
            after,
        }])
    }

    #[test]
    fn successful_commit_updates_document_and_revision_once() {
        let mut session = session(0);

        let commit = session
            .commit(height_patch(0, 64))
            .unwrap_or_else(|error| unreachable!("valid commit: {error:?}"))
            .unwrap_or_else(|| unreachable!("non-empty patch creates a commit"));

        assert_eq!(session.document().height_at(vertex()), Some(64));
        assert_eq!(session.revision(), 1);
        assert_eq!(commit.revision, 1);
        assert_eq!(
            commit.impact,
            ChangeImpact::TERRAIN_GEOMETRY
                | ChangeImpact::NORMALS
                | ChangeImpact::LIGHTING
                | ChangeImpact::LOCATIONS
                | ChangeImpact::BOUNDS
        );
        assert!(session.can_undo());
        assert!(!session.can_redo());
    }

    #[test]
    fn rejected_commit_changes_neither_document_history_nor_revision() {
        let mut session = session(0);

        assert!(session.commit(height_patch(999, 64)).is_err());
        assert_eq!(session.document().height_at(vertex()), Some(0));
        assert_eq!(session.revision(), 0);
        assert!(!session.can_undo());
        assert!(!session.can_redo());
    }

    #[test]
    fn undo_and_redo_each_advance_revision_after_success() {
        let mut session = session(0);
        assert!(session.commit(height_patch(0, 64)).is_ok());

        let undo = session
            .undo()
            .unwrap_or_else(|error| unreachable!("valid undo: {error:?}"))
            .unwrap_or_else(|| unreachable!("undo entry exists"));
        assert_eq!(session.document().height_at(vertex()), Some(0));
        assert_eq!(undo.revision, 2);
        assert_eq!(session.revision(), 2);
        assert!(session.can_redo());

        let redo = session
            .redo()
            .unwrap_or_else(|error| unreachable!("valid redo: {error:?}"))
            .unwrap_or_else(|| unreachable!("redo entry exists"));
        assert_eq!(session.document().height_at(vertex()), Some(64));
        assert_eq!(redo.revision, 3);
        assert_eq!(session.revision(), 3);
    }

    #[test]
    fn empty_commit_is_not_a_revision_and_preserves_redo() {
        let mut session = session(0);
        assert!(session.commit(height_patch(0, 64)).is_ok());
        assert!(session.undo().is_ok());
        assert!(session.can_redo());
        let revision = session.revision();

        assert_eq!(session.commit(MapPatch::default()), Ok(None));
        assert_eq!(session.revision(), revision);
        assert!(session.can_redo());
    }

    #[test]
    fn empty_undo_and_redo_do_not_advance_revision() {
        let mut session = session(0);

        assert_eq!(session.undo(), Ok(None));
        assert_eq!(session.redo(), Ok(None));
        assert_eq!(session.revision(), 0);
    }

    #[test]
    fn failed_undo_does_not_advance_revision_or_corrupt_history() {
        let mut session = session(0);
        assert!(session.commit(height_patch(0, 64)).is_ok());
        assert_eq!(session.revision(), 1);

        let out_of_band = height_patch(64, 128);
        assert!(session.document.apply_patch(&out_of_band).is_ok());

        assert!(session.undo().is_err());
        assert_eq!(session.document().height_at(vertex()), Some(128));
        assert_eq!(session.revision(), 1);
        assert!(session.can_undo());
        assert!(!session.can_redo());
    }
}
