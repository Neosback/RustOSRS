//! Shared Spline widgets. Drawing emits point actions; the interaction owner applies them.
use crate::paint::{PathCurve, SplineSettings};
use eframe::egui;

/// One labelled control that stays together when the row wraps.
fn pair(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.label(label);
        add(ui);
    });
}

/// Panel width below which each control gets its own line. A control that is itself a row (a
/// label and its field) does not wrap inside a wrapping flow, so narrow panels stack them.
const STACK_BELOW: f32 = 300.0;
/// The same for the three grading fields.
const STACK_BELOW_GRADE: f32 = 520.0;

fn width_field(ui: &mut egui::Ui, settings: &mut SplineSettings) {
    pair(ui, "Width", |ui| {
        ui.add(
            egui::DragValue::new(&mut settings.width)
                .range(0.5..=24.0)
                .speed(0.1)
                .suffix(" tiles"),
        );
    });
}

fn edge_field(ui: &mut egui::Ui, settings: &mut SplineSettings) {
    pair(ui, "Edge", |ui| {
        ui.add(
            egui::DragValue::new(&mut settings.edge_weight)
                .range(0.0..=2.0)
                .speed(0.05),
        )
        .on_hover_text("How strongly neighbouring tiles' shapes are made to line up");
    });
}

/// The path's shape and terrain options. Every control fits any panel width: narrow panels stack
/// them, wider ones flow them into rows.
pub(in crate::app) fn settings(ui: &mut egui::Ui, settings: &mut SplineSettings) {
    if ui.available_width() < STACK_BELOW {
        width_field(ui, settings);
        edge_field(ui, settings);
    } else {
        ui.horizontal_wrapped(|ui| {
            width_field(ui, settings);
            edge_field(ui, settings);
        });
    }
    let mut straight = settings.curve == PathCurve::Straight;
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut straight, "Straight lines")
            .on_hover_text("Straight segments between points instead of a smooth curve");
        ui.checkbox(&mut settings.snap_points, "Snap to tiles");
        ui.checkbox(&mut settings.follow_cursor, "Preview to cursor")
            .on_hover_text("Show the next segment up to the cursor while placing points");
    });
    settings.curve = if straight {
        PathCurve::Straight
    } else {
        PathCurve::Smooth
    };
    ui.checkbox(&mut settings.grade, "Grade terrain")
        .on_hover_text(
            "Level the ground across the path and follow it along the path; laid in the same undo step",
        );
    if settings.grade {
        // Three fields side by side need more room than two.
        if ui.available_width() < STACK_BELOW_GRADE {
            grade_smooth(ui, settings);
            grade_slope(ui, settings);
            grade_blend(ui, settings);
        } else {
            ui.horizontal_wrapped(|ui| {
                grade_smooth(ui, settings);
                grade_slope(ui, settings);
                grade_blend(ui, settings);
            });
        }
    }
}

fn grade_smooth(ui: &mut egui::Ui, settings: &mut SplineSettings) {
    pair(ui, "Smooth", |ui| {
        ui.add(
            egui::DragValue::new(&mut settings.grade_smoothing)
                .range(0.0..=osrs_edit::MAX_SMOOTHING_TILES)
                .speed(0.25)
                .suffix(" tiles"),
        )
        .on_hover_text("Length over which the road bed is smoothed along the path");
    });
}

fn grade_slope(ui: &mut egui::Ui, settings: &mut SplineSettings) {
    pair(ui, "Max slope", |ui| {
        ui.add(
            egui::DragValue::new(&mut settings.grade_max_slope)
                .range(osrs_edit::MIN_SLOPE..=osrs_edit::MAX_SLOPE)
                .speed(0.5),
        )
        .on_hover_text("Steepest road bed change per tile, in client units (128 = a tile)");
    });
}

fn grade_blend(ui: &mut egui::Ui, settings: &mut SplineSettings) {
    pair(ui, "Blend", |ui| {
        ui.add(
            egui::DragValue::new(&mut settings.grade_blend)
                .range(0.0..=osrs_edit::MAX_BLEND_TILES)
                .speed(0.25)
                .suffix(" tiles"),
        )
        .on_hover_text("Distance beyond the path edge over which the bed fades into the terrain");
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
                    for grade in [false, true] {
                        let mut settings_value = SplineSettings {
                            grade,
                            ..SplineSettings::default()
                        };
                        let response = ui.scope(|ui| settings(ui, &mut settings_value));
                        assert!(
                            response.response.rect.right() <= right + 0.5,
                            "width {width}, scale {scale}, grade {grade}: {:?}",
                            response.response.rect
                        );
                    }
                });
                output.textures_delta.clear();
            }
        }
    }
}
