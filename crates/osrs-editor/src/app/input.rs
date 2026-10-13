//! Keyboard actions and viewport input dispatch.

use super::*;

impl EditorApp {
    /// Fire the editor's own bound actions (none while a text field has keyboard focus, and none
    /// while a binding is being captured in the Keyboard Shortcuts window).
    pub(super) fn editor_actions(&mut self, ctx: &egui::Context) {
        self.tool_release(ctx);
        if self.keys_blocked(ctx) {
            return;
        }
        let pressed = |id: &str| ctx.input(|i| self.keymap.pressed(i, id));
        let mut open_teleport = false;
        let mut toggle_panel = false;
        let mut toggle_camera = false;
        let mut focus = false;
        let mut clear = false;
        let mut plane = None;
        if pressed("teleport.open") {
            open_teleport = true;
        }
        if pressed("panel.properties") {
            toggle_panel = true;
        }
        if pressed("camera.mode") {
            toggle_camera = true;
        }
        if pressed("camera.focus") {
            focus = true;
        }
        if pressed("selection.clear") {
            clear = true;
        }
        let toggle_map = pressed("worldmap.toggle");

        let open_settings = pressed("settings.open");
        let switch_menu = pressed("contextmenu.style");
        for (index, id) in ["plane.0", "plane.1", "plane.2", "plane.3"]
            .into_iter()
            .enumerate()
        {
            if pressed(id) {
                plane = Some(index as u8);
            }
        }
        if toggle_map {
            self.toggle_world_map();
        }
        if open_settings {
            self.open_settings(None);
            self.settings_focus_search = true;
        }
        if switch_menu {
            self.prefs.context_menu = match self.prefs.context_menu {
                crate::settings::ContextMenuStyle::Editor => {
                    crate::settings::ContextMenuStyle::Game
                }
                crate::settings::ContextMenuStyle::Game => {
                    crate::settings::ContextMenuStyle::Editor
                }
            };
            self.notice = Some((
                format!("Context menu: {}", self.prefs.context_menu.label()),
                Instant::now(),
            ));
        }
        if open_teleport {
            self.open_teleport();
        }
        if toggle_panel {
            self.prefs.properties_panel = !self.prefs.properties_panel;
        }
        if toggle_camera {
            self.camera.mode = match self.camera.mode {
                CameraMode::Fly => CameraMode::Orbit,
                CameraMode::Orbit => CameraMode::Fly,
            };
        }
        if focus {
            self.focus_selection();
        }
        if clear {
            self.escape_action();
        }
        if let Some(plane) = plane {
            self.view_plane = plane;
        }
        self.tool_actions(ctx);
        self.edit_actions(ctx);
        self.script_frame(ctx);
        self.object_actions(ctx);
        self.resolve_reselect();
    }

    pub(super) fn handle_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
        let delta = response.drag_delta();
        let shift = ctx.input(|i| i.modifiers.shift);
        // Right drag moves the view in both modes; the left button is left to selection and tools.
        match self.camera.mode {
            CameraMode::Fly => {
                if response.dragged_by(egui::PointerButton::Secondary) && !shift {
                    self.camera.look(delta.x, delta.y);
                }
                if response.dragged_by(egui::PointerButton::Middle)
                    || (response.dragged_by(egui::PointerButton::Secondary) && shift)
                {
                    self.camera.pan(delta.x, delta.y);
                }
            }
            CameraMode::Orbit => {
                if response.dragged_by(egui::PointerButton::Middle)
                    || (response.dragged_by(egui::PointerButton::Secondary) && shift)
                {
                    self.camera.pan(delta.x, delta.y);
                } else if response.dragged_by(egui::PointerButton::Secondary) {
                    self.camera.orbit(delta.x, delta.y);
                }
            }
        }
        if response.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            match self.camera.mode {
                CameraMode::Fly => {
                    if scroll != 0.0 {
                        self.camera.speed =
                            (self.camera.speed * (1.0 + scroll * 0.002)).clamp(50.0, 20_000.0);
                    }
                }
                CameraMode::Orbit => {
                    let pinch = ctx.input(|i| i.zoom_delta());
                    self.camera.zoom(scroll + (pinch - 1.0) * 500.0);
                }
            }
        }
        if self.keys_blocked(ctx) {
            return;
        }
        ctx.input(|i| {
            let axis = |plus: &str, minus: &str| {
                f32::from(self.keymap.held(i, plus)) - f32::from(self.keymap.held(i, minus))
            };
            let boost = if i.modifiers.shift { 4.0 } else { 1.0 };
            let forward = axis("camera.forward", "camera.back") * boost;
            let right = axis("camera.right", "camera.left") * boost;
            let up = axis("camera.up", "camera.down") * boost;
            if forward != 0.0 || right != 0.0 || up != 0.0 {
                self.camera.translate(forward, right, up, dt);
            }
        });
    }
}
