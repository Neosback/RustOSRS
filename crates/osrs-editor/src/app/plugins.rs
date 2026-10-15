//! Glue between the editor and its [`EditorPlugin`]s.

use super::EditorApp;
use crate::plugin::{EditorCommand, EditorPlugin, EditorView, PluginContext};
use eframe::egui;

impl EditorApp {
    /// Run `f` for every plugin, then apply the commands they issued.
    pub(super) fn run_plugins(&mut self, f: impl FnMut(&mut dyn EditorPlugin, &mut PluginContext)) {
        let view = EditorView {
            camera_tile: self.camera_tile(),
            camera_region: self.camera_region(),
            view_plane: self.interaction.view_plane(),
            render_origin: self.renderer.render_origin(),
            infos: self.world.infos(),
            infos_version: self.world.infos_version(),
            colors: &self.settings.prefs.colors,
            selected: self.interaction.selected().as_ref(),
        };
        let commands = self
            .plugins
            .run(view, &self.settings.prefs.disabled_plugins, f);
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
        !self
            .settings
            .prefs
            .disabled_plugins
            .iter()
            .any(|n| n == name)
    }

    /// Switch a plugin on or off: off, it is not run anywhere and what it drew is gone.
    pub(super) fn set_plugin_enabled(&mut self, name: &str, enabled: bool) {
        self.settings.prefs.disabled_plugins.retain(|n| n != name);
        if !enabled {
            self.settings.prefs.disabled_plugins.push(name.to_owned());
        }
        let view = EditorView {
            camera_tile: self.camera_tile(),
            camera_region: self.camera_region(),
            view_plane: self.interaction.view_plane(),
            render_origin: self.renderer.render_origin(),
            infos: self.world.infos(),
            infos_version: self.world.infos_version(),
            colors: &self.settings.prefs.colors,
            selected: self.interaction.selected().as_ref(),
        };
        let commands = self.plugins.change_enabled(view, name, enabled);
        self.apply_commands(commands);
    }

    /// The Plugins window (menu bar): the user's own tools, and the plugins with a switch each.
    /// Switching a plugin off removes it everywhere; core plugins have no switch.
    pub(super) fn plugins_window(&mut self, ctx: &egui::Context) {
        if !self.ui.plugins_open {
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
                            .plugins()
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
            self.ui.plugins_open = false;
        }
    }

    /// Fire the plugin actions whose bindings were pressed (no text field has focus).
    pub(super) fn plugin_actions(&mut self, ctx: &egui::Context) {
        if self.keys_blocked(ctx) {
            return;
        }
        let keymap = &self.settings.keymap;
        let mut fired: Vec<&'static str> = Vec::new();
        ctx.input(|input| {
            for plugin in self.plugins.plugins() {
                if self
                    .settings
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
        for (plugin, command) in commands {
            match command {
                EditorCommand::Teleport(destination) => self.teleport_to(destination),
                EditorCommand::SetPlane(plane) => self.interaction.set_view_plane(plane),
                EditorCommand::SetLines(lines) => {
                    self.plugins.set_lines(plugin, lines);
                }
                EditorCommand::Status(message) => self.ui.set_status(message),
                EditorCommand::SetAnimationSmoothing(enabled) => {
                    self.world.set_animation_smoothing(enabled);
                }
            }
        }
        if let Some(merged) = self.plugins.take_lines_update() {
            self.renderer.set_overlay_lines(&merged);
        }
    }
}
