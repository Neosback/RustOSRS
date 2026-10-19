//! Chronological undo and redo routing across semantic history entries.
//!
//! Semantic replay is checkpointed by `EditRuntime`. Active worker projection is derived from the
//! exact authoritative `EditReplayDescriptor`; the parallel `EditStep` stack remains temporarily
//! only for chronological labels/stack movement until CP2H removes it.

#[cfg(test)]
use super::super::path::project_semantic_path_values;
#[cfg(test)]
use super::super::plan::{project_height_values, project_paint_values};
use super::replay_projection::{
    MapReplayInput, PreparedMapReplay, capture_map_replay_input, prepare_map_replay,
};
use super::{EditStep, EditorApp, SemanticHeightHistory, SemanticPaintHistory, SemanticPathHistory};
#[cfg(test)]
use super::TileKey;
use crate::app::edit_runtime::{EditExecution, EditExecutionError, EditReplayDirection};
#[cfg(test)]
use crate::app::object_tool::build_object_history_projection;
use crate::app::object_tool::ObjectHistoryRoute;
#[cfg(test)]
use crate::paint::TileValues;
use crate::session::EditReplayDescriptor;
use osrs_edit::MapQuery;
#[cfg(test)]
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

    /// The semantic replay succeeded: move its temporary chronology marker to the opposite stack.
    fn settle(self, history: &mut Vec<EditStep>, redo_stack: &mut Vec<EditStep>, step: EditStep) {
        match self {
            Self::Undo => redo_stack.push(step),
            Self::Redo => history.push(step),
        }
    }

    /// The semantic replay did not complete: return its temporary marker to its source stack.
    fn restore(self, history: &mut Vec<EditStep>, redo_stack: &mut Vec<EditStep>, step: EditStep) {
        match self {
            Self::Undo => history.push(step),
            Self::Redo => redo_stack.push(step),
        }
    }
}

/// The semantic transaction label carried by the temporary application chronology marker.
///
/// Payloads remain only for CP2H/CP2I compatibility and focused parity tests. Active projection no
/// longer reads them.
#[allow(dead_code)]
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

/// Disposable pre-replay worker values captured from the authoritative descriptor.
enum DescriptorProjectionInput {
    Map(MapReplayInput),
    Scenario,
    Appearance,
}

/// A fully validated replay projection prepared from the authoritative descriptor.
enum DescriptorProjection {
    Map(PreparedMapReplay),
    Scenario,
    Appearance,
}

/// Transitional route-specific replay input retained only as a parity oracle during CP2G.
#[cfg(test)]
enum ProjectionInput {
    Tiles(Vec<(TileKey, Option<TileValues>)>),
    Object,
    TilesAndObject(Vec<(TileKey, Option<TileValues>)>),
}

/// Transitional route-specific prepared projection retained only for focused migration tests.
#[cfg(test)]
#[allow(dead_code)]
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

    fn matches_descriptor(&self, descriptor: &EditReplayDescriptor) -> bool {
        matches!(
            (self, descriptor),
            (Self::Appearance, EditReplayDescriptor::Appearance)
                | (Self::Scenario, EditReplayDescriptor::Scenario)
                | (
                    Self::Paint(_)
                        | Self::Height(_)
                        | Self::Path(_)
                        | Self::Object(_)
                        | Self::Composite(..),
                    EditReplayDescriptor::Map(_)
                )
        )
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

    #[cfg(test)]
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

fn prepare_descriptor_projection(
    query: &impl MapQuery,
    descriptor: &EditReplayDescriptor,
    input: DescriptorProjectionInput,
) -> Result<DescriptorProjection, String> {
    match (descriptor, input) {
        (EditReplayDescriptor::Map(footprint), DescriptorProjectionInput::Map(input)) => {
            prepare_map_replay(query, footprint, input).map(DescriptorProjection::Map)
        }
        (EditReplayDescriptor::Scenario, DescriptorProjectionInput::Scenario) => {
            Ok(DescriptorProjection::Scenario)
        }
        (EditReplayDescriptor::Appearance, DescriptorProjectionInput::Appearance) => {
            Ok(DescriptorProjection::Appearance)
        }
        _ => Err("semantic replay descriptor changed while projection was captured".to_owned()),
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

    /// Replay one semantic step through `EditRuntime`, then apply a projection derived from the
    /// exact authoritative replay descriptor. `EditStep` participates only in the temporary
    /// chronology stack and success-label compatibility until CP2H removes that duplicate stack.
    fn route_semantic(&mut self, direction: Direction, step: EditStep, route: &Route) {
        let (label, word) = (route.label(), direction.word());
        let Some(descriptor) = self.session.replay_descriptor(direction.gateway()) else {
            direction.restore(
                &mut self.terrain.history,
                &mut self.terrain.redo_stack,
                step,
            );
            self.ui
                .notify(format!("{label} {word} history is out of sync with the semantic session"));
            return;
        };
        if !route.matches_descriptor(&descriptor) {
            direction.restore(
                &mut self.terrain.history,
                &mut self.terrain.redo_stack,
                step,
            );
            self.ui.notify(format!(
                "{label} {word} marker does not match the authoritative semantic history; no state was changed"
            ));
            return;
        }
        let input = match &descriptor {
            EditReplayDescriptor::Map(footprint) => DescriptorProjectionInput::Map(
                capture_map_replay_input(footprint, |key| self.current_tile(key)),
            ),
            EditReplayDescriptor::Scenario => DescriptorProjectionInput::Scenario,
            EditReplayDescriptor::Appearance => DescriptorProjectionInput::Appearance,
        };
        let execution = self.session.replay(
            direction.gateway(),
            crate::app::edit_runtime::EditFootprint::default(),
            |document| prepare_descriptor_projection(document, &descriptor, input),
        );

        let notice = match execution {
            Ok(EditExecution::Committed {
                receipt,
                projection,
            }) => {
                let count = self.apply_descriptor_projection(projection);
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

    /// Apply an already-validated descriptor-driven projection mechanically. Projection invalidation
    /// is dispatched separately from the authoritative replay receipt after this returns.
    fn apply_descriptor_projection(&mut self, projection: DescriptorProjection) -> usize {
        match projection {
            DescriptorProjection::Scenario | DescriptorProjection::Appearance => 1,
            DescriptorProjection::Map(projection) => {
                let count = projection.count();
                let has_object_edits = !projection.object_edits.is_empty();
                let mut tile_edits = Vec::new();
                for tile in projection.tiles {
                    if tile.height_changed {
                        self.terrain.height_edited.insert(tile.key);
                    }
                    if tile.after != tile.before {
                        self.terrain.edited.insert(tile.key, tile.after);
                        tile_edits.push(self.edit_for(tile.key, tile.after));
                    }
                }
                self.world.project_tile_edits(tile_edits);
                if has_object_edits {
                    self.world.project_loc_edits(projection.object_edits);
                    self.clear_selection();
                    self.interaction.set_pending_reselect(projection.reselect);
                }
                count
            }
        }
    }
}

#[cfg(test)]
mod descriptor_tests;
#[cfg(test)]
mod footprint;
#[cfg(test)]
mod tests;
