//! Spline tool pointer input and commit.

use super::*;
use crate::app::interaction_runtime::InteractionRuntime;

impl InteractionRuntime {
    pub(super) fn spline_points(&self) -> &[(f32, f32)] {
        &self.spline.points
    }

    pub(super) fn begin_spline_drag(&mut self, index: usize) {
        if index < self.spline.points.len() {
            self.spline.drag = Some(index);
        }
    }

    pub(super) fn append_spline_point_and_drag(&mut self, point: (f32, f32)) {
        self.spline.points.push(point);
        self.spline.drag = Some(self.spline.points.len() - 1);
    }

    pub(super) fn insert_spline_point_and_drag(&mut self, index: usize, point: (f32, f32)) {
        let index = index.min(self.spline.points.len());
        self.spline.points.insert(index, point);
        self.spline.drag = Some(index);
    }

    pub(super) fn update_spline_drag(&mut self, point: (f32, f32)) {
        if let Some(index) = self.spline.drag
            && let Some(current) = self.spline.points.get_mut(index)
        {
            *current = point;
        }
    }

    pub(super) fn finish_spline_drag(&mut self) {
        self.spline.drag = None;
    }

    pub(super) fn cancel_spline_drag(&mut self) {
        self.spline.drag = None;
    }

    pub(super) fn remove_spline_point(&mut self, index: usize) {
        if index < self.spline.points.len() {
            self.spline.points.remove(index);
        }
    }

    pub(super) fn pop_spline_point(&mut self) {
        self.spline.points.pop();
    }

    pub(super) fn clear_spline_points(&mut self) {
        self.spline.points.clear();
    }
}

impl EditorApp {
    /// Add, move, insert and remove control points; Enter lays the path.
    pub(in crate::app) fn spline_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        self.interaction
            .spline
            .ctx
            .get_or_insert_with(|| ctx.clone());
        if self.prefs.tool != Tool::Spline {
            self.interaction.cancel_spline_drag();
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
                self.interaction.pop_spline_point();
            }
        }
        let Some(hit) = self.interaction.hover.tile else {
            if !ctx.input(|i| i.pointer.primary_down()) {
                self.interaction.finish_spline_drag();
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
        self.interaction.clear_spline_points();
        self.notice = Some((
            format!("Laid a fitted path over {changed} tiles"),
            web_time::Instant::now(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::FlyCamera;

    fn interaction() -> InteractionRuntime {
        InteractionRuntime::new(FlyCamera::new(0.0, 0.0, 0.0))
    }

    #[test]
    fn spline_point_lifecycle_appends_inserts_and_updates_only_active_point() {
        let mut interaction = interaction();
        interaction.append_spline_point_and_drag((1.0, 2.0));
        assert_eq!(interaction.spline_points(), &[(1.0, 2.0)]);
        assert_eq!(interaction.spline.drag, Some(0));

        interaction.finish_spline_drag();
        interaction.append_spline_point_and_drag((5.0, 6.0));
        interaction.finish_spline_drag();
        interaction.insert_spline_point_and_drag(1, (3.0, 4.0));
        assert_eq!(
            interaction.spline_points(),
            &[(1.0, 2.0), (3.0, 4.0), (5.0, 6.0)]
        );
        assert_eq!(interaction.spline.drag, Some(1));

        interaction.update_spline_drag((3.5, 4.5));
        assert_eq!(
            interaction.spline_points(),
            &[(1.0, 2.0), (3.5, 4.5), (5.0, 6.0)]
        );
    }

    #[test]
    fn spline_drag_finish_and_cancel_clear_capture_without_changing_points() {
        let mut interaction = interaction();
        interaction.append_spline_point_and_drag((1.0, 2.0));
        interaction.finish_spline_drag();
        assert_eq!(interaction.spline.drag, None);
        assert_eq!(interaction.spline_points(), &[(1.0, 2.0)]);

        interaction.begin_spline_drag(0);
        interaction.cancel_spline_drag();
        interaction.update_spline_drag((9.0, 9.0));
        assert_eq!(interaction.spline.drag, None);
        assert_eq!(interaction.spline_points(), &[(1.0, 2.0)]);
    }

    #[test]
    fn spline_remove_pop_and_clear_mutate_points_through_runtime() {
        let mut interaction = interaction();
        interaction.append_spline_point_and_drag((1.0, 1.0));
        interaction.finish_spline_drag();
        interaction.append_spline_point_and_drag((2.0, 2.0));
        interaction.finish_spline_drag();
        interaction.append_spline_point_and_drag((3.0, 3.0));
        interaction.finish_spline_drag();

        interaction.remove_spline_point(1);
        assert_eq!(interaction.spline_points(), &[(1.0, 1.0), (3.0, 3.0)]);
        interaction.pop_spline_point();
        assert_eq!(interaction.spline_points(), &[(1.0, 1.0)]);
        interaction.clear_spline_points();
        assert!(interaction.spline_points().is_empty());
    }
}
