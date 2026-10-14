//! Mixed editor history routing and worker projection.
//!
//! Semantic Paint, Height, Object, and Path actions keep their authoritative before/after state in
//! `EditorSession::history`. This module owns only the chronological application marker and the
//! transitional worker/render projection required after semantic undo/redo.

mod routing;

use super::{
    EditorApp, TileKey, path::project_semantic_path_values, project_height_values,
    project_paint_values,
};
use crate::app::object_tool::{ObjectHistoryRoute, build_object_history_projection};
use crate::paint::{Brush, TileValues};
use eframe::egui;
use osrs_world::{LocEdit, LocSpec};
use web_time::Instant;

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

/// One chronological editor-history marker.
///
/// Paint, Height, Objects, and Path use only semantic routing metadata. `changes`, `removed`, and
/// `added` remain transitional storage for editor actions that have not migrated to the persistent
/// session.
#[derive(Debug, Clone, Default)]
pub(in crate::app) struct EditStep {
    pub(super) changes: Vec<(TileKey, TileValues, TileValues)>,
    pub(super) removed: Vec<LocSpec>,
    pub(super) added: Vec<LocSpec>,
    pub(super) semantic_paint: Option<SemanticPaintHistory>,
    pub(super) semantic_height: Option<SemanticHeightHistory>,
    pub(super) semantic_object: Option<ObjectHistoryRoute>,
    pub(super) semantic_path: Option<SemanticPathHistory>,
}

impl EditStep {
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
    /// One finished undo step. Semantic tools carry only projection-routing metadata; remaining
    /// legacy actions still carry their before/after values here.
    pub(in crate::app) fn push_step(&mut self, step: EditStep) {
        self.history.push(step);
        if self.history.len() > HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.redo_stack.clear();
    }

    pub(in crate::app) fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub(in crate::app) fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub(super) fn revert(&mut self, step: &EditStep, forward: bool) {
        let mut edits = Vec::new();
        for (key, before, after) in &step.changes {
            let values = if forward { *after } else { *before };
            self.edited.insert(*key, values);
            edits.push(self.edit_for(*key, values));
        }
        self.send_edits(edits);
        if !step.removed.is_empty() || !step.added.is_empty() {
            let (gone, came) = if forward {
                (&step.removed, &step.added)
            } else {
                (&step.added, &step.removed)
            };
            let mut loc_edits: Vec<LocEdit> = gone.iter().map(|s| LocEdit::Remove(*s)).collect();
            loc_edits.extend(came.iter().map(|s| LocEdit::Add(*s)));
            self.send_loc_edits(loc_edits);
            self.interaction.pending_reselect = came.clone();
            self.clear_selection();
        }
    }

    fn project_semantic_paint_history(
        &mut self,
        paint: &SemanticPaintHistory,
    ) -> Result<usize, &'static str> {
        let mut projected = Vec::with_capacity(paint.keys.len());
        for key in &paint.keys {
            let current = self
                .current_tile(*key)
                .ok_or("a painted tile is no longer available to the projection adapter")?;
            let values = project_paint_values(self.session.document(), &paint.brush, *key, current)
                .ok_or("semantic Paint state cannot be represented by the legacy projection")?;
            projected.push((*key, current, values));
        }

        let mut edits = Vec::new();
        for (key, current, values) in projected {
            if values != current {
                self.edited.insert(key, values);
                edits.push(self.edit_for(key, values));
            }
        }
        self.send_edits(edits);
        Ok(paint.keys.len())
    }

    fn project_semantic_height_history(
        &mut self,
        height: &SemanticHeightHistory,
    ) -> Result<usize, &'static str> {
        let mut projected = Vec::with_capacity(height.keys.len());
        for key in &height.keys {
            let current = self
                .current_tile(*key)
                .ok_or("a Height tile is no longer available to the projection adapter")?;
            let values = project_height_values(self.session.document(), *key, current)
                .ok_or("semantic Height state cannot be represented by the legacy projection")?;
            projected.push((*key, current, values));
        }

        let mut edits = Vec::new();
        for (key, current, values) in projected {
            self.height_edited.insert(key);
            if values != current {
                self.edited.insert(key, values);
                edits.push(self.edit_for(key, values));
            }
        }
        self.send_edits(edits);
        Ok(height.keys.len())
    }

    fn project_semantic_path_history(
        &mut self,
        path: &SemanticPathHistory,
    ) -> Result<usize, &'static str> {
        let mut projected = Vec::with_capacity(path.keys.len());
        for key in &path.keys {
            let current = self
                .current_tile(*key)
                .ok_or("a Path tile is no longer available to the projection adapter")?;
            let values = project_semantic_path_values(
                self.session.document(),
                *key,
                current,
                path.include_height,
            )
            .ok_or("semantic Path state cannot be represented by the legacy projection")?;
            projected.push((*key, current, values));
        }

        let mut edits = Vec::new();
        for (key, current, values) in projected {
            if path.include_height {
                self.height_edited.insert(key);
            }
            if values != current {
                self.edited.insert(key, values);
                edits.push(self.edit_for(key, values));
            }
        }
        self.send_edits(edits);
        Ok(path.keys.len())
    }

    fn project_semantic_object_history(
        &mut self,
        object: &ObjectHistoryRoute,
        forward: bool,
    ) -> Result<usize, String> {
        let projection = build_object_history_projection(self.session.document(), object, forward)
            .map_err(|error| format!("{error:?}"))?;
        let count = object.object_count();
        self.send_loc_edits(projection.edits);
        self.clear_selection();
        self.interaction.pending_reselect = projection.reselect;
        Ok(count)
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
        let undo = ctx.input(|i| self.keymap.pressed(i, "edit.undo"));
        let redo = ctx.input(|i| self.keymap.pressed(i, "edit.redo"));
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
    }
}
