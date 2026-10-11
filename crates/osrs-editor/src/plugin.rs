//! Editor plugin interface.
//!
//! A plugin is a self-contained tool or overlay. It contributes any of: a View-menu submenu, a
//! Properties section, a viewport overlay, hotkeys, per-frame work, viewport guide lines and
//! persisted settings. Plugins never touch editor internals; they read an [`EditorView`] and
//! push [`EditorCommand`]s that the editor applies after the plugin returns.
//!
//! Built-in features that are optional by nature (the grid today; measurement, bookmarks, region
//! exporters later) are written against this interface so `app.rs` stays small.

use crate::hud::Destination;
use crate::keymap::ActionDef;
use crate::pick::{Infos, ObjectPick};
use eframe::egui;
use osrs_render::gpu::LineVertex;

/// Read-only editor state offered to plugins.
#[allow(
    dead_code,
    reason = "plugin API surface; fields are for tools that are not written yet"
)]
pub struct EditorView<'a> {
    /// World tile under the camera.
    pub camera_tile: (i32, i32),
    pub camera_region: (i32, i32),
    /// Plane being viewed (`0..=3`).
    pub view_plane: u8,
    /// World point that renders at local `(0, 0)`; subtract it from world positions when
    /// producing [`LineVertex`] positions.
    pub render_origin: (i32, i32),
    /// Inspection data of the resident regions.
    pub infos: &'a Infos,
    /// Bumped whenever `infos` gains or loses a region.
    pub infos_version: u64,
    pub selected: Option<&'a ObjectPick>,
}

/// Requests a plugin makes of the editor.
#[allow(
    dead_code,
    reason = "plugin API surface; variants are for tools that are not written yet"
)]
pub enum EditorCommand {
    Teleport(Destination),
    SetPlane(u8),
    /// Replace the plugin's guide lines (render-local positions; empty clears them).
    SetLines(Vec<LineVertex>),
    /// Message for the status bar.
    Status(String),
    /// Blend animation frames (RuneLite's Animation Smoothing plugin).
    SetAnimationSmoothing(bool),
}

/// What a plugin sees and can ask for while it runs.
pub struct PluginContext<'a> {
    pub view: EditorView<'a>,
    commands: &'a mut Vec<(&'static str, EditorCommand)>,
    plugin: &'static str,
}

impl<'a> PluginContext<'a> {
    pub fn new(
        view: EditorView<'a>,
        commands: &'a mut Vec<(&'static str, EditorCommand)>,
        plugin: &'static str,
    ) -> Self {
        Self {
            view,
            commands,
            plugin,
        }
    }

    pub fn send(&mut self, command: EditorCommand) {
        self.commands.push((self.plugin, command));
    }
}

pub trait EditorPlugin {
    /// Stable identifier (settings key, line-set key).
    fn name(&self) -> &'static str;

    /// Title of the View-menu submenu and Properties section.
    fn title(&self) -> &'static str;

    /// Per-frame work before the UI is drawn (rebuild guide lines, poll tasks).
    fn update(&mut self, _cx: &mut PluginContext) {}

    /// Contents of the plugin's submenu under View. Leave empty for no submenu.
    fn menu(&mut self, _ui: &mut egui::Ui, _cx: &mut PluginContext) {}

    /// Contents of the plugin's Properties section. Leave empty for no section.
    fn properties(&mut self, _ui: &mut egui::Ui, _cx: &mut PluginContext) {}

    /// Whether [`Self::menu`] / [`Self::properties`] draw anything.
    fn has_menu(&self) -> bool {
        false
    }
    fn has_properties(&self) -> bool {
        false
    }

    /// Compact controls for the toolbar under the menu bar (buttons, toggles).
    fn toolbar(&mut self, _ui: &mut egui::Ui, _cx: &mut PluginContext) {}

    /// Buttons shown with the selected object (Object tab).
    fn selection_ui(&mut self, _ui: &mut egui::Ui, _cx: &mut PluginContext) {}

    /// Draw over the 3D viewport.
    fn overlay(&mut self, _ctx: &egui::Context, _viewport: egui::Rect, _cx: &mut PluginContext) {}

    /// Rebindable actions the plugin offers (registered in the keymap, listed in the Keyboard
    /// Shortcuts window).
    fn actions(&self) -> &'static [ActionDef] {
        &[]
    }

    /// An action of this plugin fired (its binding was pressed, no text field has focus).
    fn on_action(&mut self, _id: &'static str, _cx: &mut PluginContext) {}

    /// State to persist (stored under the plugin's name in the settings file).
    fn save_settings(&self) -> Option<serde_json::Value> {
        None
    }

    /// Restore persisted state; `Null` when nothing was saved.
    fn load_settings(&mut self, _value: &serde_json::Value) {}
}
