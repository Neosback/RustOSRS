//! Chronological undo and redo routing across semantic history entries.
//!
//! Semantic replay is checkpointed by `EditRuntime`. This router snapshots only the transitional
//! worker values needed for projection, prepares the projection from the authoritative post-replay
//! `MapDocument`, and mutates worker/UI mirrors only after gateway finalization succeeds.

use super::super::path::project_semantic_path_values;
use super::super::plan::{project_height_values, project_paint_values};
use super::{
    EditStep, EditorApp, SemanticHeightHistory, SemanticPaintHistory, SemanticPathHistory, TileKey,
};
use crate::app::edit_runtime::{EditExecution, EditExecutionError, EditReplayDirection};
use crate::app::object_tool::{ObjectHistoryRoute, build_object_history_projection};
use crate::paint::TileValues;
use osrs_edit::MapQuery;
use osrs_world::{LocEdit, LocSpec};

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

    fn gateway(self) -> EditReplayDirection {
        match self {
            Self::Undo => EditReplayDirection::Undo,
            Self::Redo => EditReplayDirection::Redo,
        }
    }

    /// The semantic replay succeeded: move its history marker to the opposite stack.
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

/// The semantic transaction kind a history step routes to.
enum Route {
    Appearance,
    Scenario,
    Paint(SemanticPaintHistory),
    Height(SemanticHeightHistory),
    Path(SemanticPathHistory),
    Object(ObjectHistoryRoute),
    /// Terrain and objects changed by one transaction (prefabs, generated buildings).
    Composite(SemanticPathHistory, ObjectHistoryRoute),
}

/// Transitional worker values captured before semantic replay. Missing tiles are retained as
/// `None` so the failure occurs inside the gateway projection boundary and triggers exact rollback.
enum ProjectionInput {
    Tiles(Vec<(TileKey, Option<TileValues>)>),
    Object,
    TilesAndObject(Vec<(TileKey, Option<TileValues>)>),
}

/// A fully validated replay projection. Applying this after gateway finalization is infallible.
enum PreparedProjection {
    Appearance,
    Scenario,
    Tiles {
        projected: Vec<(TileKey, TileValues, TileValues)>,
        mark_height: bool,
    },
    Object {
        edits: Vec<LocEdit>,
        reselect: Vec<LocSpec>,
        count: usize,
    },
    Composite {
        projected: Vec<(TileKey, TileValues, TileValues)>,
        mark_height: bool,
        edits: Vec<LocEdit>,
        reselect: Vec<LocSpec>,
        count: usize,
    },
}

impl Route {
    fn of(step: &EditStep) -> Option<Self> {
        if step.semantic_appearance {
            Some(Self::Appearance)
        } else if step.semantic_scenario {
            Some(Self::Scenario)
        } else if let Some(paint) = &step.semantic_paint {
            Some(Self::Paint(paint.clone()))
        } else if let Some(height) = &step.semantic_height {
            Some(Self::Height(height.clone()))
        } else if let (Some(path), Some(object)) = (&step.semantic_path, &step.semantic_object) {
            Some(Self::Composite(path.clone(), object.clone()))
        } else if let Some(path) = &step.semantic_path {
            Some(Self::Path(path.clone()))
        } else {
            step.semantic_object.clone().map(Self::Object)
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Scenario => "NPC scenario",
            Self::Paint(_) => "Paint",
            Self::Height(_) => "Height",
            Self::Path(_) => "Path",
            Self::Object(_) => "Object",
            Self::Composite(..) => "Prefab",
        }
    }

    /// "3 painted tiles", "1 object": what a success notice counts.
    fn counted(&self, count: usize) -> String {
        let (noun, plural) = match self {
            Self::Appearance => ("appearance change", "s"),
            Self::Scenario => ("NPC scenario change", "s"),
            Self::Paint(_) => ("painted tile", "s"),
            Self::Height(_) => ("Height tile", "s"),
            Self::Path(_) => ("path tile", "s"),
            Self::Object(_) => ("object", "s"),
            Self::Composite(..) => ("map change", "s"),
        };
        format!("{count} {noun}{}", if count == 1 { "" } else { plural })
    }

    fn capture_projection_input(&self, app: &EditorApp) -> ProjectionInput {
        let keys = match self {
            Self::Paint(paint) => Some(&paint.keys),
            Self::Height(height) => Some(&height.keys),
            Self::Path(path) => Some(&path.keys),
            Self::Composite(path, _) => Some(&path.keys),
            Self::Object(_) | Self::Scenario | Self::Appearance => None,
        };
        let tiles = |keys: &Vec<TileKey>| -> Vec<(TileKey, Option<TileValues>)> {
            keys.iter()
                .map(|key| (*key, app.current_tile(*key)))
                .collect()
        };
        match (keys, self) {
            (Some(keys), Self::Composite(..)) => ProjectionInput::TilesAndObject(tiles(keys)),
            (Some(keys), _) => ProjectionInput::Tiles(tiles(keys)),
            (None, _) => ProjectionInput::Object,
        }
    }

    fn prepare_projection(
        &self,
        query: &impl MapQuery,
        direction: Direction,
        input: ProjectionInput,
    ) -> Result<PreparedProjection, String> {
        match (self, input) {
            (Self::Appearance, _) => Ok(PreparedProjection::Appearance),
            (Self::Scenario, _) => Ok(PreparedProjection::Scenario),
            (Self::Paint(paint), ProjectionInput::Tiles(current)) => {
                let mut projected = Vec::with_capacity(current.len());
                for (key, current) in current {
                    let current = current.ok_or_else(|| {
                        "a painted tile is no longer available to the projection adapter".to_owned()
                    })?;
                    let values = project_paint_values(query, &paint.brush, key, current)
                        .ok_or_else(|| {
                            "semantic Paint state cannot be represented by the legacy projection"
                                .to_owned()
                        })?;
                    projected.push((key, current, values));
                }
                Ok(PreparedProjection::Tiles {
                    projected,
                    mark_height: false,
                })
            }
            (Self::Height(_), ProjectionInput::Tiles(current)) => {
                let mut projected = Vec::with_capacity(current.len());
                for (key, current) in current {
                    let current = current.ok_or_else(|| {
                        "a Height tile is no longer available to the projection adapter".to_owned()
                    })?;
                    let values = project_height_values(query, key, current).ok_or_else(|| {
                        "semantic Height state cannot be represented by the legacy projection"
                            .to_owned()
                    })?;
                    projected.push((key, current, values));
                }
                Ok(PreparedProjection::Tiles {
                    projected,
                    mark_height: true,
                })
            }
            (Self::Path(path), ProjectionInput::Tiles(current)) => {
                let mut projected = Vec::with_capacity(current.len());
                for (key, current) in current {
                    let current = current.ok_or_else(|| {
                        "a Path tile is no longer available to the projection adapter".to_owned()
                    })?;
                    let values =
                        project_semantic_path_values(query, key, current, path.include_height)
                            .ok_or_else(|| {
                                "semantic Path state cannot be represented by the legacy projection"
                                    .to_owned()
                            })?;
                    projected.push((key, current, values));
                }
                Ok(PreparedProjection::Tiles {
                    projected,
                    mark_height: path.include_height,
                })
            }
            (Self::Object(object), ProjectionInput::Object) => {
                let projection =
                    build_object_history_projection(query, object, direction == Direction::Redo)
                        .map_err(|error| format!("{error:?}"))?;
                Ok(PreparedProjection::Object {
                    edits: projection.edits,
                    reselect: projection.reselect,
                    count: object.object_count(),
                })
            }
            (Self::Composite(path, object), ProjectionInput::TilesAndObject(current)) => {
                let mut projected = Vec::with_capacity(current.len());
                for (key, current) in current {
                    let current = current.ok_or_else(|| {
                        "a tile of the area is no longer available to the projection adapter"
                            .to_owned()
                    })?;
                    let values =
                        project_semantic_path_values(query, key, current, path.include_height)
                            .ok_or_else(|| {
                                "the semantic result cannot be represented by the legacy projection"
                                    .to_owned()
                            })?;
                    projected.push((key, current, values));
                }
                let projection =
                    build_object_history_projection(query, object, direction == Direction::Redo)
                        .map_err(|error| format!("{error:?}"))?;
                Ok(PreparedProjection::Composite {
                    projected,
                    mark_height: path.include_height,
                    edits: projection.edits,
                    reselect: projection.reselect,
                    count: object.object_count() + path.keys.len(),
                })
            }
            _ => unreachable!("projection input is captured from the same history route"),
        }
    }
}

impl EditorApp {
    pub(in crate::app) fn undo(&mut self) {
        let Some(step) = self.terrain.history.pop() else {
            return;
        };
        let Some(route) = Route::of(&step) else {
            self.terrain.history.push(step);
            self.ui.notify(
                "Undo history marker has no semantic route; no state was changed".to_owned(),
            );
            return;
        };
        self.route_semantic(Direction::Undo, step, &route);
    }

    pub(in crate::app) fn redo(&mut self) {
        let Some(step) = self.terrain.redo_stack.pop() else {
            return;
        };
        let Some(route) = Route::of(&step) else {
            self.terrain.redo_stack.push(step);
            self.ui.notify(
                "Redo history marker has no semantic route; no state was changed".to_owned(),
            );
            return;
        };
        self.route_semantic(Direction::Redo, step, &route);
    }

    /// Replay one semantic step through `EditRuntime`, then apply its already-validated projection.
    fn route_semantic(&mut self, direction: Direction, step: EditStep, route: &Route) {
        let (label, word) = (route.label(), direction.word());
        let input = route.capture_projection_input(self);
        let footprint = route.footprint(direction);
        let execution = self
            .session
            .replay(direction.gateway(), footprint, |document| {
                route.prepare_projection(document, direction, input)
            });

        let notice = match execution {
            Ok(EditExecution::Committed {
                receipt,
                projection,
            }) => {
                let count = self.apply_replay_projection(projection);
                self.world.invalidate_semantic_edit(&receipt);
                direction.settle(
                    &mut self.terrain.history,
                    &mut self.terrain.redo_stack,
                    step,
                );
                format!("{} {}", direction.past(), route.counted(count))
            }
            Ok(EditExecution::NoChange) => {
                direction.restore(
                    &mut self.terrain.history,
                    &mut self.terrain.redo_stack,
                    step,
                );
                format!("{label} {word} history is out of sync with the semantic session")
            }
            Err(EditExecutionError::Semantic(error)) => {
                direction.restore(
                    &mut self.terrain.history,
                    &mut self.terrain.redo_stack,
                    step,
                );
                format!("{label} {word} was rejected by the semantic session: {error:?}")
            }
            Err(EditExecutionError::Projection(message)) => {
                direction.restore(
                    &mut self.terrain.history,
                    &mut self.terrain.redo_stack,
                    step,
                );
                format!("{label} {word} was rolled back: {message}")
            }
            Err(EditExecutionError::Rollback {
                projection,
                rollback,
            }) => {
                direction.restore(
                    &mut self.terrain.history,
                    &mut self.terrain.redo_stack,
                    step,
                );
                format!(
                    "{label} {word} projection failed and semantic rollback failed: \
                     {projection} ({rollback:?})"
                )
            }
            Err(EditExecutionError::Finalize(error)) => {
                direction.restore(
                    &mut self.terrain.history,
                    &mut self.terrain.redo_stack,
                    step,
                );
                format!(
                    "{label} {word} projection succeeded but semantic finalize failed: {error:?}"
                )
            }
        };
        self.ui.notify(notice);
    }

    /// Apply an already-validated semantic replay projection mechanically. Projection invalidation
    /// is dispatched separately from the authoritative replay receipt after this returns.
    fn apply_replay_projection(&mut self, projection: PreparedProjection) -> usize {
        match projection {
            PreparedProjection::Scenario | PreparedProjection::Appearance => 1,
            PreparedProjection::Tiles {
                projected,
                mark_height,
            } => {
                let count = projected.len();
                let mut edits = Vec::new();
                for (key, current, values) in projected {
                    if mark_height {
                        self.terrain.height_edited.insert(key);
                    }
                    if values != current {
                        self.terrain.edited.insert(key, values);
                        edits.push(self.edit_for(key, values));
                    }
                }
                self.world.project_tile_edits(edits);
                count
            }
            PreparedProjection::Object {
                edits,
                reselect,
                count,
            } => {
                self.world.project_loc_edits(edits);
                self.clear_selection();
                self.interaction.set_pending_reselect(reselect);
                count
            }
            PreparedProjection::Composite {
                projected,
                mark_height,
                edits,
                reselect,
                count,
            } => {
                let mut tile_edits = Vec::new();
                for (key, current, values) in projected {
                    if mark_height {
                        self.terrain.height_edited.insert(key);
                    }
                    if values != current {
                        self.terrain.edited.insert(key, values);
                        tile_edits.push(self.edit_for(key, values));
                    }
                }
                self.world.project_tile_edits(tile_edits);
                self.world.project_loc_edits(edits);
                self.clear_selection();
                self.interaction.set_pending_reselect(reselect);
                count
            }
        }
    }
}

mod footprint;
#[cfg(test)]
mod tests;
