//! Spline tool pointer input and semantic commit routing.
use super::*;
use crate::app::interaction_runtime::fit_points;

impl EditorApp {
    /// Click adds a point on the tile under the cursor; Enter lays the path.
    pub(in crate::app) fn spline_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        self.interaction.register_spline_repaint(ctx);
        if self.settings.prefs.tool != Tool::Spline {
            return;
        }
        self.spline_update();
        if self.ui.quick_open.is_some() || self.ui.context_menu.is_some() {
            return;
        }
        if !self.keys_blocked(ctx) {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.commit_spline();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Backspace)) {
                self.interaction.pop_spline_point();
            }
        }
        let Some(point) = pending_point(self) else {
            return;
        };
        if ctx.input(|i| i.pointer.primary_pressed())
            && response.is_pointer_button_down_on()
            && !ctx.input(|i| i.modifiers.alt)
        {
            self.interaction.add_spline_point(point);
        }
    }

    /// Lay the path as one semantic patch / one undo step, then (when asked) smooth the terrain
    /// under it as a second one. The path is fitted again from the placed points, so the cursor's
    /// preview is never part of what is laid.
    pub(in crate::app) fn commit_spline(&mut self) {
        if self.laying_overlay().is_none() {
            self.ui
                .notify("Choose an overlay to lay the path with".to_owned());
            return;
        }
        let points = self.interaction.spline_points().to_vec();
        if points.is_empty() {
            self.ui
                .notify("Click at least one point on the terrain first".to_owned());
            return;
        }
        let Some(overlay_id) = self.laying_overlay() else {
            return;
        };
        let options = PathOptions::from_settings(&self.settings.prefs.spline);
        let (line, paints) = fit_points(&points, None, options);
        if paints.is_empty() {
            self.ui
                .notify("The fitted path has no overlay tiles".to_owned());
            return;
        }
        let tiles: Vec<PathTile> = paints
            .iter()
            .map(|paint| PathTile {
                tile: (paint.tile_x, paint.tile_y),
                overlay: Some(overlay_id),
                shape: paint.shape,
                rotation: paint.orientation,
            })
            .collect();
        let changed = self.apply_terrain_path(&tiles);
        if self.settings.prefs.spline.smooth_height {
            terrain::smooth_height(self, &line);
        }
        self.interaction.clear_spline_points();
        self.ui.notify(format!("Laid a path over {changed} tiles"));
    }
}
