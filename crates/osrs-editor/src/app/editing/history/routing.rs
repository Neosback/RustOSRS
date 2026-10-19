//! Undo and redo projection from authoritative semantic history.
//!
//! `EditorSession` owns the only chronology. `EditRuntime` derives the exact replay descriptor from
//! that history, checkpoints the semantic transition, and restores it exactly if projection fails.
//! The application keeps only disposable worker/UI projection state.

use super::replay_projection::{
    MapReplayInput, PreparedMapReplay, capture_map_replay_input, prepare_map_replay,
};
use super::EditorApp;
use crate::app::edit_runtime::{EditExecution, EditExecutionError, EditReplayDirection};
use crate::session::EditReplayDescriptor;
use osrs_edit::MapQuery;

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

fn descriptor_label(descriptor: &EditReplayDescriptor) -> &'static str {
    match descriptor {
        EditReplayDescriptor::Map(_) => "Map edit",
        EditReplayDescriptor::Scenario => "NPC scenario",
        EditReplayDescriptor::Appearance => "Appearance",
    }
}

fn descriptor_counted(descriptor: &EditReplayDescriptor, count: usize) -> String {
    let noun = match descriptor {
        EditReplayDescriptor::Map(_) => "map change",
        EditReplayDescriptor::Scenario => "NPC scenario change",
        EditReplayDescriptor::Appearance => "appearance change",
    };
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
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
        self.route_semantic(Direction::Undo);
    }

    pub(in crate::app) fn redo(&mut self) {
        self.route_semantic(Direction::Redo);
    }

    /// Replay the exact authoritative semantic history head and apply its already-validated
    /// projection. No application-level marker stack participates in ordering or rollback.
    fn route_semantic(&mut self, direction: Direction) {
        let Some(descriptor) = self.session.replay_descriptor(direction.gateway()) else {
            return;
        };
        let label = descriptor_label(&descriptor);
        let word = direction.word();
        let input = match &descriptor {
            EditReplayDescriptor::Map(footprint) => DescriptorProjectionInput::Map(
                capture_map_replay_input(footprint, |key| self.current_tile(key)),
            ),
            EditReplayDescriptor::Scenario => DescriptorProjectionInput::Scenario,
            EditReplayDescriptor::Appearance => DescriptorProjectionInput::Appearance,
        };
        let execution = self.session.replay(direction.gateway(), |document| {
            prepare_descriptor_projection(document, &descriptor, input)
        });

        let notice = match execution {
            Ok(EditExecution::Committed {
                receipt,
                projection,
            }) => {
                let count = self.apply_descriptor_projection(projection);
                self.world.invalidate_semantic_edit(&receipt);
                format!(
                    "{} {}",
                    direction.past(),
                    descriptor_counted(&descriptor, count)
                )
            }
            Ok(EditExecution::NoChange) => {
                format!("{label} {word} history is out of sync with the semantic session")
            }
            Err(EditExecutionError::Semantic(error)) => {
                format!("{label} {word} was rejected by the semantic session: {error:?}")
            }
            Err(EditExecutionError::Projection(message)) => {
                format!("{label} {word} was rolled back: {message}")
            }
            Err(EditExecutionError::Rollback {
                projection,
                rollback,
            }) => {
                format!(
                    "{label} {word} projection failed and semantic rollback failed: \
                     {projection} ({rollback:?})"
                )
            }
            Err(EditExecutionError::Finalize(error)) => {
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
