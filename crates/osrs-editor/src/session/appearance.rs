//! Application composition of authored appearance with the existing headless workspace/history.
use osrs_edit::{
    ChangeImpact, HistoryDocument, PatchApplyError, ReversiblePatch, WorkspaceDocument,
    WorkspacePatch,
};
use osrs_hd::theme::{ThemePack, ThemePatch};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct EditorDocument {
    pub workspace: WorkspaceDocument,
    pub appearance: ThemePack,
}
impl std::ops::Deref for EditorDocument {
    type Target = WorkspaceDocument;
    fn deref(&self) -> &Self::Target {
        &self.workspace
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum EditorPatch {
    Workspace(WorkspacePatch),
    Appearance(Box<ThemePatch>),
}
impl ReversiblePatch for EditorPatch {
    fn inverse(&self) -> Self {
        match self {
            Self::Workspace(p) => Self::Workspace(p.inverse()),
            Self::Appearance(p) => Self::Appearance(Box::new(p.inverse())),
        }
    }
    fn is_empty(&self) -> bool {
        match self {
            Self::Workspace(p) => p.is_empty(),
            Self::Appearance(p) => p.is_empty(),
        }
    }
}
impl HistoryDocument for EditorDocument {
    type Patch = EditorPatch;
    fn apply_history_patch(
        &mut self,
        patch: &Self::Patch,
    ) -> Result<ChangeImpact, PatchApplyError> {
        match patch {
            EditorPatch::Workspace(p) => self.workspace.apply_history_patch(p),
            EditorPatch::Appearance(p) => {
                p.apply(&mut self.appearance)
                    .map_err(|_| PatchApplyError::AuthoringMismatch { op_index: 0 })?;
                // Appearance advances authored revision but never changes map/collision revision.
                Ok(ChangeImpact::NONE)
            }
        }
    }
}
