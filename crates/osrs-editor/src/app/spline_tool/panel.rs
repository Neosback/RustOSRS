//! Shared Spline widgets. Drawing emits point actions; the interaction owner applies them.
use crate::paint::{PathStyle, SplineSettings};
use crate::terrain_path::ControlPoint;
use eframe::egui;

/// The path's style and ground option. Width is a preset (Trail, Path, Road); with a point
/// selected the preset applies to that point, otherwise to every point and to new ones. Everything
/// else is decided by how the real map is built.
pub(in crate::app) fn settings(ui: &mut egui::Ui, settings: &mut SplineSettings) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Width");
        for style in PathStyle::ALL {
            let selected = settings.width == style.width();
            if ui
                .selectable_label(selected, style.label())
                .on_hover_text(format!(
                    "{} tile{} wide. Applies to the selected point, or to all points and new ones.",
                    style.width(),
                    if style.width() == 1.0 { "" } else { "s" }
                ))
                .clicked()
            {
                settings.width = style.width();
            }
        }
    });
    ui.checkbox(&mut settings.smooth_ground, "Smooth ground along the path")
        .on_hover_text(
            "Ease bumps and too-steep stretches along the path. Off: the path follows the ground, as the real map's paths do.",
        );
}

#[derive(Debug, PartialEq)]
pub(in crate::app) enum PointAction {
    Select(usize),
    Move(usize, (f32, f32)),
    Remove(usize),
    Width(usize, f32),
    ToggleCorner(usize),
}

pub(in crate::app) fn points(
    ui: &mut egui::Ui,
    points: &[ControlPoint],
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
                .max_height(160.0)
                .show(ui, |ui| {
                    for (index, point) in points.iter().copied().enumerate() {
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .selectable_label(selected == Some(index), format!("{}", index + 1))
                                .clicked()
                            {
                                action = Some(PointAction::Select(index));
                            }
                            let mut position = (point.x, point.y);
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
                            let mut width = point.width;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut width)
                                        .range(1.0..=24.0)
                                        .speed(0.1)
                                        .prefix("W "),
                                )
                                .on_hover_text("Width at this point, in tiles")
                                .changed()
                            {
                                action = Some(PointAction::Width(index, width));
                            }
                            if ui
                                .selectable_label(point.corner, egui_phosphor::regular::SQUARE)
                                .on_hover_text(
                                    "Corner: the path bends sharply here (double-click the point)",
                                )
                                .clicked()
                            {
                                action = Some(PointAction::ToggleCorner(index));
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
                let ctx = crate::theme::layout_test_context(scale);
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 600.0),
                    )),
                    ..Default::default()
                };
                let mut output = ctx.run_ui(input, |ui| {
                    let right = ui.max_rect().right();
                    for smooth_ground in [false, true] {
                        let mut value = SplineSettings {
                            smooth_ground,
                            ..SplineSettings::default()
                        };
                        let response = ui.scope(|ui| settings(ui, &mut value));
                        assert!(
                            response.response.rect.right() <= right + 0.5,
                            "width {width}, scale {scale}, smooth {smooth_ground}: {:?}",
                            response.response.rect
                        );
                    }
                });
                output.textures_delta.clear();
            }
        }
    }
}
