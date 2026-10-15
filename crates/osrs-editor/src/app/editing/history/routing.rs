//! Chronological undo and redo routing across semantic and legacy history entries.
//!
//! Semantic replay is checkpointed by `EditRuntime`. This router snapshots only the transitional
//! worker values needed for projection, prepares the projection from the authoritative post-replay
//! `MapDocument`, and mutates worker/UI mirrors only after gateway finalization succeeds.

use super::super::path::project_semantic_path_values;
use super::super::plan::{project_height_values, project_paint_values};
use super::{
    EditStep, EditorApp, SemanticHeightHistory, SemanticPaintHistory, SemanticPathHistory, TileKey,
};
use crate::app::edit_runtime::{
    EditExecution, EditExecutionError, EditFootprint, EditReplayDirection,
};
use crate::app::object_tool::{ObjectHistoryRoute, build_object_history_projection};
use crate::paint::TileValues;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{HeightVertexCoord, MapQuery};
use osrs_world::{LocEdit, LocSpec};
use web_time::Instant;

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

/// The semantic transaction kind a history step routes to.
enum Route {
    Paint(SemanticPaintHistory),
    Height(SemanticHeightHistory),
    Path(SemanticPathHistory),
    Object(ObjectHistoryRoute),
}

/// Transitional worker values captured before semantic replay. Missing tiles are retained as
/// `None` so the failure occurs inside the gateway projection boundary and triggers exact rollback.
enum ProjectionInput {
    Tiles(Vec<(TileKey, Option<TileValues>)>),
    Object,
}

/// A fully validated replay projection. Applying this after gateway finalization is infallible.
enum PreparedProjection {
    Tiles {
        projected: Vec<(TileKey, TileValues, TileValues)>,
        mark_height: bool,
    },
    Object {
        edits: Vec<LocEdit>,
        reselect: Vec<LocSpec>,
        count: usize,
    },
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

    fn capture_projection_input(&self, app: &EditorApp) -> ProjectionInput {
        let keys = match self {
            Self::Paint(paint) => Some(&paint.keys),
            Self::Height(height) => Some(&height.keys),
            Self::Path(path) => Some(&path.keys),
            Self::Object(_) => None,
        };
        match keys {
            Some(keys) => ProjectionInput::Tiles(
                keys.iter()
                    .map(|key| (*key, app.current_tile(*key)))
                    .collect(),
            ),
            None => ProjectionInput::Object,
        }
    }

    /// Reconstruct the semantic coordinate classes of the opaque session-history patch.
    fn footprint(&self) -> EditFootprint {
        let map_tiles = |keys: &[TileKey]| {
            keys.iter()
                .map(|key| MapTile::new(key.1, key.2))
                .collect::<Vec<_>>()
        };
        let height_vertices = |keys: &[TileKey]| {
            keys.iter()
                .map(|key| {
                    let plane = SourcePlane::new(key.0)
                        .unwrap_or_else(|| unreachable!("history TileKey plane is valid"));
                    HeightVertexCoord::new(MapTile::new(key.1, key.2), plane)
                })
                .collect::<Vec<_>>()
        };
        match self {
            Self::Paint(paint) => EditFootprint::new(map_tiles(&paint.keys), [], []),
            Self::Height(height) => EditFootprint::new([], height_vertices(&height.keys), []),
            Self::Path(path) if path.include_height => EditFootprint::new(
                map_tiles(&path.keys),
                height_vertices(&path.keys),
                [],
            ),
            Self::Path(path) => EditFootprint::new(map_tiles(&path.keys), [], []),
            Self::Object(object) => EditFootprint::new(
                [],
                [],
                object
                    .removed()
                    .iter()
                    .chain(object.added())
                    .map(|spec| MapTile::new(spec.tile.0, spec.tile.1)),
            ),
        }
    }

    fn prepare_projection(
        &self,
        query: &impl MapQuery,
        direction: Direction,
        input: ProjectionInput,
    ) -> Result<PreparedProjection, String> {
        match (self, input) {
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
            _ => unreachable!("projection input is captured from the same history route"),
        }
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

    /// Replay one semantic step through `EditRuntime`, then apply its already-validated projection.
    fn route_semantic(&mut self, direction: Direction, step: EditStep, route: &Route) {
        let (label, word) = (route.label(), direction.word());
        let input = route.capture_projection_input(self);
        let footprint = route.footprint();
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
                self.world
                    .invalidate_semantic_edit(receipt.impact, &receipt.touched_regions);
                direction.settle(&mut self.history, &mut self.redo_stack, step);
                format!("{} {}", direction.past(), route.counted(count))
            }
            Ok(EditExecution::NoChange) => {
                direction.restore(&mut self.history, &mut self.redo_stack, step);
                format!("{label} {word} history is out of sync with the semantic session")
            }
            Err(EditExecutionError::Semantic(error)) => {
                direction.restore(&mut self.history, &mut self.redo_stack, step);
                format!("{label} {word} was rejected by the semantic session: {error:?}")
            }
            Err(EditExecutionError::Projection(message)) => {
                direction.restore(&mut self.history, &mut self.redo_stack, step);
                format!("{label} {word} was rolled back: {message}")
            }
            Err(EditExecutionError::Rollback {
                projection,
                rollback,
            }) => {
                direction.restore(&mut self.history, &mut self.redo_stack, step);
                format!(
                    "{label} {word} projection failed and semantic rollback failed: \
                     {projection} ({rollback:?})"
                )
            }
            Err(EditExecutionError::Finalize(error)) => {
                direction.restore(&mut self.history, &mut self.redo_stack, step);
                format!(
                    "{label} {word} projection succeeded but semantic finalize failed: {error:?}"
                )
            }
        };
        self.notice = Some((notice, Instant::now()));
    }

    /// Apply an already-validated semantic replay projection mechanically. Projection invalidation
    /// is dispatched separately from the authoritative replay receipt after this returns.
    fn apply_replay_projection(&mut self, projection: PreparedProjection) -> usize {
        match projection {
            PreparedProjection::Tiles {
                projected,
                mark_height,
            } => {
                let count = projected.len();
                let mut edits = Vec::new();
                for (key, current, values) in projected {
                    if mark_height {
                        self.height_edited.insert(key);
                    }
                    if values != current {
                        self.edited.insert(key, values);
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
                self.interaction.pending_reselect = reselect;
                count
            }
        }
    }
}

#[cfg(test)]
mod tests;
