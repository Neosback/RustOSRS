//! Spline tool pointer input and semantic commit routing.
use super::*;

impl EditorApp {
    /// Add, move, insert and remove control points; Enter lays the path.
    pub(in crate::app) fn spline_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        self.interaction.register_spline_repaint(ctx);
        if self.settings.prefs.tool != Tool::Spline {
            self.interaction.cancel_spline_drag();
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
        let Some(hit) = self.interaction.hover().tile else {
            if !ctx.input(|i| i.pointer.primary_down()) {
                self.interaction.finish_spline_drag();
            }
            return;
        };
        let at = if self.settings.prefs.spline.snap_points {
            (hit.tile.0 as f32 + 0.5, hit.tile.1 as f32 + 0.5)
        } else {
            hit.at
        };
        let modifiers = ctx.input(|i| i.modifiers);
        let nearest = self
            .interaction
            .spline_points()
            .iter()
            .enumerate()
            .map(|(i, p)| (i, distance(*p, at)))
            .filter(|(_, d)| *d <= GRAB_DISTANCE)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        if !self.keys_blocked(ctx) && ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            match nearest {
                Some(index) => self.interaction.remove_spline_point(index),
                None => self.interaction.pop_spline_point(),
            }
        }
        if ctx.input(|i| i.pointer.primary_pressed()) && response.is_pointer_button_down_on() {
            if modifiers.alt {
                return;
            }
            match nearest {
                Some(index) if modifiers.command => {
                    self.interaction.remove_spline_point(index);
                }
                Some(index) => self.interaction.begin_spline_drag(index),
                None if modifiers.shift && self.interaction.spline_points().len() >= 2 => {
                    // Insert on the stretch of the control polygon nearest the click.
                    let points = self.interaction.spline_points();
                    let mut best = (f32::MAX, 0);
                    for i in 0..points.len() - 1 {
                        let d = segment_distance(at, points[i], points[i + 1]);
                        if d < best.0 {
                            best = (d, i);
                        }
                    }
                    self.interaction
                        .insert_spline_point_and_drag(best.1 + 1, at);
                }
                None => self.interaction.append_spline_point_and_drag(at),
            }
        }
        if ctx.input(|i| i.pointer.primary_down()) {
            self.interaction.update_spline_drag(at);
        } else {
            self.interaction.finish_spline_drag();
        }
    }

    /// Lay the previewed fitted path as one semantic patch / one undo step.
    pub(in crate::app) fn commit_spline(&mut self) {
        if self.laying_overlay().is_none() {
            self.ui
                .notify("Choose an overlay to lay the path with".to_owned());
            return;
        }
        self.spline_plan_now();
        let Some(preview) = self.interaction.current_spline_preview().cloned() else {
            return;
        };
        if preview.paints.is_empty() {
            self.ui
                .notify("The fitted path has no overlay tiles".to_owned());
            return;
        }
        let tiles: Vec<PathTile> = preview
            .paints
            .iter()
            .map(|paint| PathTile {
                tile: (paint.tile_x, paint.tile_y),
                overlay: Some(preview.overlay_id),
                shape: paint.shape,
                rotation: paint.orientation,
            })
            .collect();
        let changed = self.apply_terrain_path(&tiles);
        self.interaction.clear_spline_points();
        self.ui
            .notify(format!("Laid a fitted path over {changed} tiles"));
    }
}
