//! Keyboard actions and viewport input dispatch.

use super::*;

impl EditorApp {
    /// Fire the editor's own bound actions (none while a text field has keyboard focus, and none
    /// while a binding is being captured in the Keyboard Shortcuts window).
    pub(super) fn handle_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
        let delta = response.drag_delta();
        let shift = ctx.input(|i| i.modifiers.shift);
        let pan = response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged_by(egui::PointerButton::Secondary) && shift);
        let (scroll, pinch) = if response.hovered() {
            ctx.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta() - 1.0))
        } else {
            (0.0, 0.0)
        };
        let movement = if self.keys_blocked(ctx) {
            [0.0; 3]
        } else {
            ctx.input(|i| {
                let axis = |plus: &str, minus: &str| {
                    f32::from(self.settings.keymap.held(i, plus))
                        - f32::from(self.settings.keymap.held(i, minus))
                };
                let raw_dir = [
                    axis("camera.forward", "camera.back"),
                    axis("camera.right", "camera.left"),
                    axis("camera.up", "camera.down"),
                ];
                let len =
                    (raw_dir[0] * raw_dir[0] + raw_dir[1] * raw_dir[1] + raw_dir[2] * raw_dir[2])
                        .sqrt();
                let norm = if len > 1.0 { 1.0 / len } else { 1.0 };
                let boost = if i.modifiers.shift {
                    4.0
                } else if i.modifiers.ctrl || i.modifiers.command {
                    0.25
                } else {
                    1.0
                };
                [
                    raw_dir[0] * norm * boost,
                    raw_dir[1] * norm * boost,
                    raw_dir[2] * norm * boost,
                ]
            })
        };
        self.interaction
            .apply_camera_input(interaction_runtime::CameraInput {
                delta: [delta.x, delta.y],
                pan,
                secondary_drag: response.dragged_by(egui::PointerButton::Secondary),
                shift,
                scroll,
                pinch,
                hovered: response.hovered(),
                movement,
                dt,
            });
    }
}

pub(super) fn editor_actions(app: &mut EditorApp, ctx: &egui::Context, include_projects: bool) {
    app.tool_release(ctx);
    if app.keys_blocked(ctx) {
        return;
    }
    let pressed = |id: &str| ctx.input(|i| app.settings.keymap.pressed(i, id));
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
        app.toggle_world_map();
    }
    if open_settings {
        app.open_settings(None);
        app.ui.settings_focus_search = true;
    }
    if switch_menu {
        app.settings.prefs.context_menu = match app.settings.prefs.context_menu {
            crate::settings::ContextMenuStyle::Editor => crate::settings::ContextMenuStyle::Game,
            crate::settings::ContextMenuStyle::Game => crate::settings::ContextMenuStyle::Editor,
        };
        app.ui.notify(format!(
            "Context menu: {}",
            app.settings.prefs.context_menu.label()
        ));
    }
    if open_teleport {
        app.open_teleport();
    }
    if toggle_panel {
        app.settings.prefs.properties_panel = !app.settings.prefs.properties_panel;
    }
    if toggle_camera {
        app.interaction.toggle_camera_mode();
    }
    if focus {
        app.focus_selection();
    }
    if clear {
        app.escape_action();
    }
    if let Some(plane) = plane {
        app.interaction.switch_plane(plane);
    }
    app.tool_actions(ctx);
    app.edit_actions(ctx);
    app.clipboard_actions(ctx);
    if include_projects {
        app.project_actions(ctx);
    }
    app.script_frame(ctx);
    app.object_actions(ctx);
    app.resolve_reselect();
}
