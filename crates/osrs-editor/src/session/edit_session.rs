use osrs_core::coords::RegionCoord;
use osrs_edit::{
    ChangeImpact, EditHistory, EditHistoryTransaction, HistoryTransactionError, MapDocument, MapOp,
    MapPatch, MapRegionSnapshot, PatchApplyError,
};
use std::collections::BTreeSet;

/// One successful semantic edit state transition in the editor session.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditCommit {
    pub revision: u64,
    pub impact: ChangeImpact,
}

/// Opaque state required to either finalize or exactly restore one semantic edit transition.
///
/// The checkpoint stores only history-operation identity and small session metadata. It does not
/// clone the authoritative map document, so rollback cost remains proportional to the edit.
#[must_use = "an edit checkpoint must be finalized or restored"]
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
pub struct EditCheckpoint {
    history: EditHistoryTransaction,
    revision_before: u64,
    edit_revision_before: u64,
    loaded_regions_before: BTreeSet<RegionCoord>,
    newly_dirty_regions: BTreeSet<RegionCoord>,
}

/// Why an edit checkpoint can no longer be finalized or restored safely.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditCheckpointError {
    /// Session state changed after the checkpointed semantic operation was applied.
    StaleSession,
    /// The underlying history transaction is stale or its exact inverse no longer applies.
    History(HistoryTransactionError),
}

impl From<HistoryTransactionError> for EditCheckpointError {
    fn from(error: HistoryTransactionError) -> Self {
        Self::History(error)
    }
}

/// Result of synchronizing one streamed region with authoritative session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionSyncOutcome {
    /// Authoritative semantic state changed.
    Changed,
    /// The incoming/removal operation was semantically identical to current state.
    Unchanged,
    /// A session-authored edit has touched this region, so streaming may not overwrite/evict it.
    PreservedDirty,
}

/// Persistent authoritative semantic editing state for one editor session.
///
/// `MapDocument` is the source of truth. `EditHistory` records reversible semantic patches.
/// Streamed region hydration is history-free but still advances `revision` when it changes the
/// authoritative document. Once an interactive semantic edit touches a region, streaming may no
/// longer replace or evict that region's document state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditorSession {
    document: MapDocument,
    history: EditHistory,
    revision: u64,
    /// Advances only on real edits (commit, undo, redo, applied projects), never on streaming.
    edit_revision: u64,
    loaded_regions: BTreeSet<RegionCoord>,
    dirty_regions: BTreeSet<RegionCoord>,
}

impl EditorSession {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(document: MapDocument) -> Self {
        Self {
            document,
            history: EditHistory::new(),
            revision: 0,
            edit_revision: 0,
            loaded_regions: BTreeSet::new(),
            dirty_regions: BTreeSet::new(),
        }
    }

    /// Regions an interactive edit has touched (and that therefore keep their document state).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn dirty_regions(&self) -> impl Iterator<Item = RegionCoord> + '_ {
        self.dirty_regions.iter().copied()
    }

    /// Counts real edits only; compare with a saved value to know whether work is unsaved.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn edit_revision(&self) -> u64 {
        self.edit_revision
    }

    /// Apply a patch that is not an interactive edit (a project being opened): no history entry,
    /// but the touched regions become streaming-safe dirty regions and the revision advances.
    /// An empty patch is a no-op that returns `Ok(false)`.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn apply_untracked(&mut self, patch: &MapPatch) -> Result<bool, PatchApplyError> {
        if patch.is_empty() {
            return Ok(false);
        }
        self.document.apply_patch(patch)?;
        self.dirty_regions.extend(patch_regions(patch));
        self.edit_revision += 1;
        self.advance_revision();
        Ok(true)
    }

    /// Drop all semantic state when switching to a different cache/source.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn document(&self) -> &MapDocument {
        &self.document
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Hydrate or refresh one clean streamed region without creating user-edit history.
    ///
    /// Repeated identical region builds are true no-ops. A region touched by a semantic edit is
    /// protected from later worker snapshots, because the session is authoritative once edited.
    pub fn ingest_region(&mut self, snapshot: &MapRegionSnapshot) -> RegionSyncOutcome {
        let region = snapshot.region();
        self.loaded_regions.insert(region);
        if self.dirty_regions.contains(&region) {
            return RegionSyncOutcome::PreservedDirty;
        }
        if self.document.replace_region(snapshot) {
            self.advance_revision();
            RegionSyncOutcome::Changed
        } else {
            RegionSyncOutcome::Unchanged
        }
    }

    /// Evict one clean streamed region from the authoritative working set.
    ///
    /// Dirty regions remain in the semantic document even after their renderer projection leaves
    /// memory, so camera streaming can never discard an authored edit.
    pub fn unload_region(&mut self, region: RegionCoord) -> RegionSyncOutcome {
        if !self.loaded_regions.remove(&region) {
            return RegionSyncOutcome::Unchanged;
        }
        if self.dirty_regions.contains(&region) {
            return RegionSyncOutcome::PreservedDirty;
        }
        if self.document.remove_region(region) {
            self.advance_revision();
            RegionSyncOutcome::Changed
        } else {
            RegionSyncOutcome::Unchanged
        }
    }

    /// Apply one logical semantic edit and finalize it immediately.
    ///
    /// Call [`Self::commit_checkpointed`] when downstream projection must succeed before the
    /// semantic edit becomes final.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn commit(&mut self, patch: MapPatch) -> Result<Option<EditCommit>, PatchApplyError> {
        let Some((commit, checkpoint)) = self.commit_checkpointed(patch)? else {
            return Ok(None);
        };
        self.finalize_checkpoint(checkpoint)
            .unwrap_or_else(|error| unreachable!("fresh edit checkpoint: {error:?}"));
        Ok(Some(commit))
    }

    /// Apply one logical semantic edit while retaining an exact rollback checkpoint.
    ///
    /// Redo invalidation is deferred until [`Self::finalize_checkpoint`]. If downstream projection
    /// fails, [`Self::restore_checkpoint`] returns the document, history, revision counters, and
    /// dirty-region ownership to their exact pre-transaction state.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn commit_checkpointed(
        &mut self,
        patch: MapPatch,
    ) -> Result<Option<(EditCommit, EditCheckpoint)>, PatchApplyError> {
        let touched_regions = patch_regions(&patch);
        let newly_dirty_regions = touched_regions
            .difference(&self.dirty_regions)
            .copied()
            .collect();
        let revision_before = self.revision;
        let edit_revision_before = self.edit_revision;
        let loaded_regions_before = self.loaded_regions.clone();

        let (impact, transaction) = self.history.begin_commit(&mut self.document, patch)?;
        let Some(history) = transaction else {
            return Ok(None);
        };

        self.dirty_regions.extend(touched_regions);
        let commit = self.record_edit(impact);
        let checkpoint = EditCheckpoint {
            history,
            revision_before,
            edit_revision_before,
            loaded_regions_before,
            newly_dirty_regions,
        };
        Ok(Some((commit, checkpoint)))
    }

    /// Undo the most recent semantic edit and finalize it immediately.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn undo(&mut self) -> Result<Option<EditCommit>, PatchApplyError> {
        let Some((commit, checkpoint)) = self.undo_checkpointed()? else {
            return Ok(None);
        };
        self.finalize_checkpoint(checkpoint)
            .unwrap_or_else(|error| unreachable!("fresh edit checkpoint: {error:?}"));
        Ok(Some(commit))
    }

    /// Undo while retaining an exact rollback checkpoint for downstream projection.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn undo_checkpointed(
        &mut self,
    ) -> Result<Option<(EditCommit, EditCheckpoint)>, PatchApplyError> {
        let revision_before = self.revision;
        let edit_revision_before = self.edit_revision;
        let loaded_regions_before = self.loaded_regions.clone();
        let Some((impact, history)) = self.history.begin_undo(&mut self.document)? else {
            return Ok(None);
        };
        let commit = self.record_edit(impact);
        let checkpoint = EditCheckpoint {
            history,
            revision_before,
            edit_revision_before,
            loaded_regions_before,
            newly_dirty_regions: BTreeSet::new(),
        };
        Ok(Some((commit, checkpoint)))
    }

    /// Redo the most recently undone semantic edit and finalize it immediately.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn redo(&mut self) -> Result<Option<EditCommit>, PatchApplyError> {
        let Some((commit, checkpoint)) = self.redo_checkpointed()? else {
            return Ok(None);
        };
        self.finalize_checkpoint(checkpoint)
            .unwrap_or_else(|error| unreachable!("fresh edit checkpoint: {error:?}"));
        Ok(Some(commit))
    }

    /// Redo while retaining an exact rollback checkpoint for downstream projection.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn redo_checkpointed(
        &mut self,
    ) -> Result<Option<(EditCommit, EditCheckpoint)>, PatchApplyError> {
        let revision_before = self.revision;
        let edit_revision_before = self.edit_revision;
        let loaded_regions_before = self.loaded_regions.clone();
        let Some((impact, history)) = self.history.begin_redo(&mut self.document)? else {
            return Ok(None);
        };
        let commit = self.record_edit(impact);
        let checkpoint = EditCheckpoint {
            history,
            revision_before,
            edit_revision_before,
            loaded_regions_before,
            newly_dirty_regions: BTreeSet::new(),
        };
        Ok(Some((commit, checkpoint)))
    }

    /// Accept a checkpointed edit after downstream projection succeeds.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn finalize_checkpoint(
        &mut self,
        checkpoint: EditCheckpoint,
    ) -> Result<(), EditCheckpointError> {
        self.validate_checkpoint(&checkpoint)?;
        self.history.finalize(checkpoint.history)?;
        Ok(())
    }

    /// Make a checkpointed edit semantically invisible after downstream projection fails.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn restore_checkpoint(
        &mut self,
        checkpoint: EditCheckpoint,
    ) -> Result<(), EditCheckpointError> {
        self.validate_checkpoint(&checkpoint)?;
        self.history
            .rollback(&mut self.document, checkpoint.history)?;
        self.revision = checkpoint.revision_before;
        self.edit_revision = checkpoint.edit_revision_before;
        for region in checkpoint.newly_dirty_regions {
            self.dirty_regions.remove(&region);
        }
        Ok(())
    }

    fn validate_checkpoint(&self, checkpoint: &EditCheckpoint) -> Result<(), EditCheckpointError> {
        if self.revision != checkpoint.revision_before + 1
            || self.edit_revision != checkpoint.edit_revision_before + 1
            || self.loaded_regions != checkpoint.loaded_regions_before
        {
            return Err(EditCheckpointError::StaleSession);
        }
        Ok(())
    }

    fn record_edit(&mut self, impact: ChangeImpact) -> EditCommit {
        self.edit_revision += 1;
        self.advance_revision();
        EditCommit {
            revision: self.revision,
            impact,
        }
    }

    fn advance_revision(&mut self) {
        self.revision += 1;
    }
}

fn patch_regions(patch: &MapPatch) -> BTreeSet<RegionCoord> {
    patch
        .ops()
        .iter()
        .map(|op| match op {
            MapOp::SetTerrain { coord, .. } => coord.tile.split_region().0,
            MapOp::SetHeight { coord, .. } => coord.corner.split_region().0,
            MapOp::AddObject { placement } | MapOp::RemoveObject { placement } => {
                placement.position.tile.split_region().0
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
