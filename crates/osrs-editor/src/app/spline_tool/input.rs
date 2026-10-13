//! Spline tool pointer input and commit.

use super::*;

impl EditorApp {
    /// Add, move, insert and remove control points; Enter lays the path.
    pub(in crate::app) fn spline_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        self.interaction
            .spline
            .ctx
            .get_or_insert_with(|| ctx.clone());
        if self.prefs.tool != Tool::Spline {
            self.interaction.spline.drag = None;
            return;
        }
        self.spline_update();
        if self.quick_open.is_some() || self.context_menu.is_some() {
            return;
        }
        if !self.keys_blocked(ctx) {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.commit_spline();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Backspace)) {
                self.interaction.spline.points.pop();
            }
        }
        let Some(hit) = self.interaction.hover.tile else {
            if !ctx.input(|i| i.pointer.primary_down()) {
                self.interaction.spline.drag = None;
            }
            return;
        };
        let at = if self.prefs.spline.snap_points {
            (hit.tile.0 as f32 + 0.5, hit.tile.1 as f32 + 0.5)
        } else {
            hit.at
        };
        let modifiers = ctx.input(|i| i.modifiers);
        let nearest = self
            .interaction
            .spline
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| (i, distance(*p, at)))
            .filter(|(_, d)| *d <= GRAB_DISTANCE)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        if !self.keys_blocked(ctx) && ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            match nearest {
                Some(index) => {
                    self.interaction.spline.points.remove(index);
                }
                None => {
                    self.interaction.spline.points.pop();
                }
            }
        }
        if ctx.input(|i| i.pointer.primary_pressed()) && response.is_pointer_button_down_on() {
            if modifiers.alt {
                return;
            }
            match nearest {
                Some(index) if modifiers.command => {
                    self.interaction.spline.points.remove(index);
                }
                Some(index) => self.interaction.spline.drag = Some(index),
                None if modifiers.shift && self.interaction.spline.points.len() >= 2 => {
                    // Insert on the stretch of the control polygon nearest the click.
                    let points = &self.interaction.spline.points;
                    let mut best = (f32::MAX, 0);
                    for i in 0..points.len() - 1 {
                        let d = segment_distance(at, points[i], points[i + 1]);
                        if d < best.0 {
                            best = (d, i);
                        }
                    }
                    self.interaction.spline.points.insert(best.1 + 1, at);
                    self.interaction.spline.drag = Some(best.1 + 1);
                }
                None => {
                    self.interaction.spline.points.push(at);
                    self.interaction.spline.drag = Some(self.interaction.spline.points.len() - 1);
                }
            }
        }
        if ctx.input(|i| i.pointer.primary_down()) {
            if let Some(index) = self.interaction.spline.drag
                && let Some(point) = self.interaction.spline.points.get_mut(index)
            {
                *point = at;
            }
        } else {
            self.interaction.spline.drag = None;
        }
    }

    /// Lay the previewed fitted path as one semantic patch / one undo step.
    pub(in crate::app) fn commit_spline(&mut self) {
        if self.laying_overlay().is_none() {
            self.notice = Some((
                "Choose an overlay to lay the path with".to_owned(),
                web_time::Instant::now(),
            ));
            return;
        }
        self.spline_plan_now();
        let Some(preview) = self.interaction.spline.preview.take() else {
            return;
        };
        if preview.paints.is_empty() {
            self.interaction.spline.preview = Some(preview);
            self.notice = Some((
                "The fitted path has no overlay tiles".to_owned(),
                web_time::Instant::now(),
            ));
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
        self.interaction.spline.points.clear();
        self.notice = Some((
            format!("Laid a fitted path over {changed} tiles"),
            web_time::Instant::now(),
        ));
    }
}
