//! Semantic editor history routing and worker projection.
//!
//! Paint, Height, Object, Path, Scenario, and Appearance actions keep authoritative before/after
//! state in `EditorSession::history`. This module owns only chronological application markers and
//! the transitional worker/render projection needed after semantic undo/redo.

mod replay_projection;
mod routing;

use super::{EditorApp, TileKey};
use crate::app::object_tool::ObjectHistoryRoute;
use crate::paint::Brush;
use eframe::egui;
use osrs_world::{LocEdit, LocSpec};

/// Most global undo steps kept by the editor shell.
const HISTORY_LIMIT: usize = 200;

/// Global-history routing metadata for one semantic Paint transaction.
///
/// The authoritative before/after state stays in `EditorSession::history`; this stores only enough
/// adapter information to project semantic undo/redo back into the still-transitional worker path.
#[derive(Debug, Clone)]
pub(super) struct SemanticPaintHistory {
    pub(super) brush: Brush,
    pub(super) keys: Vec<TileKey>,
}

/// Global-history routing metadata for one semantic Height transaction.
///
/// Heights themselves remain exclusively in `EditorSession::history`; the keys only tell the
/// worker/render projection which tiles need to mirror the authoritative semantic state.
#[derive(Debug, Clone)]
pub(super) struct SemanticHeightHistory {
    pub(super) keys: Vec<TileKey>,
}

/// Global-history routing metadata for one semantic terrain-Path transaction.
///
/// Terrain and optional Height before/after state remain exclusively in `EditorSession::history`.
/// The route only identifies touched worker tiles and whether Height participates in projection.
#[derive(Debug, Clone)]
pub(super) struct SemanticPathHistory {
    pub(super) keys: Vec<TileKey>,
    pub(super) include_height: bool,
}

/// One chronological semantic editor-history marker.
///
/// This type stores routing metadata only. Authoritative before/after state remains in
/// `EditorSession::history`; `EditStep` never carries duplicate tile or object payload authority.
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
    /// generated building). Both routes set means undo and redo replay them as one step.
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

impl EditorApp {
    /// One finished semantic undo marker.
    pub(in crate::app) fn push_step(&mut self, step: EditStep) {
        self.terrain.history.push(step);
        if self.terrain.history.len() > HISTORY_LIMIT {
            self.terrain.history.remove(0);
        }
        self.terrain.redo_stack.clear();
    }

    pub(in crate::app) fn can_undo(&self) -> bool {
        !self.terrain.history.is_empty()
    }

    pub(in crate::app) fn can_redo(&self) -> bool {
        !self.terrain.redo_stack.is_empty()
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
