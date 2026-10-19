//! Authentic OSRS gameframe HUD: Minimap orbs (Hitpoints, Prayer, Run, Special attack),
//! Side-panel stone tab rows with authentic icons, and resizable Chatbox frame.
//!
//! Uses decoded interface sprites from cache index 8 with vector fallbacks when sprites
//! are loading or unavailable.

mod chatbox;
mod orbs;
mod tabs;

use super::EditorApp;
use crate::camera::CameraMode;
use eframe::egui::{self, Rect};

/// Top-level gameframe drawer, called by `viewport_overlays`.
pub(in crate::app) fn draw(app: &mut EditorApp, ui: &egui::Ui, rect: Rect) {
    let in_game = app.interaction.camera().mode == CameraMode::Game;
    if !in_game && !app.settings.prefs.show_gameframe {
        return;
    }
    let scale = app.settings.prefs.minimap_size.scale();
    orbs::draw(app, ui, rect, scale);
    tabs::draw(app, ui, rect);
    chatbox::draw(app, ui, rect);
}
