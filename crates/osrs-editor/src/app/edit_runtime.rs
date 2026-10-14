//! Application-level semantic edit transaction gateway.
//!
//! `EditorSession` remains the semantic authority. This runtime owns that session and centralizes
//! the checkpoint -> projection -> finalize/restore policy for interactive semantic transactions.
//! Read/query and source/project lifecycle operations are exposed explicitly; transaction lifecycle
//! primitives stay private to this runtime so callers cannot bypass gateway policy accidentally.

use crate::session::{
    EditCheckpoint, EditCheckpointError, EditorSession, RegionSyncOutcome,
};
use osrs_core::coords::RegionCoord;
use osrs_edit::{
    ChangeImpact, MapDocument, MapOp, MapPatch, MapRegionSnapshot, PatchApplyError,
};
use std::collections::BTreeSet;

/// Application owner of the authoritative semantic edit session.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct EditRuntime {
    session: EditorSession,
}

/// Which semantic history transition the gateway should replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditReplayDirection {
    Undo,
    Redo,
}

/// Successful application-level semantic transaction metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EditReceipt {
    pub revision: u64,
    pub impact: ChangeImpact,
    pub touched_regions: Vec<RegionCoord>,
}

/// Result of executing one semantic transition through the application gateway.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum EditExecution<T> {
    /// No semantic transition occurred, so no projection/finalization work ran.
    NoChange,
    /// Semantic mutation, projection, and checkpoint finalization all succeeded.
    Committed { receipt: EditReceipt, projection: T },
}

/// Failure while executing one semantic transition through the application gateway.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum EditExecutionError<E> {
    /// The semantic commit/replay was rejected before projection ran.
    Semantic(PatchApplyError),
    /// Projection failed and the semantic checkpoint was restored exactly.
    Projection(E),
    /// Projection failed, then exact semantic restoration also failed.
    Rollback {
        projection: E,
        rollback: EditCheckpointError,
    },
    /// Projection succeeded but the semantic checkpoint could not be finalized.
    Finalize(EditCheckpointError),
}

impl EditRuntime {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn new(session: EditorSession) -> Self {
        Self { session }
    }

    /// Authoritative semantic document for read-only queries and pure planning.
    pub(super) fn document(&self) -> &MapDocument {
        self.session.document()
    }

    /// Revision of all authoritative semantic state transitions, including streaming changes.
    pub(super) fn revision(&self) -> u64 {
        self.session.revision()
    }

    /// Revision of authored/project edits only; streaming hydration does not advance it.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(super) fn edit_revision(&self) -> u64 {
        self.session.edit_revision()
    }

    /// Regions protected from source streaming because authored/project state has touched them.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(super) fn dirty_regions(&self) -> impl Iterator<Item = RegionCoord> + '_ {
        self.session.dirty_regions()
    }

    pub(super) fn can_undo(&self) -> bool {
        self.session.can_undo()
    }

    pub(super) fn can_redo(&self) -> bool {
        self.session.can_redo()
    }

    /// Apply a project diff outside interactive history while preserving semantic authority.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(super) fn apply_untracked(&mut self, patch: &MapPatch) -> Result<bool, PatchApplyError> {
        self.session.apply_untracked(patch)
    }

    /// Replace all semantic state when the backing cache/source changes.
    pub(super) fn reset(&mut self) {
        self.session.reset();
    }

    /// Synchronize one clean streamed region into authoritative semantic state.
    pub(super) fn ingest_region(&mut self, snapshot: &MapRegionSnapshot) -> RegionSyncOutcome {
        self.session.ingest_region(snapshot)
    }

    /// Remove one clean streamed region from authoritative semantic residency.
    pub(super) fn unload_region(&mut self, region: RegionCoord) -> RegionSyncOutcome {
        self.session.unload_region(region)
    }

    /// Execute one `MapPatch` as an application transaction.
    ///
    /// The projection closure receives only the authoritative post-commit `MapDocument`. It cannot
    /// mutate `EditorSession`, which keeps checkpoint ownership and lifecycle policy inside this
    /// runtime. A projection error restores the exact pre-transaction semantic state.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn execute<T, E>(
        &mut self,
        patch: MapPatch,
        project: impl FnOnce(&MapDocument) -> Result<T, E>,
    ) -> Result<EditExecution<T>, EditExecutionError<E>> {
        let touched_regions = touched_regions(&patch);
        let Some((commit, checkpoint)) = self
            .session
            .commit_checkpointed(patch)
            .map_err(EditExecutionError::Semantic)?
        else {
            return Ok(EditExecution::NoChange);
        };

        let projection = project(self.session.document());
        self.finish_execution(
            commit.revision,
            commit.impact,
            touched_regions,
            checkpoint,
            projection,
        )
    }

    /// Replay one semantic Undo/Redo as an application transaction.
    ///
    /// `touched_regions` comes from the lightweight application history route because
    /// `EditorSession` intentionally keeps history patches opaque. The projection closure receives
    /// only the authoritative post-replay document; worker/UI side effects belong after this method
    /// returns `Committed`.
    pub(super) fn replay<T, E>(
        &mut self,
        direction: EditReplayDirection,
        touched_regions: Vec<RegionCoord>,
        project: impl FnOnce(&MapDocument) -> Result<T, E>,
    ) -> Result<EditExecution<T>, EditExecutionError<E>> {
        let replay = match direction {
            EditReplayDirection::Undo => self.session.undo_checkpointed(),
            EditReplayDirection::Redo => self.session.redo_checkpointed(),
        }
        .map_err(EditExecutionError::Semantic)?;
        let Some((commit, checkpoint)) = replay else {
            return Ok(EditExecution::NoChange);
        };

        let projection = project(self.session.document());
        self.finish_execution(
            commit.revision,
            commit.impact,
            normalize_regions(touched_regions),
            checkpoint,
            projection,
        )
    }

    fn finish_execution<T, E>(
        &mut self,
        revision: u64,
        impact: ChangeImpact,
        touched_regions: Vec<RegionCoord>,
        checkpoint: EditCheckpoint,
        projection: Result<T, E>,
    ) -> Result<EditExecution<T>, EditExecutionError<E>> {
        match projection {
            Ok(projection) => {
                self.session
                    .finalize_checkpoint(checkpoint)
                    .map_err(EditExecutionError::Finalize)?;
                Ok(EditExecution::Committed {
                    receipt: EditReceipt {
                        revision,
                        impact,
                        touched_regions,
                    },
                    projection,
                })
            }
            Err(projection) => match self.session.restore_checkpoint(checkpoint) {
                Ok(()) => Err(EditExecutionError::Projection(projection)),
                Err(rollback) => Err(EditExecutionError::Rollback {
                    projection,
                    rollback,
                }),
            },
        }
    }
}

fn normalize_regions(regions: Vec<RegionCoord>) -> Vec<RegionCoord> {
    regions
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn touched_regions(patch: &MapPatch) -> Vec<RegionCoord> {
    let mut regions = BTreeSet::new();
    for op in patch.ops() {
        let tile = match op {
            MapOp::SetTerrain { coord, .. } => coord.tile,
            MapOp::SetHeight { coord, .. } => coord.corner,
            MapOp::AddObject { placement } | MapOp::RemoveObject { placement } => {
                placement.position.tile
            }
        };
        regions.insert(tile.split_region().0);
    }
    regions.into_iter().collect()
}

#[cfg(test)]
mod tests;
