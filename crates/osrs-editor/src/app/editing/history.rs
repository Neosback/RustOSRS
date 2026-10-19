//! Semantic editor undo/redo projection.
//!
//! Paint, Height, Object, Path, Scenario, and Appearance actions keep authoritative before/after
//! state and chronological ordering in `EditorSession::history`. This module owns only the
//! transitional worker/render projection needed after semantic undo/redo.

mod replay_projection;
mod routing;

use super::{EditorApp, TileKey};
use crate::app::edit_runtime::EditReplayDirection;
use crate::app::object_tool::ObjectHistoryRoute;
use crate::paint::Brush;
use eframe::egui;
use osrs_world::{LocEdit, LocSpec};

/// Zero-state compatibility marker for commit paths that still call the retired application-history
/// API. It carries no routing metadata, is never stored, and is never consulted by undo/redo.
///
/// CP2J removes the remaining producer calls together with the architecture ratchet that prevents a
/// second application chronology from being reintroduced.
#[derive(Debug, Clone, Copy, Default)]
pub(in crate::app) struct EditStep;

impl EditStep {
    pub(in crate::app) fn appearance() -> Self {
        Self
    }

    pub(in crate::app) fn scenario() -> Self {
        Self
    }

    pub(super) fn semantic_paint(_brush: Brush, _keys: Vec<TileKey>) -> Self {
        Self
    }

    pub(super) fn semantic_height(_keys: Vec<TileKey>) -> Self {
        Self
    }

    pub(in crate::app) fn semantic_object(_removed: Vec<LocSpec>, _added: Vec<LocSpec>) -> Self {
        Self
    }

    pub(super) fn semantic_composite(
        _keys: Vec<TileKey>,
        _include_height: bool,
        _objects: Option<ObjectHistoryRoute>,
    ) -> Self {
        Self
    }

    pub(super) fn semantic_path(_keys: Vec<TileKey>, _include_height: bool) -> Self {
        Self
    }
}

impl super::TerrainController {
    /// Compatibility sink for scenario commit paths. Scenario chronology is already recorded by
    /// `EditorSession`; this method deliberately stores nothing.
    pub(in crate::app) fn record_scenario(&mut self) {}
}

impl EditorApp {
    /// Compatibility sink for commit paths that still construct `EditStep`. The semantic
    /// transaction is already present in `EditorSession::history`; no second chronology is kept.
    pub(in crate::app) fn push_step(&mut self, _step: EditStep) {}

    pub(in crate::app) fn can_undo(&self) -> bool {
        self.session
            .replay_descriptor(EditReplayDirection::Undo)
            .is_some()
    }

    pub(in crate::app) fn can_redo(&self) -> bool {
        self.session
            .replay_descriptor(EditReplayDirection::Redo)
            .is_some()
    }

    /// Hand object edits to the world worker and queue the regions they sit in for a rebuild.
    pub(in crate::app) fn send_loc_edits(&mut self, edits: Vec<LocEdit>) {
        if edits.is_empty() {
            return;
        }
        self.world.mirror_loc_edits(edits);
    }

    /// Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z.
    pub(in crate::app) fn edit_actions(&mut self, ctx: &egui::Context) {
        let undo = ctx.input(|i| self.settings.keymap.pressed(i, "edit.undo"));
        let redo = ctx.input(|i| self.settings.keymap.pressed(i, "edit.redo"));
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
    }
}
