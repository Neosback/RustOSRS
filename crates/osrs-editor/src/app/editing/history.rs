//! Semantic editor undo/redo projection.
//!
//! Paint, Height, Object, Path, Scenario, and Appearance actions keep authoritative before/after
//! state and chronological ordering in `EditorSession::history`. This module owns only the
//! transitional worker/render projection needed after semantic undo/redo.

mod replay_projection;
mod routing;

use super::EditorApp;
use crate::app::edit_runtime::EditReplayDirection;
use eframe::egui;
use osrs_world::LocEdit;

impl EditorApp {
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

    pub(in crate::app) fn send_loc_edits(&mut self, edits: Vec<LocEdit>) {
        self.world.project_loc_edits(edits);
    }

    pub(in crate::app) fn edit_actions(&mut self, ctx: &egui::Context) {
        if self.keys_blocked(ctx) {
            return;
        }
        let (undo, redo) = ctx.input(|i| {
            (
                self.settings.keymap.pressed(i, "edit.undo"),
                self.settings.keymap.pressed(i, "edit.redo"),
            )
        });
        if undo {
            self.undo();
        } else if redo {
            self.redo();
        }
    }
}
