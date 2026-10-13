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
    pub fn dirty_regions(&self) -> impl Iterator<Item = RegionCoord> + '_ {
        self.dirty_regions.iter().copied()
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
mod tests {
    use super::*;
    use osrs_core::{
        coords::{MapTile, SourcePlane},
        ids::FloorUnderlayId,
    };
    use osrs_edit::{HeightVertexCoord, MapPosition, TerrainTile};

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn vertex() -> HeightVertexCoord {
        HeightVertexCoord::new(MapTile::new(3200, 3200), plane())
    }

    fn position() -> MapPosition {
        MapPosition::new(MapTile::new(3200, 3200), plane())
    }

    fn region() -> RegionCoord {
        RegionCoord::new(50, 50)
    }

    fn session(height: i32) -> EditorSession {
        EditorSession::new(MapDocument::from_parts([], [(vertex(), height)], []))
    }

    fn snapshot(height: i32, underlay: u32) -> MapRegionSnapshot {
        MapRegionSnapshot::from_parts(
            region(),
            [(
                position(),
                TerrainTile {
                    underlay: Some(FloorUnderlayId::new(underlay)),
                    ..TerrainTile::default()
                },
            )],
            [(vertex(), height)],
            [],
        )
        .unwrap_or_else(|error| unreachable!("valid snapshot: {error:?}"))
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
    fn clean_region_ingestion_is_idempotent_and_replacement_advances_revision() {
        let mut session = EditorSession::default();
        let first = snapshot(-64, 7);

        assert_eq!(session.ingest_region(&first), RegionSyncOutcome::Changed);
        assert_eq!(session.revision(), 1);
        assert_eq!(session.document().height_at(vertex()), Some(-64));
        assert_eq!(
            session.document().terrain_at(position()).underlay,
            Some(FloorUnderlayId::new(7))
        );

        assert_eq!(session.ingest_region(&first), RegionSyncOutcome::Unchanged);
        assert_eq!(session.revision(), 1);

        let replacement = snapshot(-128, 8);
        assert_eq!(
            session.ingest_region(&replacement),
            RegionSyncOutcome::Changed
        );
        assert_eq!(session.revision(), 2);
        assert_eq!(session.document().height_at(vertex()), Some(-128));
        assert_eq!(
            session.document().terrain_at(position()).underlay,
            Some(FloorUnderlayId::new(8))
        );
    }

    #[test]
    fn unloading_clean_region_removes_it_without_touching_history() {
        let mut session = EditorSession::default();
        assert_eq!(
            session.ingest_region(&snapshot(-64, 7)),
            RegionSyncOutcome::Changed
        );
        assert_eq!(session.unload_region(region()), RegionSyncOutcome::Changed);
        assert_eq!(session.revision(), 2);
        assert_eq!(session.document().height_at(vertex()), None);
        assert_eq!(
            session.document().terrain_at(position()),
            TerrainTile::default()
        );
        assert!(!session.can_undo());
        assert!(!session.can_redo());
        assert_eq!(
            session.unload_region(region()),
            RegionSyncOutcome::Unchanged
        );
    }

    #[test]
    fn edited_region_is_not_overwritten_or_evicted_by_streaming() {
        let mut session = EditorSession::default();
        assert_eq!(
            session.ingest_region(&snapshot(0, 7)),
            RegionSyncOutcome::Changed
        );
        assert!(session.commit(height_patch(0, 64)).is_ok());
        assert_eq!(session.revision(), 2);

        assert_eq!(
            session.ingest_region(&snapshot(128, 8)),
            RegionSyncOutcome::PreservedDirty
        );
        assert_eq!(session.document().height_at(vertex()), Some(64));
        assert_eq!(
            session.document().terrain_at(position()).underlay,
            Some(FloorUnderlayId::new(7))
        );
        assert_eq!(session.revision(), 2);

        assert_eq!(
            session.unload_region(region()),
            RegionSyncOutcome::PreservedDirty
        );
        assert_eq!(session.document().height_at(vertex()), Some(64));
        assert_eq!(session.revision(), 2);
    }

    #[test]
    fn reset_clears_loaded_state_history_and_revision() {
        let mut session = EditorSession::default();
        assert_eq!(
            session.ingest_region(&snapshot(0, 7)),
            RegionSyncOutcome::Changed
        );
        assert!(session.commit(height_patch(0, 64)).is_ok());
        session.reset();

        assert_eq!(session.revision(), 0);
        assert_eq!(session.document().height_at(vertex()), None);
        assert_eq!(
            session.document().terrain_at(position()),
            TerrainTile::default()
        );
        assert!(!session.can_undo());
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
