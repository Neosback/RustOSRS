//! Application-level semantic edit transaction gateway.
//!
//! `EditorSession` remains the semantic authority. This runtime owns that session and centralizes
//! the checkpoint -> projection -> finalize/restore policy for interactive semantic transactions.
//! Read/query and source/project lifecycle operations are exposed explicitly; transaction lifecycle
//! primitives stay private to this runtime so callers cannot bypass gateway policy accidentally.

mod baseline;
mod footprint;
pub(super) use footprint::{EditFootprint, EditTouches};

use super::diagnostics::{DiagnosticSink, TraceEvent};
use crate::session::{
    EditCheckpoint, EditCheckpointError, EditReplayDescriptor, EditorSession, RegionSyncOutcome,
};
use osrs_core::coords::RegionCoord;
use osrs_edit::{
    ChangeImpact, MapDocument, MapPatch, MapRegionSnapshot, PatchApplyError, ScenarioDocument,
    ScenarioPatch,
};
#[cfg(test)]
use std::convert::Infallible;

/// Application owner of the authoritative semantic edit session.
#[derive(Debug, Clone, Default)]
pub(super) struct EditRuntime {
    session: EditorSession,
    diagnostics: DiagnosticSink,
}

impl PartialEq for EditRuntime {
    fn eq(&self, other: &Self) -> bool {
        self.session == other.session
    }
}
impl Eq for EditRuntime {}

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
    pub(super) fn set_diagnostics(&mut self, sink: DiagnosticSink) {
        self.diagnostics = sink;
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn new(session: EditorSession) -> Self {
        Self {
            session,
            diagnostics: DiagnosticSink::default(),
        }
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

    pub(super) fn available_regions(&self) -> impl Iterator<Item = RegionCoord> + '_ {
        self.session.available_regions()
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
        let _ = self.replay(EditReplayDirection::Undo, |_, _| Ok::<_, Infallible>(()))?;
        Ok(())
    }

    pub(super) fn map_revision(&self) -> u64 {
        self.session.map_revision()
    }
    pub(super) fn scenario(&self) -> &ScenarioDocument {
        self.session.scenario()
    }
    pub(super) fn execute_scenario<T, E>(
        &mut self,
        patch: ScenarioPatch,
        project: impl FnOnce(&ScenarioDocument) -> Result<T, E>,
    ) -> Result<EditExecution<T>, EditExecutionError<E>> {
        let Some((commit, checkpoint)) = self
            .session
            .commit_scenario_checkpointed(patch)
            .map_err(EditExecutionError::Semantic)?
        else {
            return Ok(EditExecution::NoChange);
        };
        let projection = project(self.session.scenario());
        self.finish_execution(
            commit.revision,
            commit.impact,
            EditTouches::default(),
            checkpoint,
            projection,
        )
    }

    pub(super) fn appearance(&self) -> &osrs_hd::theme::ThemePack {
        self.session.appearance()
    }
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(super) fn load_appearance(
        &mut self,
        appearance: osrs_hd::theme::ThemePack,
    ) -> Result<(), String> {
        self.session.load_appearance(appearance)
    }
    pub(super) fn execute_appearance<T, E>(
        &mut self,
        patch: osrs_hd::theme::ThemePatch,
        project: impl FnOnce(&osrs_hd::theme::ThemePack) -> Result<T, E>,
    ) -> Result<EditExecution<T>, EditExecutionError<E>> {
        let Some((commit, checkpoint)) = self
            .session
            .commit_appearance_checkpointed(patch)
            .map_err(EditExecutionError::Semantic)?
        else {
            return Ok(EditExecution::NoChange);
        };
        let projection = project(self.session.appearance());
        self.finish_execution(
            commit.revision,
            commit.impact,
            EditTouches::default(),
            checkpoint,
            projection,
        )
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
        self.execute_named("edit", patch, project)
    }

    pub(super) fn execute_named<T, E>(
        &mut self,
        operation: &'static str,
        patch: MapPatch,
        project: impl FnOnce(&MapDocument) -> Result<T, E>,
    ) -> Result<EditExecution<T>, EditExecutionError<E>> {
        let before = self.revision();
        let touches = EditFootprint::from_patch(&patch).into_touches();
        let footprint = if self.diagnostics.is_enabled() {
            touches.footprint.clone()
        } else {
            EditFootprint::default()
        };
        let result = (|| {
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
        })();
        self.observe_execution(operation, before, footprint, &result);
        result
    }

    /// Replay one semantic Undo/Redo as an application transaction.
    ///
    /// The exact replay descriptor comes from `EditorSession`'s authoritative history before the
    /// history transition begins. The application receives only that immutable descriptor plus the
    /// authoritative post-replay `MapDocument`; raw history patches never escape the session.
    pub(super) fn replay<T, E>(
        &mut self,
        direction: EditReplayDirection,
        project: impl FnOnce(&MapDocument, &EditReplayDescriptor) -> Result<T, E>,
    ) -> Result<EditExecution<T>, EditExecutionError<E>> {
        let before = self.revision();
        let descriptor = match direction {
            EditReplayDirection::Undo => self.session.next_undo_replay_descriptor(),
            EditReplayDirection::Redo => self.session.next_redo_replay_descriptor(),
        };
        let footprint = match descriptor.as_ref() {
            Some(EditReplayDescriptor::Map(footprint)) => {
                EditFootprint::from_replay_footprint(footprint)
            }
            Some(EditReplayDescriptor::Scenario | EditReplayDescriptor::Appearance) | None => {
                EditFootprint::default()
            }
        };
        let observed = if self.diagnostics.is_enabled() {
            footprint.clone()
        } else {
            EditFootprint::default()
        };
        let result = (|| {
            let Some(descriptor) = descriptor else {
                return Ok(EditExecution::NoChange);
            };
            let replay = match direction {
                EditReplayDirection::Undo => self.session.undo_checkpointed(),
                EditReplayDirection::Redo => self.session.redo_checkpointed(),
            }
            .map_err(EditExecutionError::Semantic)?;
            let Some((commit, checkpoint)) = replay else {
                return Ok(EditExecution::NoChange);
            };

            let projection = project(self.session.document(), &descriptor);
            self.finish_execution(
                commit.revision,
                commit.impact,
                footprint.into_touches(),
                checkpoint,
                projection,
            )
        })();
        let operation = match direction {
            EditReplayDirection::Undo => "undo",
            EditReplayDirection::Redo => "redo",
        };
        self.observe_execution(operation, before, observed, &result);
        result
    }

    fn observe_execution<T, E>(
        &self,
        operation: &'static str,
        before: u64,
        footprint: EditFootprint,
        result: &Result<EditExecution<T>, EditExecutionError<E>>,
    ) {
        self.diagnostics.record(|| {
            let (outcome, impact) = match result {
                Ok(EditExecution::Committed { receipt, .. }) => ("committed", receipt.impact),
                Ok(EditExecution::NoChange) => ("no change", ChangeImpact::NONE),
                Err(EditExecutionError::Semantic(_)) => ("semantic rejection", ChangeImpact::NONE),
                Err(EditExecutionError::Projection(_)) => {
                    ("projection rejected; rolled back", ChangeImpact::NONE)
                }
                Err(EditExecutionError::Rollback { .. }) => ("rollback failed", ChangeImpact::NONE),
                Err(EditExecutionError::Finalize(_)) => ("finalization failed", ChangeImpact::NONE),
            };
            TraceEvent::Edit {
                operation,
                before,
                after: self.revision(),
                outcome,
                impact,
                footprint,
            }
        });
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

#[cfg(test)]
mod tests;
