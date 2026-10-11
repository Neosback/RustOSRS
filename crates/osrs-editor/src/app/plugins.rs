//! Glue between the editor and its [`EditorPlugin`]s.

use super::EditorApp;
use crate::plugin::{EditorCommand, EditorPlugin, EditorView, PluginContext};
use eframe::egui;

impl EditorApp {
    fn editor_view(&self) -> EditorView<'_> {
        EditorView {
            camera_tile: self.camera_tile(),
            camera_region: self.camera_region(),
            view_plane: self.view_plane,
            render_origin: self.renderer.render_origin(),
            infos: &self.infos,
            infos_version: self.infos_version,
            selected: self.selected.as_ref(),
        }
    }

    /// Run `f` for every plugin, then apply the commands they issued.
    pub(super) fn run_plugins(
        &mut self,
        mut f: impl FnMut(&mut dyn EditorPlugin, &mut PluginContext),
    ) {
        let mut plugins = std::mem::take(&mut self.plugins);
        let mut commands = Vec::new();
        for plugin in &mut plugins {
            let mut cx = PluginContext::new(self.editor_view(), &mut commands, plugin.name());
            f(plugin.as_mut(), &mut cx);
        }
        self.plugins = plugins;
        self.apply_commands(commands);
    }

    /// Fire the plugin actions whose bindings were pressed (no text field has focus).
    pub(super) fn plugin_actions(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let keymap = &self.keymap;
        let mut fired: Vec<&'static str> = Vec::new();
        ctx.input(|input| {
            for plugin in &self.plugins {
                for action in plugin.actions() {
                    if keymap.pressed(input, action.id) {
                        fired.push(action.id);
                    }
                }
            }
        });
        if fired.is_empty() {
            return;
        }
        self.run_plugins(|plugin, cx| {
            for action in plugin.actions() {
                if fired.contains(&action.id) {
                    plugin.on_action(action.id, cx);
                }
            }
        });
    }

    fn apply_commands(&mut self, commands: Vec<(&'static str, EditorCommand)>) {
        let mut lines_changed = false;
        for (plugin, command) in commands {
            match command {
                EditorCommand::Teleport(destination) => self.teleport_to(destination),
                EditorCommand::SetPlane(plane) => self.view_plane = plane.min(3),
                EditorCommand::SetLines(lines) => {
                    self.plugin_lines.insert(plugin, lines);
                    lines_changed = true;
                }
                EditorCommand::Status(message) => self.status = Some(message),
                EditorCommand::SetAnimationSmoothing(enabled) => {
                    self.animation_smoothing = enabled;
                    self.animations.set_smoothing(enabled);
                }
            }
        }
        if lines_changed {
            let merged: Vec<_> = self.plugin_lines.values().flatten().copied().collect();
            self.renderer.set_overlay_lines(&merged);
        }
    }
}
