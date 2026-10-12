//! The tool shelf (docked left or right, or floating over the viewport): how the active tool acts (shape, size, falloff, strength; path width and style).
//!
//! It exists only while a tool other than Select is active, so the viewport has its full width when
//! just looking and selecting. What a tool paints *with* (floors, shapes, flags) is in the drawer.

use super::EditorApp;
use super::widgets::chips;
use super::workspace::tool_icon;
use crate::autotile::EdgeStyle;
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
        if self.prefs.shelf_floating {
            self.floating_shelf(ui.ctx(), tool);
            return;
        }
        let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
        let panel = if self.prefs.shelf_right {
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
        let [x, y] = self.prefs.shelf_pos;
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
            self.prefs.shelf_pos = [min.x, min.y];
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
                let (icon, tip) = if self.prefs.shelf_floating {
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
                    self.prefs.shelf_floating = !self.prefs.shelf_floating;
                }
                if !self.prefs.shelf_floating
                    && ui
                        .add(egui::Button::new(icons::SWAP_SIDE).small().frame(false))
                        .on_hover_text("Move the shelf to the other side")
                        .clicked()
                {
                    self.prefs.shelf_right = !self.prefs.shelf_right;
                }
            });
        });
        ui.add_space(2.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false, self.prefs.shelf_floating])
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

    /// The spline tool's path: width, edge style, wall kit.
    fn path_section(&mut self, ui: &mut egui::Ui) {
        crate::theme::section_heading(ui, "Path");
        ui.label(egui::RichText::new("Width").weak());
        ui.spacing_mut().slider_width = ui.available_width() - 76.0;
        ui.add(
            egui::Slider::new(&mut self.prefs.spline.width, 1.0..=12.0)
                .step_by(1.0)
                .max_decimals(0)
                .suffix(" tiles"),
        );
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
        if self.prefs.spline.style == EdgeStyle::Walls {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Wall kit").weak());
            let spline = &mut self.prefs.spline;
            ui.horizontal(|ui| {
                ui.label("Side");
                ui.add(egui::DragValue::new(&mut spline.wall_side).speed(1));
            });
            ui.horizontal(|ui| {
                ui.label("Top");
                ui.add(egui::DragValue::new(&mut spline.wall_top).speed(1));
            });
            ui.horizontal(|ui| {
                ui.label("Door");
                ui.add(egui::DragValue::new(&mut spline.wall_door).speed(1));
            });
            ui.horizontal(|ui| {
                ui.label("Diagonal");
                ui.add(egui::DragValue::new(&mut spline.wall_diagonal).speed(1));
            });
            ui.label(egui::RichText::new("Thickness").weak());
            ui.add(
                egui::Slider::new(&mut spline.wall_thickness, 1.0..=12.0)
                    .max_decimals(0)
                    .suffix(" tiles"),
            );
            ui.label(egui::RichText::new("Height").weak());
            ui.add(
                egui::Slider::new(&mut spline.wall_height, 1.0..=3.0)
                    .max_decimals(0)
                    .suffix(" planes"),
            );
            ui.checkbox(&mut spline.wall_towers, "Towers at control points");
            if spline.wall_towers {
                ui.add(
                    egui::Slider::new(&mut spline.tower_size, 1.0..=4.0)
                        .max_decimals(0)
                        .suffix(" tiles"),
                );
            }
        }
    }
}
