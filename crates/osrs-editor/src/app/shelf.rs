//! The tool shelf (docked left or right, or floating over the viewport): how the active tool acts (shape, size, falloff, strength; path width and style).
//!
//! It exists only while a tool other than Select is active, so the viewport has its full width when
//! just looking and selecting. What a tool paints *with* (floors, shapes, flags) is in the drawer.

use super::EditorApp;
use super::widgets::chips;
use super::workspace::tool_icon;
use crate::heights::Falloff;
use crate::icons;
use crate::paint::{BrushShape, Tool};
use eframe::egui::{self, Stroke};

const SHELF_WIDTH: f32 = 196.0;

impl EditorApp {
    pub(super) fn tool_shelf(&mut self, ui: &mut egui::Ui) {
        let tool = self.settings.prefs.tool;
        if !tool.has_shelf() {
            return;
        }
        if self.settings.prefs.shelf_floating {
            self.floating_shelf(ui.ctx(), tool);
            return;
        }
        let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
        let panel = if self.settings.prefs.shelf_right {
            egui::Panel::right("tool_shelf")
        } else {
            egui::Panel::left("tool_shelf")
        };
        panel
            .resizable(false)
            .exact_size(SHELF_WIDTH)
            .frame(
                egui::Frame::new()
                    .fill(ui.visuals().panel_fill)
                    .stroke(Stroke::new(1.0, line))
                    .inner_margin(egui::Margin::symmetric(12, 10)),
            )
            .show(ui, |ui| self.shelf_contents(ui, tool));
    }

    /// The shelf as a window you can drag anywhere over the viewport (its position is kept).
    fn floating_shelf(&mut self, ctx: &egui::Context, tool: Tool) {
        let [x, y] = self.settings.prefs.shelf_pos;
        let response = egui::Window::new(tool.label())
            .id(egui::Id::new("shelf_window"))
            .title_bar(false)
            .default_pos(egui::pos2(x, y))
            .default_width(SHELF_WIDTH)
            .min_width(SHELF_WIDTH)
            .max_height(560.0)
            .resizable([false, true])
            .constrain_to(ctx.content_rect())
            .show(ctx, |ui| self.shelf_contents(ui, tool));
        if let Some(response) = response {
            let min = response.response.rect.min;
            self.settings.prefs.shelf_pos = [min.x, min.y];
        }
    }

    /// Header (tool, pop-out and side buttons) and the tool's sections.
    fn shelf_contents(&mut self, ui: &mut egui::Ui, tool: Tool) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(tool_icon(tool))
                    .size(18.0)
                    .color(ui.visuals().selection.stroke.color),
            );
            ui.label(egui::RichText::new(tool.label()).strong().size(15.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (icon, tip) = if self.settings.prefs.shelf_floating {
                    (icons::DOCK_IN, "Dock the shelf back to the edge")
                } else {
                    (
                        icons::POP_OUT,
                        "Undock the shelf into a window you can move",
                    )
                };
                if ui
                    .add(egui::Button::new(icon).small().frame(false))
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.settings.prefs.shelf_floating = !self.settings.prefs.shelf_floating;
                }
                if !self.settings.prefs.shelf_floating
                    && ui
                        .add(egui::Button::new(icons::SWAP_SIDE).small().frame(false))
                        .on_hover_text("Move the shelf to the other side")
                        .clicked()
                {
                    self.settings.prefs.shelf_right = !self.settings.prefs.shelf_right;
                }
            });
        });
        ui.add_space(2.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false, self.settings.prefs.shelf_floating])
            .show(ui, |ui| match tool {
                Tool::Paint => self.brush_section(ui),
                Tool::Height => {
                    self.brush_section(ui);
                    self.sculpt_section(ui);
                }
                Tool::Spline => self.path_section(ui),
                Tool::Select | Tool::Objects => {}
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
                        self.settings.prefs.brush.footprint == shape,
                        format!("{icon}  {label}"),
                    )
                    .clicked()
                {
                    self.settings.prefs.brush.footprint = shape;
                }
            }
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Size").weak());
        let mut radius = i32::from(self.settings.prefs.brush.radius);
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
            self.settings.prefs.brush.radius = radius as u8;
        }
        ui.add_space(2.0);
        ui.weak("[ and ] resize");
    }

    /// Strength and falloff of the Height tool.
    fn sculpt_section(&mut self, ui: &mut egui::Ui) {
        crate::theme::section_heading(ui, "Strength");
        let tool = &mut self.settings.prefs.height_tool;
        ui.spacing_mut().slider_width = ui.available_width() - 56.0;
        ui.add(egui::Slider::new(&mut tool.strength, 8..=128).step_by(8.0));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Falloff").weak());
        let options: Vec<_> = Falloff::ALL.iter().map(|f| (*f, f.label())).collect();
        if let Some(falloff) = chips(ui, tool.falloff, &options) {
            tool.falloff = falloff;
        }
    }

    /// Deterministic width/shape fitting controls used by the Spline tool.
    fn path_section(&mut self, ui: &mut egui::Ui) {
        crate::theme::section_heading(ui, "Path");
        let spline = &mut self.settings.prefs.spline;
        ui.checkbox(&mut spline.snap_points, "Snap points to tile centres")
            .on_hover_text(
                "Place control points at tile centres instead of the exact cursor position",
            );

        ui.add_space(4.0);
        ui.label(egui::RichText::new("Width").weak());
        ui.add(
            egui::Slider::new(&mut spline.width, 0.5..=24.0)
                .max_decimals(1)
                .suffix(" tiles"),
        );
        ui.label(egui::RichText::new("Edge continuity").weak());
        ui.add(egui::Slider::new(&mut spline.edge_weight, 0.0..=2.0).max_decimals(2))
            .on_hover_text("0 fits every tile independently; higher values prefer neighbouring shapes whose shared edges agree");
        ui.add_space(6.0);
        ui.weak(
            "The path follows the line you draw. Obstacle routing and walls are disabled for now.",
        );
    }
}
