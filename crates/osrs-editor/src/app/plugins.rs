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
            colors: &self.prefs.colors,
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
            if self
                .prefs
                .disabled_plugins
                .iter()
                .any(|n| n == plugin.name())
            {
                continue;
            }
            let mut cx = PluginContext::new(self.editor_view(), &mut commands, plugin.name());
            f(plugin.as_mut(), &mut cx);
        }
        self.plugins = plugins;
        self.apply_commands(commands);
    }

    /// Fire one plugin action by id (a script or a button, rather than a key press).
    pub(super) fn plugin_action(&mut self, id: &'static str) {
        self.run_plugins(|plugin, cx| {
            if plugin.actions().iter().any(|action| action.id == id) {
                plugin.on_action(id, cx);
            }
        });
    }

    /// Whether a plugin is switched on.
    pub(super) fn plugin_enabled(&self, name: &str) -> bool {
        !self.prefs.disabled_plugins.iter().any(|n| n == name)
    }

    /// Switch a plugin on or off: off, it is not run anywhere and what it drew is gone.
    pub(super) fn set_plugin_enabled(&mut self, name: &str, enabled: bool) {
        self.prefs.disabled_plugins.retain(|n| n != name);
        if !enabled {
            self.prefs.disabled_plugins.push(name.to_owned());
        }
        let mut plugins = std::mem::take(&mut self.plugins);
        let mut commands = Vec::new();
        for plugin in &mut plugins {
            if plugin.name() != name || plugin.core() {
                continue;
            }
            let mut cx = PluginContext::new(self.editor_view(), &mut commands, plugin.name());
            plugin.set_enabled(enabled, &mut cx);
            if !enabled {
                commands.push((plugin.name(), EditorCommand::SetLines(Vec::new())));
            }
        }
        self.plugins = plugins;
        self.apply_commands(commands);
    }

    /// The Plugins window (menu bar): the user's own tools, and the plugins with a switch each.
    /// Switching a plugin off removes it everywhere; core plugins have no switch.
    pub(super) fn plugins_window(&mut self, ctx: &egui::Context) {
        if !self.plugins_open {
            return;
        }
        let ((), close) = super::modal::Modal::new("plugins_window", "Plugins")
            .size(440.0, 460.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        crate::theme::section_heading(ui, "Plugins");
                        let rows: Vec<(&'static str, &'static str, &'static str, bool)> = self
                            .plugins
                            .iter()
                            .map(|p| (p.name(), p.title(), p.description(), p.core()))
                            .collect();
                        for (name, title, description, core) in rows {
                            let mut on = self.plugin_enabled(name);
                            ui.horizontal(|ui| {
                                if core {
                                    ui.add_enabled(false, egui::Label::new(title));
                                    ui.weak("core");
                                } else if crate::theme::toggle(ui, &mut on, title).changed() {
                                    self.set_plugin_enabled(name, on);
                                }
                            });
                            if !description.is_empty() {
                                ui.indent(name, |ui| ui.weak(description));
                            }
                            ui.add_space(2.0);
                        }
                        ui.add_space(4.0);
                        ui.weak("A plugin that is off is gone: no menu, button, overlay or key.");
                        self.user_tool_list(ui);
                    });
            });
        if close {
            self.plugins_open = false;
        }
    }

    /// Fire the plugin actions whose bindings were pressed (no text field has focus).
    pub(super) fn plugin_actions(&mut self, ctx: &egui::Context) {
        if self.keys_blocked(ctx) {
            return;
        }
        let keymap = &self.keymap;
        let mut fired: Vec<&'static str> = Vec::new();
        ctx.input(|input| {
            for plugin in &self.plugins {
                if self
                    .prefs
                    .disabled_plugins
                    .iter()
                    .any(|n| n == plugin.name())
                {
                    continue;
                }
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
