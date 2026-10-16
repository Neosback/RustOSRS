//! Shared Spline widgets. Drawing emits point actions; the interaction owner applies them.
use crate::paint::SplineSettings;
use eframe::egui;

pub(in crate::app) fn settings(ui: &mut egui::Ui, settings: &mut SplineSettings) {
    if ui.available_width() < 280.0 {
        ui.horizontal(|ui| {
            ui.label("Width");
            ui.add(
                egui::DragValue::new(&mut settings.width)
                    .range(0.5..=24.0)
                    .speed(0.1)
                    .suffix(" tiles"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Edge");
            ui.add(
                egui::DragValue::new(&mut settings.edge_weight)
                    .range(0.0..=2.0)
                    .speed(0.05),
            );
        });
        ui.checkbox(&mut settings.snap_points, "Snap to tiles");
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.horizontal(|ui| {
            ui.label("Width");
            ui.add(
                egui::DragValue::new(&mut settings.width)
                    .range(0.5..=24.0)
                    .speed(0.1)
                    .suffix(" tiles"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Edge");
            ui.add(
                egui::DragValue::new(&mut settings.edge_weight)
                    .range(0.0..=2.0)
                    .speed(0.05),
            );
        });
        ui.checkbox(&mut settings.snap_points, "Snap to tiles");
    });
}

#[derive(Debug, PartialEq)]
pub(in crate::app) enum PointAction {
    Select(usize),
    Move(usize, (f32, f32)),
    Remove(usize),
}

pub(in crate::app) fn points(
    ui: &mut egui::Ui,
    points: &[(f32, f32)],
    selected: Option<usize>,
) -> Option<PointAction> {
    let mut action = None;
    egui::CollapsingHeader::new(format!("Control points ({})", points.len()))
        .id_salt("spline_control_points")
        .show(ui, |ui| {
            if points.is_empty() {
                ui.weak("Click the terrain to add the first point.");
            }
            egui::ScrollArea::vertical()
                .id_salt("spline_points_scroll")
                .max_height(140.0)
                .show(ui, |ui| {
                    for (index, point) in points.iter().copied().enumerate() {
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .selectable_label(selected == Some(index), format!("{}", index + 1))
                                .clicked()
                            {
                                action = Some(PointAction::Select(index));
                            }
                            let mut position = point;
                            let x = ui.add(
                                egui::DragValue::new(&mut position.0)
                                    .speed(0.1)
                                    .prefix("X "),
                            );
                            let z = ui.add(
                                egui::DragValue::new(&mut position.1)
                                    .speed(0.1)
                                    .prefix("Z "),
                            );
                            if x.changed() || z.changed() {
                                action = Some(PointAction::Move(index, position));
                            }
                            if ui
                                .small_button(egui_phosphor::regular::TRASH)
                                .on_hover_text("Remove this control point")
                                .clicked()
                            {
                                action = Some(PointAction::Remove(index));
                            }
                        });
                    }
                });
        });
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_controls_fit_narrow_panels_at_multiple_scales() {
        for width in [172.0, 240.0, 320.0, 480.0] {
            for scale in [1.0, 2.0] {
                let ctx = egui::Context::default();
                ctx.set_pixels_per_point(scale);
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 600.0),
                    )),
                    ..Default::default()
                };
                let mut output = ctx.run_ui(input, |ui| {
                    let right = ui.max_rect().right();
                    let response = ui.scope(|ui| settings(ui, &mut SplineSettings::default()));
                    assert!(
                        response.response.rect.right() <= right + 0.5,
                        "width {width}, scale {scale}: {:?}",
                        response.response.rect
                    );
                });
                output.textures_delta.clear();
            }
        }
    }
}
