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

/// Transitional tool-specific metadata retained only until CP2I removes the old replay parity
/// oracle and its marker-producing call sites. These values are no longer stored chronologically or
/// consulted by production undo/redo.
#[derive(Debug, Clone)]
pub(super) struct SemanticPaintHistory {
    pub(super) brush: Brush,
    pub(super) keys: Vec<TileKey>,
}

#[derive(Debug, Clone)]
pub(super) struct SemanticHeightHistory {
    pub(super) keys: Vec<TileKey>,
}

#[derive(Debug, Clone)]
pub(super) struct SemanticPathHistory {
    pub(super) keys: Vec<TileKey>,
    pub(super) include_height: bool,
}

/// Transitional test/parity marker retained until CP2I.
///
/// Production chronology is exclusively `EditorSession::history`; `EditStep` is never stored by
/// the application after CP2H.
#[derive(Debug, Clone, Default)]
pub(in crate::app) struct EditStep {
    pub(super) semantic_paint: Option<SemanticPaintHistory>,
    pub(super) semantic_height: Option<SemanticHeightHistory>,
    pub(super) semantic_object: Option<ObjectHistoryRoute>,
    pub(super) semantic_path: Option<SemanticPathHistory>,
    pub(super) semantic_scenario: bool,
    pub(super) semantic_appearance: bool,
}

impl EditStep {
    pub(in crate::app) fn appearance() -> Self {
        Self {
            semantic_appearance: true,
            ..Default::default()
        }
    }
    pub(in crate::app) fn scenario() -> Self {
        Self {
            semantic_scenario: true,
            ..Default::default()
        }
    }
    pub(super) fn semantic_paint(brush: Brush, keys: Vec<TileKey>) -> Self {
        Self {
            semantic_paint: Some(SemanticPaintHistory { brush, keys }),
            ..Self::default()
        }
    }

    pub(super) fn semantic_height(keys: Vec<TileKey>) -> Self {
        Self {
            semantic_height: Some(SemanticHeightHistory { keys }),
            ..Self::default()
        }
    }

    pub(in crate::app) fn semantic_object(removed: Vec<LocSpec>, added: Vec<LocSpec>) -> Self {
        Self {
            semantic_object: Some(ObjectHistoryRoute::new(removed, added)),
            ..Self::default()
        }
    }

    /// One semantic transaction that changed terrain and/or objects together (a prefab or a
    /// generated building). Retained only as the CP2I parity oracle.
    pub(super) fn semantic_composite(
        keys: Vec<TileKey>,
        include_height: bool,
        objects: Option<ObjectHistoryRoute>,
    ) -> Self {
        Self {
            semantic_path: (!keys.is_empty()).then_some(SemanticPathHistory {
                keys,
                include_height,
            }),
            semantic_object: objects,
            ..Self::default()
        }
    }

    pub(super) fn semantic_path(keys: Vec<TileKey>, include_height: bool) -> Self {
        Self {
            semantic_path: Some(SemanticPathHistory {
                keys,
                include_height,
            }),
            ..Self::default()
        }
    }
}

impl super::TerrainController {
    /// Compatibility sink removed in CP2I. Scenario chronology is already recorded by
    /// `EditorSession`; this method deliberately stores nothing.
    pub(in crate::app) fn record_scenario(&mut self) {}
}

impl EditorApp {
    /// Compatibility sink for marker-producing commit paths that CP2I removes. The semantic
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
