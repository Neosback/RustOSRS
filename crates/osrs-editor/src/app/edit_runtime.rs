//! Application-level semantic edit transaction gateway.
//!
//! `EditorSession` remains the semantic authority. This runtime owns that session and centralizes
//! the checkpoint -> projection -> finalize/restore policy for interactive semantic transactions.
//! Read/query and source/project lifecycle operations are exposed explicitly; transaction lifecycle
//! primitives stay private to this runtime so callers cannot bypass gateway policy accidentally.

use crate::session::{EditCheckpoint, EditCheckpointError, EditorSession, RegionSyncOutcome};
use osrs_core::coords::{MapTile, RegionCoord};
use osrs_edit::{ChangeImpact, MapDocument, MapOp, MapPatch, MapRegionSnapshot, PatchApplyError};
use std::collections::BTreeSet;
use std::ops::Deref;
#[cfg(test)]
use std::convert::Infallible;

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

/// Exact semantic coordinates touched by one application transaction.
///
/// This remains renderer-independent. The world projection decides how far each class of source
/// mutation can propagate into disposable derived geometry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct EditFootprint {
    pub terrain_tiles: Vec<MapTile>,
    pub height_vertices: Vec<MapTile>,
    pub object_tiles: Vec<MapTile>,
}

impl EditFootprint {
    pub(super) fn new(
        terrain_tiles: impl IntoIterator<Item = MapTile>,
        height_vertices: impl IntoIterator<Item = MapTile>,
        object_tiles: impl IntoIterator<Item = MapTile>,
    ) -> Self {
        Self {
            terrain_tiles: normalize_tiles(terrain_tiles),
            height_vertices: normalize_tiles(height_vertices),
            object_tiles: normalize_tiles(object_tiles),
        }
    }

    fn from_patch(patch: &MapPatch) -> Self {
        let mut terrain_tiles = Vec::new();
        let mut height_vertices = Vec::new();
        let mut object_tiles = Vec::new();
        for op in patch.ops() {
            match op {
                MapOp::SetTerrain { coord, .. } => terrain_tiles.push(coord.tile),
                MapOp::SetHeight { coord, .. } => height_vertices.push(coord.corner),
                MapOp::AddObject { placement } | MapOp::RemoveObject { placement } => {
                    object_tiles.push(placement.position.tile);
                }
            }
        }
        Self::new(terrain_tiles, height_vertices, object_tiles)
    }

    fn into_touches(self) -> EditTouches {
        let regions = self
            .terrain_tiles
            .iter()
            .chain(&self.height_vertices)
            .chain(&self.object_tiles)
            .map(|tile| tile.split_region().0)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        EditTouches {
            regions,
            footprint: self,
        }
    }
}

/// Region summary plus the exact coordinate footprint that produced it.
///
/// Dereferencing preserves the historical `&[RegionCoord]` view for callers that only need region
/// membership, while projection invalidation can inspect the richer footprint without a parallel
/// side channel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct EditTouches {
    regions: Vec<RegionCoord>,
    pub footprint: EditFootprint,
}

impl Deref for EditTouches {
    type Target = [RegionCoord];

    fn deref(&self) -> &Self::Target {
        &self.regions
    }
}

impl PartialEq<Vec<RegionCoord>> for EditTouches {
    fn eq(&self, other: &Vec<RegionCoord>) -> bool {
        self.regions == *other
    }
}

/// Successful application-level semantic transaction metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EditReceipt {
    pub revision: u64,
    pub impact: ChangeImpact,
    pub touched_regions: EditTouches,
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

    #[cfg(test)]
    pub(super) fn can_undo(&self) -> bool {
        self.session.can_undo()
    }

    #[cfg(test)]
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

    /// Test fixture convenience that still exercises the production transaction gateway.
    #[cfg(test)]
    pub(super) fn commit(&mut self, patch: MapPatch) -> Result<(), EditExecutionError<Infallible>> {
        let _ = self.execute(patch, |_| Ok::<_, Infallible>(()))?;
        Ok(())
    }

    /// Test fixture convenience that still exercises the production replay gateway.
    #[cfg(test)]
    pub(super) fn undo(&mut self) -> Result<(), EditExecutionError<Infallible>> {
        let _ = self.replay(EditReplayDirection::Undo, EditFootprint::default(), |_| {
            Ok::<_, Infallible>(())
        })?;
        Ok(())
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
        let touches = EditFootprint::from_patch(&patch).into_touches();
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
            touches,
            checkpoint,
            projection,
        )
    }

    /// Replay one semantic Undo/Redo as an application transaction.
    ///
    /// `footprint` comes from the lightweight application history route because `EditorSession`
    /// intentionally keeps history patches opaque. The projection closure receives only the
    /// authoritative post-replay document; worker/UI side effects belong after this method returns
    /// `Committed`.
    pub(super) fn replay<T, E>(
        &mut self,
        direction: EditReplayDirection,
        footprint: EditFootprint,
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
            footprint.into_touches(),
            checkpoint,
            projection,
        )
    }

    fn finish_execution<T, E>(
        &mut self,
        revision: u64,
        impact: ChangeImpact,
        touched_regions: EditTouches,
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

fn normalize_tiles(tiles: impl IntoIterator<Item = MapTile>) -> Vec<MapTile> {
    tiles.into_iter().collect::<BTreeSet<_>>().into_iter().collect()
}

#[cfg(test)]
mod tests;
