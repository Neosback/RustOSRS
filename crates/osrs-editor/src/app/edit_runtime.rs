//! Application-level semantic edit transaction gateway.
//!
//! `EditorSession` remains the semantic authority. This runtime owns that session and centralizes
//! the checkpoint -> projection -> finalize/restore policy for routes migrated to the gateway.
//! Existing routes still reach the wrapped session through `Deref`/`DerefMut` during the staged
//! migration; those compatibility escapes are temporary and should shrink as routes move here.

use crate::session::{EditCheckpoint, EditCheckpointError, EditorSession};
use osrs_core::coords::RegionCoord;
use osrs_edit::{ChangeImpact, MapDocument, MapOp, MapPatch, PatchApplyError};
use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};

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

/// Transitional compatibility for routes not yet migrated to the contained `EditRuntime` API.
///
/// This prevents a flag-day rewrite while keeping exactly one owned `EditorSession`. Remove these
/// broad escapes once remaining session access has narrow runtime methods.
impl Deref for EditRuntime {
    type Target = EditorSession;

    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

impl DerefMut for EditRuntime {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.session
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
mod tests {
    use super::*;
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_edit::{HeightVertexCoord, MapDocument, MapOp};

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn vertex(x: i32, y: i32) -> HeightVertexCoord {
        HeightVertexCoord::new(MapTile::new(x, y), plane())
    }

    fn height_patch(coord: HeightVertexCoord, before: i32, after: i32) -> MapPatch {
        MapPatch::new([MapOp::SetHeight {
            coord,
            before,
            after,
        }])
    }

    fn runtime_with_redo() -> (EditRuntime, HeightVertexCoord) {
        let prior = vertex(3200, 3200);
        let edited = vertex(3264, 3200);
        let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
            [],
            [(prior, 0), (edited, 0)],
            [],
        )));
        runtime
            .commit(height_patch(prior, 0, 32))
            .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
        runtime
            .undo()
            .unwrap_or_else(|error| unreachable!("valid setup undo: {error:?}"));
        assert!(runtime.can_redo());
        (runtime, edited)
    }

    #[test]
    fn successful_execution_finalizes_once_and_returns_receipt() {
        let (mut runtime, edited) = runtime_with_redo();
        let revision_before = runtime.revision();
        let result = runtime
            .execute(height_patch(edited, 0, 64), |document| {
                assert_eq!(document.height_at(edited), Some(64));
                Ok::<_, &'static str>("projected")
            })
            .unwrap_or_else(|error| unreachable!("gateway execution succeeds: {error:?}"));

        let EditExecution::Committed {
            receipt,
            projection,
        } = result
        else {
            unreachable!("non-empty patch commits")
        };
        assert_eq!(projection, "projected");
        assert_eq!(receipt.revision, revision_before + 1);
        assert!(receipt.impact.contains(ChangeImpact::TERRAIN_GEOMETRY));
        assert_eq!(receipt.touched_regions, vec![RegionCoord::new(51, 50)]);
        assert_eq!(runtime.document().height_at(edited), Some(64));
        assert!(runtime.can_undo());
        assert!(!runtime.can_redo());
    }

    #[test]
    fn no_op_execution_preserves_existing_redo_and_skips_projection() {
        let (mut runtime, _) = runtime_with_redo();
        let before = runtime.clone();
        let mut projected = false;
        let result = runtime
            .execute(MapPatch::default(), |_| {
                projected = true;
                Ok::<_, &'static str>(())
            })
            .unwrap_or_else(|error| unreachable!("empty patch is valid: {error:?}"));

        assert_eq!(result, EditExecution::NoChange);
        assert!(!projected);
        assert_eq!(runtime, before);
    }

    #[test]
    fn semantic_rejection_skips_projection_and_preserves_session() {
        let coord = vertex(3200, 3200);
        let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
            [],
            [(coord, 0)],
            [],
        )));
        let before = runtime.clone();
        let mut projected = false;
        let result = runtime.execute(height_patch(coord, 99, 64), |_| {
            projected = true;
            Ok::<_, &'static str>(())
        });

        assert!(matches!(result, Err(EditExecutionError::Semantic(_))));
        assert!(!projected);
        assert_eq!(runtime, before);
    }

    #[test]
    fn projection_failure_restores_exact_pre_transaction_state() {
        let (mut runtime, edited) = runtime_with_redo();
        let before = runtime.clone();
        let result = runtime.execute(height_patch(edited, 0, 64), |document| {
            assert_eq!(document.height_at(edited), Some(64));
            Err::<(), _>("injected projection failure")
        });

        assert_eq!(
            result,
            Err(EditExecutionError::Projection(
                "injected projection failure"
            ))
        );
        assert_eq!(runtime, before);
    }

    #[test]
    fn successful_undo_replay_finalizes_once_and_returns_receipt() {
        let coord = vertex(3200, 3200);
        let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
            [],
            [(coord, 0)],
            [],
        )));
        runtime
            .commit(height_patch(coord, 0, 64))
            .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
        let revision_before = runtime.revision();

        let result = runtime
            .replay(
                EditReplayDirection::Undo,
                vec![RegionCoord::new(50, 50), RegionCoord::new(50, 50)],
                |document| {
                    assert_eq!(document.height_at(coord), Some(0));
                    Ok::<_, &'static str>("undo projected")
                },
            )
            .unwrap_or_else(|error| unreachable!("gateway Undo succeeds: {error:?}"));

        let EditExecution::Committed {
            receipt,
            projection,
        } = result
        else {
            unreachable!("Undo history exists")
        };
        assert_eq!(projection, "undo projected");
        assert_eq!(receipt.revision, revision_before + 1);
        assert_eq!(receipt.touched_regions, vec![RegionCoord::new(50, 50)]);
        assert_eq!(runtime.document().height_at(coord), Some(0));
        assert!(runtime.can_redo());
    }

    #[test]
    fn failed_redo_replay_projection_restores_exact_pre_transaction_state() {
        let coord = vertex(3200, 3200);
        let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
            [],
            [(coord, 0)],
            [],
        )));
        runtime
            .commit(height_patch(coord, 0, 64))
            .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
        runtime
            .undo()
            .unwrap_or_else(|error| unreachable!("valid setup Undo: {error:?}"));
        let before = runtime.clone();

        let result = runtime.replay(
            EditReplayDirection::Redo,
            vec![RegionCoord::new(50, 50)],
            |document| {
                assert_eq!(document.height_at(coord), Some(64));
                Err::<(), _>("injected Redo projection failure")
            },
        );

        assert_eq!(
            result,
            Err(EditExecutionError::Projection(
                "injected Redo projection failure"
            ))
        );
        assert_eq!(runtime, before);
    }

    #[test]
    fn rollback_invariant_failure_is_explicit() {
        let first = vertex(3200, 3200);
        let second = vertex(3264, 3200);
        let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
            [],
            [(first, 0), (second, 0)],
            [],
        )));
        let (commit, checkpoint) = runtime
            .session
            .commit_checkpointed(height_patch(first, 0, 32))
            .unwrap_or_else(|error| unreachable!("valid candidate commit: {error:?}"))
            .unwrap_or_else(|| unreachable!("candidate changes state"));
        runtime
            .session
            .commit(height_patch(second, 0, 64))
            .unwrap_or_else(|error| unreachable!("valid intervening commit: {error:?}"));

        let result = runtime.finish_execution(
            commit.revision,
            commit.impact,
            vec![RegionCoord::new(50, 50)],
            checkpoint,
            Err::<(), _>("projection failed after intervening mutation"),
        );

        assert_eq!(
            result,
            Err(EditExecutionError::Rollback {
                projection: "projection failed after intervening mutation",
                rollback: EditCheckpointError::StaleSession,
            })
        );
    }

    #[test]
    fn finalize_invariant_failure_is_explicit() {
        let first = vertex(3200, 3200);
        let second = vertex(3264, 3200);
        let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
            [],
            [(first, 0), (second, 0)],
            [],
        )));
        let (commit, checkpoint) = runtime
            .session
            .commit_checkpointed(height_patch(first, 0, 32))
            .unwrap_or_else(|error| unreachable!("valid candidate commit: {error:?}"))
            .unwrap_or_else(|| unreachable!("candidate changes state"));
        runtime
            .session
            .commit(height_patch(second, 0, 64))
            .unwrap_or_else(|error| unreachable!("valid intervening commit: {error:?}"));

        let result = runtime.finish_execution(
            commit.revision,
            commit.impact,
            vec![RegionCoord::new(50, 50)],
            checkpoint,
            Ok::<_, &'static str>(()),
        );

        assert_eq!(
            result,
            Err(EditExecutionError::Finalize(
                EditCheckpointError::StaleSession
            ))
        );
    }
}
