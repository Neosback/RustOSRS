use osrs_core::coords::RegionCoord;
use osrs_edit::{
    ChangeImpact, EditHistory, MapDocument, MapOp, MapPatch, MapRegionSnapshot, PatchApplyError,
};
use std::collections::BTreeSet;

/// One successful semantic edit state transition in the editor session.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditCommit {
    pub revision: u64,
    pub impact: ChangeImpact,
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
            loaded_regions: BTreeSet::new(),
            dirty_regions: BTreeSet::new(),
        }
    }

    /// Regions an interactive edit has touched (and that therefore keep their document state).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn dirty_regions(&self) -> impl Iterator<Item = RegionCoord> + '_ {
        self.dirty_regions.iter().copied()
    }

    /// Whether the region's semantic state is currently in the document.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn is_loaded(&self, region: RegionCoord) -> bool {
        self.loaded_regions.contains(&region)
    }

    /// Forget all undo/redo entries (opening a project starts from a clean history).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn clear_history(&mut self) {
        self.history = EditHistory::new();
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

    /// Apply one logical semantic edit.
    ///
    /// Empty patches remain true no-ops: they do not advance the session revision and do not
    /// disturb redo history. Regions touched by a successful non-empty edit become streaming-safe
    /// dirty regions and are never replaced/evicted by later region churn.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn commit(&mut self, patch: MapPatch) -> Result<Option<EditCommit>, PatchApplyError> {
        let is_empty = patch.is_empty();
        let touched_regions = (!is_empty).then(|| patch_regions(&patch));
        let impact = self.history.commit(&mut self.document, patch)?;
        if is_empty {
            return Ok(None);
        }
        if let Some(regions) = touched_regions {
            self.dirty_regions.extend(regions);
        }
        Ok(Some(self.record_edit(impact)))
    }

    /// Undo the most recent semantic edit.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn undo(&mut self) -> Result<Option<EditCommit>, PatchApplyError> {
        self.history
            .undo(&mut self.document)
            .map(|impact| impact.map(|impact| self.record_edit(impact)))
    }

    /// Redo the most recently undone semantic edit.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn redo(&mut self) -> Result<Option<EditCommit>, PatchApplyError> {
        self.history
            .redo(&mut self.document)
            .map(|impact| impact.map(|impact| self.record_edit(impact)))
    }

    fn record_edit(&mut self, impact: ChangeImpact) -> EditCommit {
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
