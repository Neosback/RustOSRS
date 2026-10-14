//! Chronological undo and redo routing across semantic and legacy history entries.
//!
//! Undo and redo are the same transaction run in opposite directions: take the newest step from
//! one stack, replay it through an exact `EditorSession` checkpoint, project the authoritative
//! result into worker mirrors, and move the step only after the semantic checkpoint is finalized.
//! Projection failure restores both semantic state and the mixed-history marker exactly.

use super::*;
use crate::session::{EditCheckpoint, EditCheckpointError, EditorSession};
use osrs_edit::PatchApplyError;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Undo,
    Redo,
}

impl Direction {
    fn word(self) -> &'static str {
        match self {
            Self::Undo => "undo",
            Self::Redo => "redo",
        }
    }

    fn past(self) -> &'static str {
        match self {
            Self::Undo => "Undid",
            Self::Redo => "Redid",
        }
    }

    /// Begin one semantic replay without finalizing its history transition.
    fn replay(self, session: &mut EditorSession) -> Result<Option<EditCheckpoint>, PatchApplyError> {
        let replay = match self {
            Self::Undo => session.undo_checkpointed(),
            Self::Redo => session.redo_checkpointed(),
        }?;
        Ok(replay.map(|(_, checkpoint)| checkpoint))
    }

    /// The semantic replay succeeded: move its mixed-history marker to the opposite stack.
    fn settle(self, history: &mut Vec<EditStep>, redo_stack: &mut Vec<EditStep>, step: EditStep) {
        match self {
            Self::Undo => redo_stack.push(step),
            Self::Redo => history.push(step),
        }
    }

    /// The semantic replay did not complete: return its marker to the stack it came from.
    fn restore(self, history: &mut Vec<EditStep>, redo_stack: &mut Vec<EditStep>, step: EditStep) {
        match self {
            Self::Undo => history.push(step),
            Self::Redo => redo_stack.push(step),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ReplayProjectionOutcome {
    Success(usize),
    ProjectionRolledBack(String),
    RollbackFailed {
        projection: String,
        rollback: EditCheckpointError,
    },
    FinalizeFailed(EditCheckpointError),
}

/// Resolve a checkpointed semantic Undo/Redo after projection has finished.
///
/// The app-level mixed-history marker moves only after successful projection and checkpoint
/// finalization. A projection failure restores the semantic checkpoint before returning the marker
/// to its original stack, making the failed replay invisible to both history systems.
fn finish_semantic_replay(
    direction: Direction,
    session: &mut EditorSession,
    checkpoint: EditCheckpoint,
    projection: Result<usize, String>,
    step: EditStep,
    history: &mut Vec<EditStep>,
    redo_stack: &mut Vec<EditStep>,
) -> ReplayProjectionOutcome {
    match projection {
        Ok(count) => match session.finalize_checkpoint(checkpoint) {
            Ok(()) => {
                direction.settle(history, redo_stack, step);
                ReplayProjectionOutcome::Success(count)
            }
            Err(error) => {
                direction.restore(history, redo_stack, step);
                ReplayProjectionOutcome::FinalizeFailed(error)
            }
        },
        Err(projection) => {
            let rollback = session.restore_checkpoint(checkpoint);
            direction.restore(history, redo_stack, step);
            match rollback {
                Ok(()) => ReplayProjectionOutcome::ProjectionRolledBack(projection),
                Err(rollback) => ReplayProjectionOutcome::RollbackFailed {
                    projection,
                    rollback,
                },
            }
        }
    }
}

/// The semantic transaction kind a history step routes to.
enum Route {
    Paint(SemanticPaintHistory),
    Height(SemanticHeightHistory),
    Path(SemanticPathHistory),
    Object(ObjectHistoryRoute),
}

impl Route {
    fn of(step: &EditStep) -> Option<Self> {
        if let Some(paint) = &step.semantic_paint {
            Some(Self::Paint(paint.clone()))
        } else if let Some(height) = &step.semantic_height {
            Some(Self::Height(height.clone()))
        } else if let Some(path) = &step.semantic_path {
            Some(Self::Path(path.clone()))
        } else {
            step.semantic_object.clone().map(Self::Object)
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Paint(_) => "Paint",
            Self::Height(_) => "Height",
            Self::Path(_) => "Path",
            Self::Object(_) => "Object",
        }
    }

    /// "3 painted tiles", "1 object": what a success notice counts.
    fn counted(&self, count: usize) -> String {
        let (noun, plural) = match self {
            Self::Paint(_) => ("painted tile", "s"),
            Self::Height(_) => ("Height tile", "s"),
            Self::Path(_) => ("path tile", "s"),
            Self::Object(_) => ("object", "s"),
        };
        format!("{count} {noun}{}", if count == 1 { "" } else { plural })
    }
}

impl EditorApp {
    pub(in crate::app) fn undo(&mut self) {
        let Some(step) = self.history.pop() else {
            return;
        };
        match Route::of(&step) {
            Some(route) => self.route_semantic(Direction::Undo, step, &route),
            None => {
                self.revert(&step, false);
                self.notice = Some((
                    format!("Undid {} tile change(s)", step.changes.len()),
                    Instant::now(),
                ));
                self.redo_stack.push(step);
            }
        }
    }

    pub(in crate::app) fn redo(&mut self) {
        let Some(step) = self.redo_stack.pop() else {
            return;
        };
        match Route::of(&step) {
            Some(route) => self.route_semantic(Direction::Redo, step, &route),
            None => {
                self.revert(&step, true);
                self.history.push(step);
            }
        }
    }

    /// Replay one semantic step in `direction`, project it, then finalize or restore atomically.
    fn route_semantic(&mut self, direction: Direction, step: EditStep, route: &Route) {
        let (label, word) = (route.label(), direction.word());
        let notice = match direction.replay(&mut self.session) {
            Ok(Some(checkpoint)) => {
                let projection = self.project_route(route, direction);
                match finish_semantic_replay(
                    direction,
                    &mut self.session,
                    checkpoint,
                    projection,
                    step,
                    &mut self.history,
                    &mut self.redo_stack,
                ) {
                    ReplayProjectionOutcome::Success(count) => {
                        format!("{} {}", direction.past(), route.counted(count))
                    }
                    ReplayProjectionOutcome::ProjectionRolledBack(message) => {
                        format!("{label} {word} was rolled back: {message}")
                    }
                    ReplayProjectionOutcome::RollbackFailed {
                        projection,
                        rollback,
                    } => format!(
                        "{label} {word} projection failed and semantic rollback failed: \
                         {projection} ({rollback:?})"
                    ),
                    ReplayProjectionOutcome::FinalizeFailed(error) => format!(
                        "{label} {word} projection succeeded but semantic finalize failed: \
                         {error:?}"
                    ),
                }
            }
            Ok(None) => {
                direction.restore(&mut self.history, &mut self.redo_stack, step);
                format!("{label} {word} history is out of sync with the semantic session")
            }
            Err(error) => {
                direction.restore(&mut self.history, &mut self.redo_stack, step);
                format!("{label} {word} was rejected by the semantic session: {error:?}")
            }
        };
        self.notice = Some((notice, Instant::now()));
    }

    fn project_route(&mut self, route: &Route, direction: Direction) -> Result<usize, String> {
        match route {
            Route::Paint(paint) => self
                .project_semantic_paint_history(paint)
                .map_err(str::to_owned),
            Route::Height(height) => self
                .project_semantic_height_history(height)
                .map_err(str::to_owned),
            Route::Path(path) => self
                .project_semantic_path_history(path)
                .map_err(str::to_owned),
            Route::Object(object) => {
                self.project_semantic_object_history(object, direction == Direction::Redo)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_edit::{HeightVertexCoord, MapDocument, MapOp, MapPatch};

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn coord() -> HeightVertexCoord {
        HeightVertexCoord::new(MapTile::new(3200, 3200), plane())
    }

    fn height_patch(coord: HeightVertexCoord, before: i32, after: i32) -> MapPatch {
        MapPatch::new([MapOp::SetHeight {
            coord,
            before,
            after,
        }])
    }

    fn applied_session() -> EditorSession {
        let coord = coord();
        let mut session = EditorSession::new(MapDocument::from_parts([], [(coord, 0)], []));
        session
            .commit(height_patch(coord, 0, 64))
            .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
        session
    }

    fn marker() -> EditStep {
        EditStep::semantic_height(vec![(0, 3200, 3200)])
    }

    #[test]
    fn failed_semantic_undo_projection_restores_session_and_history_stack() {
        let mut session = applied_session();
        let before = session.clone();
        let mut history = vec![marker()];
        let mut redo_stack = Vec::new();
        let step = history
            .pop()
            .unwrap_or_else(|| unreachable!("Undo marker exists"));
        let checkpoint = Direction::Undo
            .replay(&mut session)
            .unwrap_or_else(|error| unreachable!("valid Undo replay: {error:?}"))
            .unwrap_or_else(|| unreachable!("Undo replay exists"));

        let outcome = finish_semantic_replay(
            Direction::Undo,
            &mut session,
            checkpoint,
            Err("injected Undo projection failure".to_owned()),
            step,
            &mut history,
            &mut redo_stack,
        );

        assert_eq!(
            outcome,
            ReplayProjectionOutcome::ProjectionRolledBack(
                "injected Undo projection failure".to_owned()
            )
        );
        assert_eq!(session, before);
        assert_eq!(history.len(), 1);
        assert!(history[0].semantic_height.is_some());
        assert!(redo_stack.is_empty());
    }

    #[test]
    fn failed_semantic_redo_projection_restores_session_and_redo_stack() {
        let mut session = applied_session();
        session
            .undo()
            .unwrap_or_else(|error| unreachable!("valid setup Undo: {error:?}"));
        let before = session.clone();
        let mut history = Vec::new();
        let mut redo_stack = vec![marker()];
        let step = redo_stack
            .pop()
            .unwrap_or_else(|| unreachable!("Redo marker exists"));
        let checkpoint = Direction::Redo
            .replay(&mut session)
            .unwrap_or_else(|error| unreachable!("valid Redo replay: {error:?}"))
            .unwrap_or_else(|| unreachable!("Redo replay exists"));

        let outcome = finish_semantic_replay(
            Direction::Redo,
            &mut session,
            checkpoint,
            Err("injected Redo projection failure".to_owned()),
            step,
            &mut history,
            &mut redo_stack,
        );

        assert_eq!(
            outcome,
            ReplayProjectionOutcome::ProjectionRolledBack(
                "injected Redo projection failure".to_owned()
            )
        );
        assert_eq!(session, before);
        assert!(history.is_empty());
        assert_eq!(redo_stack.len(), 1);
        assert!(redo_stack[0].semantic_height.is_some());
    }

    #[test]
    fn successful_semantic_undo_finalizes_once_and_moves_history_marker() {
        let mut session = applied_session();
        let revision_before = session.revision();
        let mut history = vec![marker()];
        let mut redo_stack = Vec::new();
        let step = history
            .pop()
            .unwrap_or_else(|| unreachable!("Undo marker exists"));
        let checkpoint = Direction::Undo
            .replay(&mut session)
            .unwrap_or_else(|error| unreachable!("valid Undo replay: {error:?}"))
            .unwrap_or_else(|| unreachable!("Undo replay exists"));

        let outcome = finish_semantic_replay(
            Direction::Undo,
            &mut session,
            checkpoint,
            Ok(1),
            step,
            &mut history,
            &mut redo_stack,
        );

        assert_eq!(outcome, ReplayProjectionOutcome::Success(1));
        assert_eq!(session.revision(), revision_before + 1);
        assert_eq!(session.document().height_at(coord()), Some(0));
        assert!(session.can_redo());
        assert!(history.is_empty());
        assert_eq!(redo_stack.len(), 1);
        assert!(redo_stack[0].semantic_height.is_some());
    }

    #[test]
    fn successful_semantic_redo_finalizes_once_and_moves_redo_marker() {
        let mut session = applied_session();
        session
            .undo()
            .unwrap_or_else(|error| unreachable!("valid setup Undo: {error:?}"));
        let revision_before = session.revision();
        let mut history = Vec::new();
        let mut redo_stack = vec![marker()];
        let step = redo_stack
            .pop()
            .unwrap_or_else(|| unreachable!("Redo marker exists"));
        let checkpoint = Direction::Redo
            .replay(&mut session)
            .unwrap_or_else(|error| unreachable!("valid Redo replay: {error:?}"))
            .unwrap_or_else(|| unreachable!("Redo replay exists"));

        let outcome = finish_semantic_replay(
            Direction::Redo,
            &mut session,
            checkpoint,
            Ok(1),
            step,
            &mut history,
            &mut redo_stack,
        );

        assert_eq!(outcome, ReplayProjectionOutcome::Success(1));
        assert_eq!(session.revision(), revision_before + 1);
        assert_eq!(session.document().height_at(coord()), Some(64));
        assert!(session.can_undo());
        assert_eq!(history.len(), 1);
        assert!(history[0].semantic_height.is_some());
        assert!(redo_stack.is_empty());
    }
}
