//! The floating toolbar on the viewport's left edge: the current tool, and how to change it.
//!
//! The editor rests in Select. A tool is one click away (or a key, the quick HUD, the context menu)
//! and one click or `Esc` back. In Select the toolbar also shows what a click selects (objects,
//! tiles, or both); with another tool it reminds that `Esc` returns to Select.

use super::EditorApp;
use super::workspace::{select_mode_icon, tool_icon};
use crate::paint::{SelectMode, Tool};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

const BUTTON: f32 = 34.0;

impl EditorApp {
    pub(super) fn floating_toolbar(&mut self, ctx: &egui::Context, viewport: Rect) {
        let position = Pos2::new(viewport.left() + 10.0, viewport.top() + 12.0);
        let frame = egui::Frame::new()
            .fill(ctx.global_style().visuals.window_fill.gamma_multiply(0.94))
            .stroke(Stroke::new(
                1.0,
                ctx.global_style()
                    .visuals
                    .widgets
                    .noninteractive
                    .bg_stroke
                    .color,
            ))
            .corner_radius(12.0)
            .inner_margin(egui::Margin::same(5))
            .shadow(ctx.global_style().visuals.popup_shadow);
        egui::Area::new(egui::Id::new("floating_tools"))
            .order(egui::Order::Background)
            .fixed_pos(position)
            .show(ctx, |ui| {
                frame.show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(3.0);
                    for tool in Tool::ALL {
                        let key = self.keymap.binding(tool.action()).map(|b| b.describe());
                        let tip = match (tool, key) {
                            (Tool::Select, Some(key)) => {
                                format!("Select ({key}): click or drag to select")
                            }
                            (_, Some(key)) if tool == self.prefs.tool => {
                                format!("{} ({key}): click to go back to Select", tool.label())
                            }
                            (_, Some(key)) => {
                                format!("{} ({key}): tap to switch, hold to peek", tool.label())
                            }
                            (_, None) => tool.label().to_owned(),
                        };
                        if tool_button(ui, tool_icon(tool), self.prefs.tool == tool, &tip).clicked()
                        {
                            // The active tool's own button returns to Select.
                            if self.prefs.tool == tool && tool != Tool::Select {
                                self.spline.points.clear();
                                self.set_tool(Tool::Select);
                            } else {
                                self.set_tool(tool);
                            }
                        }
                    }
                    if self.prefs.tool == Tool::Select {
                        ui.add_space(2.0);
                        ui.add(egui::Separator::default().spacing(4.0));
                        for mode in SelectMode::ALL {
                            let tip = format!("Select {}", mode.label().to_lowercase());
                            if mode_button(
                                ui,
                                select_mode_icon(mode),
                                self.prefs.select_mode == mode,
                                &tip,
                            )
                            .clicked()
                            {
                                self.prefs.select_mode = mode;
                            }
                        }
                    } else {
                        ui.add_space(2.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("Esc")
                                    .size(10.5)
                                    .color(ui.visuals().widgets.noninteractive.fg_stroke.color),
                            );
                        });
                    }
                });
            });
    }
}

fn tool_button(ui: &mut egui::Ui, icon: &str, active: bool, tooltip: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(BUTTON), Sense::click());
    let visuals = ui.visuals();
    let accent = visuals.selection.stroke.color;
    let fill = if active {
        visuals.selection.bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    let tint = if active {
        accent
    } else if response.hovered() {
        visuals.text_color()
    } else {
        visuals.widgets.noninteractive.fg_stroke.color
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(18.0),
        tint,
    );
    response.on_hover_text(tooltip)
}

/// A smaller button for the selection modes.
fn mode_button(ui: &mut egui::Ui, icon: &str, active: bool, tooltip: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(BUTTON, BUTTON - 6.0), Sense::click());
    let visuals = ui.visuals();
    let accent = visuals.selection.stroke.color;
    if active {
        ui.painter()
            .rect_filled(rect, 7.0, visuals.selection.bg_fill.gamma_multiply(0.7));
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, 7.0, visuals.widgets.hovered.weak_bg_fill);
    }
    let tint = if active {
        accent
    } else {
        visuals.widgets.noninteractive.fg_stroke.color
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(15.0),
        tint,
    );
    response.on_hover_text(tooltip)
}
