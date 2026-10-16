use crate::{ChangeImpact, MapDocument, MapPatch, PatchApplyError};

/// One short-lived history transition that can either be finalized or rolled back exactly.
///
/// The token is intentionally opaque. It carries no map snapshot: rollback uses the same patch
/// that just moved between the history stacks, so cost stays proportional to the edit itself.
#[must_use = "a reversible history transaction must be finalized or rolled back"]
#[derive(Debug)]
pub struct EditHistoryTransaction {
    generation_before: u64,
    kind: TransactionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionKind {
    Commit,
    Undo,
    Redo,
}

/// Why a reversible history transaction could not be finalized or rolled back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryTransactionError {
    /// History changed after the transaction token was created.
    StaleTransaction,
    /// The authoritative document no longer accepts the exact rollback patch.
    Patch(PatchApplyError),
}

impl From<PatchApplyError> for HistoryTransactionError {
    fn from(error: PatchApplyError) -> Self {
        Self::Patch(error)
    }
}

/// Headless undo/redo history for authoritative semantic map edits.
///
/// Each successfully committed non-empty [`MapPatch`] becomes exactly one undo
/// entry, preserving the intended "one user gesture = one history step" rule.
/// Undo applies the patch's exact inverse; redo reapplies the original patch.
///
/// History mutations are failure-safe: if the document rejects an undo or redo
/// because authoritative state no longer matches the recorded patch, the history
/// stacks are restored to their prior state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchHistory<D: HistoryDocument> {
    undo: Vec<D::Patch>,
    redo: Vec<D::Patch>,
    generation: u64,
}

/// The existing map history and workspace history share this one implementation.
pub type EditHistory = PatchHistory<MapDocument>;

pub trait ReversiblePatch: Clone + std::fmt::Debug + Eq {
    fn inverse(&self) -> Self;
    fn is_empty(&self) -> bool;
}
pub trait HistoryDocument: Clone + std::fmt::Debug + Eq {
    type Patch: ReversiblePatch;
    fn apply_history_patch(&mut self, patch: &Self::Patch)
    -> Result<ChangeImpact, PatchApplyError>;
}
impl ReversiblePatch for MapPatch {
    fn inverse(&self) -> Self {
        MapPatch::inverse(self)
    }
    fn is_empty(&self) -> bool {
        MapPatch::is_empty(self)
    }
}
impl HistoryDocument for MapDocument {
    type Patch = MapPatch;
    fn apply_history_patch(&mut self, patch: &MapPatch) -> Result<ChangeImpact, PatchApplyError> {
        self.apply_patch(patch)
    }
}
impl<D: HistoryDocument> Default for PatchHistory<D> {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            generation: 0,
        }
    }
}

impl<D: HistoryDocument> PatchHistory<D> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a new logical edit and record it as one undo step.
    ///
    /// This compatibility route finalizes immediately. Call [`Self::begin_commit`] when a
    /// downstream projection must succeed before the edit becomes final.
    pub fn commit(
        &mut self,
        document: &mut D,
        patch: D::Patch,
    ) -> Result<ChangeImpact, PatchApplyError> {
        let (impact, transaction) = self.begin_commit(document, patch)?;
        if let Some(transaction) = transaction {
            self.finalize(transaction)
                .unwrap_or_else(|error| unreachable!("fresh history transaction: {error:?}"));
        }
        Ok(impact)
    }

    /// Apply a commit while retaining the ability to restore the exact prior history state.
    ///
    /// Redo history is deliberately retained until [`Self::finalize`] so a failed downstream
    /// projection can roll the candidate back without having already destroyed the previous redo
    /// branch. Empty patches remain no-ops and produce no transaction token.
    pub fn begin_commit(
        &mut self,
        document: &mut D,
        patch: D::Patch,
    ) -> Result<(ChangeImpact, Option<EditHistoryTransaction>), PatchApplyError> {
        let impact = document.apply_history_patch(&patch)?;
        if patch.is_empty() {
            return Ok((impact, None));
        }

        self.undo.push(patch);
        let transaction = self.begin_transaction(TransactionKind::Commit);
        Ok((impact, Some(transaction)))
    }

    /// Undo the most recent semantic edit and finalize it immediately.
    pub fn undo(&mut self, document: &mut D) -> Result<Option<ChangeImpact>, PatchApplyError> {
        let Some((impact, transaction)) = self.begin_undo(document)? else {
            return Ok(None);
        };
        self.finalize(transaction)
            .unwrap_or_else(|error| unreachable!("fresh history transaction: {error:?}"));
        Ok(Some(impact))
    }

    /// Undo while retaining the ability to restore the exact pre-undo history state.
    pub fn begin_undo(
        &mut self,
        document: &mut D,
    ) -> Result<Option<(ChangeImpact, EditHistoryTransaction)>, PatchApplyError> {
        let Some(patch) = self.undo.pop() else {
            return Ok(None);
        };

        let inverse = patch.inverse();
        match document.apply_history_patch(&inverse) {
            Ok(impact) => {
                self.redo.push(patch);
                let transaction = self.begin_transaction(TransactionKind::Undo);
                Ok(Some((impact, transaction)))
            }
            Err(error) => {
                self.undo.push(patch);
                Err(error)
            }
        }
    }

    /// Redo the most recently undone semantic edit and finalize it immediately.
    pub fn redo(&mut self, document: &mut D) -> Result<Option<ChangeImpact>, PatchApplyError> {
        let Some((impact, transaction)) = self.begin_redo(document)? else {
            return Ok(None);
        };
        self.finalize(transaction)
            .unwrap_or_else(|error| unreachable!("fresh history transaction: {error:?}"));
        Ok(Some(impact))
    }

    /// Redo while retaining the ability to restore the exact pre-redo history state.
    pub fn begin_redo(
        &mut self,
        document: &mut D,
    ) -> Result<Option<(ChangeImpact, EditHistoryTransaction)>, PatchApplyError> {
        let Some(patch) = self.redo.pop() else {
            return Ok(None);
        };

        match document.apply_history_patch(&patch) {
            Ok(impact) => {
                self.undo.push(patch);
                let transaction = self.begin_transaction(TransactionKind::Redo);
                Ok(Some((impact, transaction)))
            }
            Err(error) => {
                self.redo.push(patch);
                Err(error)
            }
        }
    }

    /// Make a reversible history transition final.
    ///
    /// Only a new commit has deferred work: its prior redo branch is invalidated here, after the
    /// caller has confirmed downstream projection. Undo and redo already have their final stack
    /// shape and only need the transaction token consumed.
    pub fn finalize(
        &mut self,
        transaction: EditHistoryTransaction,
    ) -> Result<(), HistoryTransactionError> {
        self.validate_transaction(&transaction)?;
        if transaction.kind == TransactionKind::Commit {
            self.redo.clear();
        }
        Ok(())
    }

    /// Restore the exact document/history state from immediately before `transaction`.
    pub fn rollback(
        &mut self,
        document: &mut D,
        transaction: EditHistoryTransaction,
    ) -> Result<ChangeImpact, HistoryTransactionError> {
        self.validate_transaction(&transaction)?;

        let impact = match transaction.kind {
            TransactionKind::Commit => {
                let Some(patch) = self.undo.last() else {
                    return Err(HistoryTransactionError::StaleTransaction);
                };
                let inverse = patch.inverse();
                let impact = document.apply_history_patch(&inverse)?;
                self.undo.pop();
                impact
            }
            TransactionKind::Undo => {
                let Some(patch) = self.redo.last() else {
                    return Err(HistoryTransactionError::StaleTransaction);
                };
                let impact = document.apply_history_patch(patch)?;
                let patch = self
                    .redo
                    .pop()
                    .unwrap_or_else(|| unreachable!("validated redo entry exists"));
                self.undo.push(patch);
                impact
            }
            TransactionKind::Redo => {
                let Some(patch) = self.undo.last() else {
                    return Err(HistoryTransactionError::StaleTransaction);
                };
                let inverse = patch.inverse();
                let impact = document.apply_history_patch(&inverse)?;
                let patch = self
                    .undo
                    .pop()
                    .unwrap_or_else(|| unreachable!("validated undo entry exists"));
                self.redo.push(patch);
                impact
            }
        };

        self.generation = transaction.generation_before;
        Ok(impact)
    }

    pub fn next_undo(&self) -> Option<&D::Patch> {
        self.undo.last()
    }
    pub fn next_redo(&self) -> Option<&D::Patch> {
        self.redo.last()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub fn clear(&mut self) {
        if self.undo.is_empty() && self.redo.is_empty() {
            return;
        }
        self.undo.clear();
        self.redo.clear();
        self.bump_generation();
    }

    fn begin_transaction(&mut self, kind: TransactionKind) -> EditHistoryTransaction {
        let generation_before = self.generation;
        self.bump_generation();
        EditHistoryTransaction {
            generation_before,
            kind,
        }
    }

    fn validate_transaction(
        &self,
        transaction: &EditHistoryTransaction,
    ) -> Result<(), HistoryTransactionError> {
        if self.generation == transaction.generation_before.wrapping_add(1) {
            Ok(())
        } else {
            Err(HistoryTransactionError::StaleTransaction)
        }
    }

    fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        HeightVertexCoord, LocKind, MapOp, MapPosition, ObjectOrientation, ObjectPlacement,
        TerrainTile,
    };
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_core::ids::{FloorUnderlayId, ObjectId};

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn position(x: i32, y: i32) -> MapPosition {
        MapPosition::new(MapTile::new(x, y), plane())
    }

    fn height(x: i32, y: i32) -> HeightVertexCoord {
        HeightVertexCoord::new(MapTile::new(x, y), plane())
    }

    fn object() -> ObjectPlacement {
        ObjectPlacement {
            object_id: ObjectId::new(100),
            position: position(3200, 3200),
            kind: LocKind::new(10).unwrap_or_else(|| unreachable!("loc kind is valid")),
            orientation: ObjectOrientation::new(2)
                .unwrap_or_else(|| unreachable!("orientation is valid")),
        }
    }

    fn composite_patch(vertex: HeightVertexCoord) -> MapPatch {
        MapPatch::new([
            MapOp::SetTerrain {
                coord: position(3200, 3200),
                before: TerrainTile::default(),
                after: TerrainTile {
                    underlay: Some(FloorUnderlayId::new(7)),
                    ..TerrainTile::default()
                },
            },
            MapOp::SetHeight {
                coord: vertex,
                before: 32,
                after: 64,
            },
            MapOp::AddObject {
                placement: object(),
            },
        ])
    }

    #[test]
    fn one_patch_is_one_history_entry() {
        let vertex = height(3200, 3200);
        let mut document = MapDocument::from_parts([], [(vertex, 32)], []);
        let mut history = EditHistory::new();
        let patch = composite_patch(vertex);

        assert_eq!(history.commit(&mut document, patch), Ok(patch_impact()));
        assert_eq!(history.undo_len(), 1);
        assert_eq!(history.redo_len(), 0);

        assert_eq!(history.undo(&mut document), Ok(Some(patch_impact())));
        assert_eq!(
            document.terrain_at(position(3200, 3200)),
            TerrainTile::default()
        );
        assert_eq!(document.height_at(vertex), Some(32));
        assert!(!document.contains_object(object()));
        assert_eq!(history.undo_len(), 0);
        assert_eq!(history.redo_len(), 1);
    }

    #[test]
    fn undo_and_redo_restore_exact_document_states() {
        let vertex = height(3200, 3200);
        let mut document = MapDocument::from_parts([], [(vertex, 32)], []);
        let original = document.clone();
        let mut history = EditHistory::new();
        let patch = composite_patch(vertex);

        assert_eq!(history.commit(&mut document, patch), Ok(patch_impact()));
        let edited = document.clone();

        assert_eq!(history.undo(&mut document), Ok(Some(patch_impact())));
        assert_eq!(document, original);
        assert!(history.can_redo());

        assert_eq!(history.redo(&mut document), Ok(Some(patch_impact())));
        assert_eq!(document, edited);
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn successful_new_commit_clears_redo_history() {
        let vertex = height(3200, 3200);
        let mut document = MapDocument::from_parts([], [(vertex, 0)], []);
        let mut history = EditHistory::new();
        let first = MapPatch::new([MapOp::SetHeight {
            coord: vertex,
            before: 0,
            after: 64,
        }]);

        assert!(history.commit(&mut document, first).is_ok());
        assert!(history.undo(&mut document).is_ok());
        assert_eq!(history.redo_len(), 1);

        let alternate = MapPatch::new([MapOp::SetHeight {
            coord: vertex,
            before: 0,
            after: 32,
        }]);
        assert!(history.commit(&mut document, alternate).is_ok());

        assert_eq!(document.height_at(vertex), Some(32));
        assert_eq!(history.undo_len(), 1);
        assert_eq!(history.redo_len(), 0);
        assert!(!history.can_redo());
    }

    #[test]
    fn failed_commit_changes_neither_document_nor_history() {
        let vertex = height(3200, 3200);
        let mut document = MapDocument::from_parts([], [(vertex, 32)], []);
        let original = document.clone();
        let mut history = EditHistory::new();
        let invalid = MapPatch::new([MapOp::SetHeight {
            coord: vertex,
            before: 999,
            after: 64,
        }]);

        assert!(history.commit(&mut document, invalid).is_err());
        assert_eq!(document, original);
        assert_eq!(history.undo_len(), 0);
        assert_eq!(history.redo_len(), 0);
    }

    #[test]
    fn failed_undo_restores_history_stack() {
        let vertex = height(3200, 3200);
        let mut document = MapDocument::from_parts([], [(vertex, 0)], []);
        let mut history = EditHistory::new();
        let recorded = MapPatch::new([MapOp::SetHeight {
            coord: vertex,
            before: 0,
            after: 64,
        }]);

        assert!(history.commit(&mut document, recorded).is_ok());

        let out_of_band = MapPatch::new([MapOp::SetHeight {
            coord: vertex,
            before: 64,
            after: 128,
        }]);
        assert!(document.apply_patch(&out_of_band).is_ok());

        assert!(history.undo(&mut document).is_err());
        assert_eq!(document.height_at(vertex), Some(128));
        assert_eq!(history.undo_len(), 1);
        assert_eq!(history.redo_len(), 0);
    }

    #[test]
    fn empty_commit_does_not_create_or_destroy_history() {
        let vertex = height(3200, 3200);
        let mut document = MapDocument::from_parts([], [(vertex, 0)], []);
        let mut history = EditHistory::new();
        let patch = MapPatch::new([MapOp::SetHeight {
            coord: vertex,
            before: 0,
            after: 64,
        }]);

        assert!(history.commit(&mut document, patch).is_ok());
        assert!(history.undo(&mut document).is_ok());
        assert_eq!(history.redo_len(), 1);

        assert_eq!(
            history.commit(&mut document, MapPatch::default()),
            Ok(ChangeImpact::NONE)
        );
        assert_eq!(history.undo_len(), 0);
        assert_eq!(history.redo_len(), 1);
    }

    fn patch_impact() -> ChangeImpact {
        ChangeImpact::TERRAIN_GEOMETRY
            | ChangeImpact::TERRAIN_MATERIAL
            | ChangeImpact::NORMALS
            | ChangeImpact::LIGHTING
            | ChangeImpact::LOCATIONS
            | ChangeImpact::COLLISION
            | ChangeImpact::BOUNDS
    }
}
