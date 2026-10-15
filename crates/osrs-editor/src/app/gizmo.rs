//! The transform gizmo of the Objects tool (`transform-gizmo-egui`): arrows to move the selection
//! along the map's axes or across the ground, a ring to turn it, snapped to whole tiles and quarter
//! turns. A drag shows the footprints it would land on and commits as one undo step on release.

use super::EditorApp;
use crate::camera::FlyCamera;
use crate::paint::Tool;
use eframe::egui::{self, Rect};
use osrs_world::LocSpec;
use transform_gizmo_egui::math::{DQuat, DVec3, Transform};
use transform_gizmo_egui::{
    GizmoConfig, GizmoInteraction, GizmoMode, GizmoOrientation, GizmoResult,
};

/// A gizmo drag in progress.
pub struct GizmoDrag {
    start: Transform,
    current: Transform,
    specs: Vec<LocSpec>,
}

/// Near and far planes of the gizmo's projection, in world units.
const NEAR: f64 = 8.0;
const FAR: f64 = 400_000.0;

fn row_matrix(rows: [[f64; 4]; 4]) -> mint::RowMatrix4<f64> {
    mint::RowMatrix4 {
        x: rows[0].into(),
        y: rows[1].into(),
        z: rows[2].into(),
        w: rows[3].into(),
    }
}

/// The view and projection the viewport draws with, as the gizmo wants them: right-handed with
/// the camera looking down -Z and Y up in view space (the game's own view has Y down and Z
/// forward, a half turn about X away). World space is the game's, minus the render origin.
fn gizmo_matrices(
    camera: &FlyCamera,
    viewport: Rect,
) -> (mint::RowMatrix4<f64>, mint::RowMatrix4<f64>) {
    let (sp, cp) = f64::from(camera.pitch).sin_cos();
    let (sy, cy) = f64::from(camera.yaw).sin_cos();
    // Rows of Rx * Ry (see `pick::project`), then Y and Z negated.
    let r = [
        [cy, 0.0, sy],
        [-sp * sy, -cp, sp * cy],
        [cp * sy, -sp, -cp * cy],
    ];
    let eye = [
        f64::from(camera.x),
        f64::from(camera.y),
        f64::from(camera.z),
    ];
    let t = |row: [f64; 3]| -(row[0] * eye[0] + row[1] * eye[1] + row[2] * eye[2]);
    let view = row_matrix([
        [r[0][0], r[0][1], r[0][2], t(r[0])],
        [r[1][0], r[1][1], r[1][2], t(r[1])],
        [r[2][0], r[2][1], r[2][2], t(r[2])],
        [0.0, 0.0, 0.0, 1.0],
    ]);
    let scale = f64::from(camera.reference(viewport.width()).scale);
    let (w, h) = (f64::from(viewport.width()), f64::from(viewport.height()));
    let a = -(FAR + NEAR) / (FAR - NEAR);
    let b = -2.0 * FAR * NEAR / (FAR - NEAR);
    let projection = row_matrix([
        [2.0 * scale / w, 0.0, 0.0, 0.0],
        [0.0, 2.0 * scale / h, 0.0, 0.0],
        [0.0, 0.0, a, b],
        [0.0, 0.0, -1.0, 0.0],
    ]);
    (view, projection)
}

impl EditorApp {
    /// Where the gizmo sits: the middle of the selected placements, on the ground, in render space.
    fn gizmo_origin(&self, specs: &[LocSpec]) -> Option<DVec3> {
        if specs.is_empty() {
            return None;
        }
        let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
        for spec in specs {
            let corners = crate::interact::tile_corners(&self.infos, spec.tile, spec.plane)?;
            x += f64::from(spec.tile.0) * 128.0 + 64.0;
            z += f64::from(spec.tile.1) * 128.0 + 64.0;
            y += corners.iter().map(|c| c[1]).sum::<f64>() / 4.0;
        }
        let n = specs.len() as f64;
        let (ox, oz) = self.renderer.render_origin();
        Some(DVec3::new(
            x / n - f64::from(ox),
            y / n,
            z / n - f64::from(oz),
        ))
    }

    /// Draw the gizmo over the viewport and act on it. Called once per frame inside the central
    /// panel, after the viewport image.
    pub(super) fn object_gizmo(
        &mut self,
        ui: &mut egui::Ui,
        viewport: Rect,
        response: &egui::Response,
    ) {
        let specs = self.selected_specs();
        if self.prefs.tool != Tool::Objects
            || !self.prefs.object_gizmo
            || specs.is_empty()
            || self.context_menu.is_some()
            || self.modal_open()
        {
            self.interaction.gizmo_drag = None;
            self.interaction.gizmo_busy = false;
            return;
        }
        let Some(origin) = self.gizmo_origin(&specs) else {
            self.interaction.gizmo_drag = None;
            self.interaction.gizmo_busy = false;
            return;
        };
        let (view_matrix, projection_matrix) = gizmo_matrices(&self.interaction.camera, viewport);
        self.interaction.gizmo.update_config(GizmoConfig {
            view_matrix,
            projection_matrix,
            viewport,
            modes: GizmoMode::TranslateX
                | GizmoMode::TranslateZ
                | GizmoMode::TranslateXZ
                | GizmoMode::RotateY,
            orientation: GizmoOrientation::Global,
            snapping: true,
            snap_distance: 128.0,
            snap_angle: std::f32::consts::FRAC_PI_2,
            ..*self.interaction.gizmo.config()
        });
        let target = self.interaction.gizmo_drag.as_ref().map_or_else(
            || Transform::from_scale_rotation_translation(DVec3::ONE, DQuat::IDENTITY, origin),
            |drag| drag.current,
        );
        let result = self.interact_gizmo(ui, viewport, response, &[target]);
        if let Some((_, transforms)) = result
            && let Some(moved) = transforms.first()
        {
            let (drag_specs, delta) = {
                let drag = self
                    .interaction
                    .gizmo_drag
                    .get_or_insert_with(|| GizmoDrag {
                        start: target,
                        current: target,
                        specs: specs.clone(),
                    });
                drag.current = *moved;
                // Whole tiles moved so far, for the footprints.
                let from = DVec3::from(drag.start.translation);
                let to = DVec3::from(drag.current.translation);
                let delta = (
                    ((to.x - from.x) / 128.0).round() as i32,
                    ((to.z - from.z) / 128.0).round() as i32,
                );
                (drag.specs.clone(), delta)
            };
            self.interaction.begin_object_drag((0, 0), drag_specs);
            self.interaction.update_object_drag(delta);
        }
        self.interaction.gizmo_busy =
            self.interaction.gizmo.is_focused() || self.interaction.gizmo_drag.is_some();
        if self.interaction.gizmo_drag.is_some() && !ui.input(|i| i.pointer.primary_down()) {
            self.finish_gizmo_drag();
        }
    }

    /// What `GizmoExt::interact` does, minus the widget it lays over the cursor: that widget would
    /// take the pointer from the viewport, and with it the camera's drags and scrolling.
    fn interact_gizmo(
        &mut self,
        ui: &egui::Ui,
        viewport: Rect,
        response: &egui::Response,
        targets: &[Transform],
    ) -> Option<(GizmoResult, Vec<Transform>)> {
        let ctx = ui.ctx();
        self.interaction.gizmo.update_config(GizmoConfig {
            viewport,
            pixels_per_point: ctx.pixels_per_point(),
            ..*self.interaction.gizmo.config()
        });
        let cursor = ctx.input(|i| i.pointer.hover_pos()).unwrap_or_default();
        // Only the primary button works the gizmo; the camera's buttons pass through it.
        let (pressed, down) = ctx.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Primary),
                i.pointer.button_down(egui::PointerButton::Primary),
            )
        });
        let result = self.interaction.gizmo.update(
            GizmoInteraction {
                cursor_pos: (cursor.x, cursor.y),
                hovered: response.hovered() || response.dragged(),
                drag_started: pressed,
                dragging: down,
            },
            targets,
        );
        let draw = self.interaction.gizmo.draw();
        egui::Painter::new(ctx.clone(), ui.layer_id(), viewport).add(egui::Mesh {
            indices: draw.indices,
            vertices: draw
                .vertices
                .into_iter()
                .zip(draw.colors)
                .map(|(pos, [r, g, b, a])| egui::epaint::Vertex {
                    pos: pos.into(),
                    uv: egui::Pos2::default(),
                    color: egui::Rgba::from_rgba_premultiplied(r, g, b, a).into(),
                })
                .collect(),
            ..Default::default()
        });
        result
    }

    /// The button went up: commit the move, or the turn, as one undo step.
    fn finish_gizmo_drag(&mut self) {
        self.interaction.cancel_object_drag();
        self.interaction.gizmo_busy = false;
        let Some(drag) = self.interaction.gizmo_drag.take() else {
            return;
        };
        let from = DVec3::from(drag.start.translation);
        let to = DVec3::from(drag.current.translation);
        let delta = (
            ((to.x - from.x) / 128.0).round() as i32,
            ((to.z - from.z) / 128.0).round() as i32,
        );
        let q = DQuat::from(drag.current.rotation);
        let angle = 2.0 * q.y.atan2(q.w);
        let quarters = (angle / std::f64::consts::FRAC_PI_2).round() as i32;
        if delta != (0, 0) {
            self.move_objects(drag.specs, delta);
        } else if quarters.rem_euclid(4) != 0 {
            self.interaction.selection_objects.clear();
            self.rotate_selected_objects(quarters.rem_euclid(4) as i8);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use transform_gizmo_egui::math::{DMat4, DVec4};

    /// The gizmo's matrices put a world point where the viewport's own projection does.
    #[test]
    fn gizmo_matrices_agree_with_the_viewport_projection() {
        let mut camera = FlyCamera::new(500.0, -900.0, -300.0);
        camera.yaw = 0.7;
        camera.pitch = 0.9;
        let viewport = Rect::from_min_size(egui::pos2(40.0, 60.0), egui::vec2(1000.0, 600.0));
        let (view, projection) = gizmo_matrices(&camera, viewport);
        let to_mat = |m: mint::RowMatrix4<f64>| DMat4::from(m);
        let clip = to_mat(projection) * to_mat(view) * DVec4::new(900.0, -200.0, 400.0, 1.0);
        let (ndc_x, ndc_y) = (clip.x / clip.w, clip.y / clip.w);
        let screen = (
            viewport.center().x + (ndc_x as f32) * viewport.width() / 2.0,
            viewport.center().y - (ndc_y as f32) * viewport.height() / 2.0,
        );
        let projected = crate::pick::project(
            &camera,
            (0, 0),
            viewport.width(),
            viewport.height(),
            [900.0, -200.0, 400.0],
        )
        .unwrap_or([f32::NAN; 2]);
        assert!((screen.0 - (viewport.min.x + projected[0])).abs() < 0.05);
        assert!((screen.1 - (viewport.min.y + projected[1])).abs() < 0.05);
    }
}
