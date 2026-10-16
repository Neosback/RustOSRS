//! Shared Spline widgets: the path tool's settings, the same as the EikenB00m editor's. Drawing
//! edits the settings; the owner applies them.
use crate::paint::SplineSettings;
use eframe::egui;

/// A labelled slider as wide as the panel allows.
fn slider<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
    step: f64,
    suffix: &str,
    hint: &str,
) {
    ui.label(label).on_hover_text(hint);
    ui.scope(|ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 100.0).clamp(40.0, 260.0);
        ui.add(egui::Slider::new(value, range).step_by(step).suffix(suffix))
            .on_hover_text(hint);
    });
}

/// The path's shape and terrain options. Terrain sliders are available once "Smooth terrain
/// height" is on.
pub(in crate::app) fn settings(ui: &mut egui::Ui, settings: &mut SplineSettings) {
    slider(
        ui,
        "Width",
        &mut settings.width,
        SplineSettings::WIDTH,
        0.5,
        " tiles",
        "How wide the path is. 1 is a single tile.",
    );
    ui.checkbox(&mut settings.smooth_curve, "Smooth curve")
        .on_hover_text(
            "On: the path flows as a smooth curve through your points. Off: straight lines between points.",
        );
    slider(
        ui,
        "Edge smoothing",
        &mut settings.edge_smoothing,
        SplineSettings::EDGE,
        0.5,
        "",
        "Makes the shapes of neighbouring tiles line up on their edges. Higher gives a cleaner outline, 0 fits every tile on its own.",
    );
    ui.checkbox(&mut settings.smooth_height, "Smooth terrain height")
        .on_hover_text(
            "Evens out the height under the path so it has no steep steps. Applied together with the path and undone separately (Ctrl+Z twice).",
        );
    ui.add_enabled_ui(settings.smooth_height, |ui| {
        slider(
            ui,
            "Height smoothing",
            &mut settings.height_passes,
            SplineSettings::PASSES,
            1.0,
            " passes",
            "Number of smoothing passes. Higher values flatten the terrain more along the path; start low and increase if it is still too steep.",
        );
        slider(
            ui,
            "Blend distance",
            &mut settings.height_blend,
            SplineSettings::BLEND,
            1.0,
            " tiles",
            "How many tiles around the path are blended into the surrounding terrain. Use a larger distance on steep hillsides so the slope next to the path becomes gradual instead of a sudden wall.",
        );
    });
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
                    for smooth_height in [false, true] {
                        let mut value = SplineSettings {
                            smooth_height,
                            ..SplineSettings::default()
                        };
                        let response = ui.scope(|ui| settings(ui, &mut value));
                        assert!(
                            response.response.rect.right() <= right + 0.5,
                            "width {width}, scale {scale}, height {smooth_height}: {:?}",
                            response.response.rect
                        );
                    }
                });
                output.textures_delta.clear();
            }
        }
    }
}
