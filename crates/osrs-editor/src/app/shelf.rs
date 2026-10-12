//! The left shelf: how the active tool acts (shape, size, falloff, strength; path width and style).
//!
//! It exists only while a tool other than Select is active, so the viewport has its full width when
//! just looking and selecting. What a tool paints *with* (floors, shapes, flags) is in the drawer.

use super::EditorApp;
use super::widgets::chips;
use super::workspace::tool_icon;
use crate::autotile::{CurveKind, EdgeStyle};
use crate::heights::Falloff;
use crate::icons;
use crate::paint::{BrushShape, Tool};
use eframe::egui::{self, Stroke};

const SHELF_WIDTH: f32 = 196.0;

impl EditorApp {
    pub(super) fn tool_shelf(&mut self, ui: &mut egui::Ui) {
        let tool = self.prefs.tool;
        if !tool.has_shelf() {
            return;
        }
        let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
        egui::Panel::left("tool_shelf")
            .resizable(false)
            .exact_size(SHELF_WIDTH)
            .frame(
                egui::Frame::new()
                    .fill(ui.visuals().panel_fill)
                    .stroke(Stroke::new(1.0, line))
                    .inner_margin(egui::Margin::symmetric(12, 10)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(tool_icon(tool))
                            .size(18.0)
                            .color(ui.visuals().selection.stroke.color),
                    );
                    ui.label(egui::RichText::new(tool.label()).strong().size(15.0));
                });
                ui.add_space(2.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match tool {
                        Tool::Paint => self.brush_section(ui),
                        Tool::Height => {
                            self.brush_section(ui);
                            self.sculpt_section(ui);
                        }
                        Tool::Spline => self.path_section(ui),
                        Tool::Select | Tool::Objects => {}
                    });
            });
    }

    /// Footprint shape and size: shared by every brush tool.
    fn brush_section(&mut self, ui: &mut egui::Ui) {
        crate::theme::section_heading(ui, "Brush");
        ui.horizontal(|ui| {
            for (shape, icon, label) in [
                (BrushShape::Square, icons::BRUSH_SQUARE, "Square"),
                (BrushShape::Circle, icons::BRUSH_CIRCLE, "Round"),
            ] {
                if ui
                    .selectable_label(
                        self.prefs.brush.footprint == shape,
                        format!("{icon}  {label}"),
                    )
                    .clicked()
                {
                    self.prefs.brush.footprint = shape;
                }
            }
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Size").weak());
        let mut radius = i32::from(self.prefs.brush.radius);
        ui.spacing_mut().slider_width = ui.available_width() - 70.0;
        if ui
            .add(
                egui::Slider::new(&mut radius, 0..=16).custom_formatter(|r, _| {
                    let side = r as i32 * 2 + 1;
                    format!("{side}\u{d7}{side}")
                }),
            )
            .changed()
        {
            self.prefs.brush.radius = radius as u8;
        }
        ui.add_space(2.0);
        ui.weak("[ and ] resize");
    }

    /// Strength and falloff of the Height tool.
    fn sculpt_section(&mut self, ui: &mut egui::Ui) {
        crate::theme::section_heading(ui, "Strength");
        let tool = &mut self.prefs.height_tool;
        ui.spacing_mut().slider_width = ui.available_width() - 56.0;
        ui.add(egui::Slider::new(&mut tool.strength, 8..=128).step_by(8.0));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Falloff").weak());
        let options: Vec<_> = Falloff::ALL.iter().map(|f| (*f, f.label())).collect();
        if let Some(falloff) = chips(ui, tool.falloff, &options) {
            tool.falloff = falloff;
        }
    }

    /// The spline tool's path: width, curve, edge style, loop and ramp.
    fn path_section(&mut self, ui: &mut egui::Ui) {
        crate::theme::section_heading(ui, "Path");
        ui.label(egui::RichText::new("Width").weak());
        ui.spacing_mut().slider_width = ui.available_width() - 76.0;
        ui.add(
            egui::Slider::new(&mut self.prefs.spline.width, 1.0..=24.0)
                .max_decimals(1)
                .suffix(" tiles"),
        );
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Line").weak());
        if let Some(curve) = chips(
            ui,
            self.prefs.spline.curve,
            &[(CurveKind::Curve, "Curve"), (CurveKind::Lines, "Lines")],
        ) {
            self.prefs.spline.curve = curve;
        }
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Edges").weak());
        for style in EdgeStyle::ALL {
            if ui
                .selectable_label(self.prefs.spline.style == style, style.label())
                .on_hover_text(style.hint())
                .clicked()
            {
                self.prefs.spline.style = style;
            }
        }
        if self.prefs.spline.style == EdgeStyle::Runs {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Straighten").weak());
            ui.add(
                egui::Slider::new(&mut self.prefs.spline.straighten, 0.25..=0.8)
                    .max_decimals(2)
                    .suffix(" tiles"),
            )
            .on_hover_text(
                "How far an edge may stray from the drawn curve to stay one straight run. \
                 More: longer runs and fewer turns; less: closer to the curve, more turns",
            );
        }
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Clean angles").weak());
        ui.add(
            egui::Slider::new(&mut self.prefs.spline.snap, 0.0..=1.5)
                .max_decimals(2)
                .suffix(" tiles"),
        )
        .on_hover_text(
            "Nudge control points up to this far so each run takes a slope the shapes draw \
             perfectly straight (flat, 45 degrees, 2:1, upright). 0 keeps the points exactly",
        );
        crate::theme::section_heading(ui, "Shape");
        ui.checkbox(&mut self.prefs.spline.closed, "Close the loop")
            .on_hover_text("Join the last point back to the first");
        ui.checkbox(&mut self.prefs.spline.ramp, "Grade the terrain")
            .on_hover_text(
                "Slope the ground along the path, from the first point's height to the last's",
            );
    }
}
