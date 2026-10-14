//! Chronological undo and redo routing across semantic and legacy history entries.
//!
//! Undo and redo are the same transaction run in opposite directions: take the newest step from
//! one stack, replay it through `EditorSession`, project the authoritative result into the worker
//! mirrors, and move the step to the other stack (or back, with the session rolled back, when
//! projection fails). `Direction` carries everything that differs between the two.

use super::*;
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

    /// Replay through the session: `Ok(true)` when an entry was applied.
    fn replay(self, session: &mut crate::session::EditorSession) -> Result<bool, PatchApplyError> {
        match self {
            Self::Undo => session.undo(),
            Self::Redo => session.redo(),
        }
        .map(|commit| commit.is_some())
    }

    /// Undo the session's replay (a failed projection must not leave history and view diverged).
    fn rollback(self, session: &mut crate::session::EditorSession) -> bool {
        matches!(
            match self {
                Self::Undo => session.redo(),
                Self::Redo => session.undo(),
            },
            Ok(Some(_))
        )
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

    /// Replay one semantic step in `direction`, then project it.
    fn route_semantic(&mut self, direction: Direction, step: EditStep, route: &Route) {
        let (label, word) = (route.label(), direction.word());
        let notice = match direction.replay(&mut self.session) {
            Ok(true) => match self.project_route(route, direction) {
                Ok(count) => {
                    self.settle(direction, step);
                    format!("{} {}", direction.past(), route.counted(count))
                }
                Err(message) => {
                    let rolled_back = direction.rollback(&mut self.session);
                    self.restore(direction, step);
                    if rolled_back {
                        format!("{label} {word} was rolled back: {message}")
                    } else {
                        format!("{label} {word} projection failed and rollback failed: {message}")
                    }
                }
            },
            Ok(false) => {
                self.restore(direction, step);
                format!("{label} {word} history is out of sync with the semantic session")
            }
            Err(error) => {
                self.restore(direction, step);
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

    /// The step succeeded: it now belongs on the opposite stack.
    fn settle(&mut self, direction: Direction, step: EditStep) {
        match direction {
            Direction::Undo => self.redo_stack.push(step),
            Direction::Redo => self.history.push(step),
        }
    }

    /// The step did not apply: put it back where it came from.
    fn restore(&mut self, direction: Direction, step: EditStep) {
        match direction {
            Direction::Undo => self.history.push(step),
            Direction::Redo => self.redo_stack.push(step),
        }
    }
}
