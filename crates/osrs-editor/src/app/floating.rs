//! The floating picker rail on the viewport: how clicks and boxes pick things.
//!
//! It holds pickers only (the editing tools are in the bottom strip). Plain picking is **Auto**: a click
//! takes the object under the cursor, else the tile, and a box takes both. **Objects** and **Tiles** narrow
//! it when you need exactly one kind. The rail can be dragged by its grip and collapsed; both are kept in
//! the settings. Choosing a picker while an editing tool is active returns to selecting.

use super::EditorApp;
use super::workspace::select_mode_icon;
use crate::paint::{SelectMode, Tool};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

const BUTTON: f32 = 34.0;

impl EditorApp {
    pub(super) fn floating_toolbar(&mut self, ctx: &egui::Context, viewport: Rect) {
        let default_pos =
            viewport.min + Vec2::new(self.prefs.picker_pos[0], self.prefs.picker_pos[1]);
        let selecting = self.prefs.tool == Tool::Select;
        let frame = egui::Frame::new()
            .fill(
                ctx.global_style()
                    .visuals
                    .window_fill
                    .gamma_multiply(if selecting { 0.94 } else { 0.7 }),
            )
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
        let area = egui::Area::new(egui::Id::new("picker_rail"))
            .order(egui::Order::Background)
            .movable(true)
            .default_pos(default_pos)
            .constrain_to(viewport)
            .show(ctx, |ui| {
                frame.show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(3.0);
                    // The grip drags the rail (the whole frame does, but the grip shows it can); the
                    // chevron collapses it.
                    ui.horizontal(|ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::new(BUTTON - 14.0, 16.0), Sense::hover());
                        let tint = ui
                            .visuals()
                            .widgets
                            .noninteractive
                            .fg_stroke
                            .color
                            .gamma_multiply(0.6);
                        for dy in [-3.0, 0.0, 3.0] {
                            ui.painter().circle_filled(
                                rect.center() + Vec2::new(-3.0, dy),
                                1.2,
                                tint,
                            );
                            ui.painter().circle_filled(
                                rect.center() + Vec2::new(3.0, dy),
                                1.2,
                                tint,
                            );
                        }
                        response.on_hover_text("Drag to move the picker rail");
                        let (icon, tip) = if self.prefs.picker_collapsed {
                            (egui_phosphor::regular::CARET_DOWN, "Expand")
                        } else {
                            (egui_phosphor::regular::CARET_UP, "Collapse")
                        };
                        if ui
                            .add(egui::Button::new(icon).small().frame(false))
                            .on_hover_text(tip)
                            .clicked()
                        {
                            self.prefs.picker_collapsed = !self.prefs.picker_collapsed;
                        }
                    });
                    let modes: Vec<SelectMode> = if self.prefs.picker_collapsed {
                        vec![self.prefs.select_mode]
                    } else {
                        SelectMode::ALL.to_vec()
                    };
                    for mode in modes {
                        let active = selecting && self.prefs.select_mode == mode;
                        if picker_button(
                            ui,
                            select_mode_icon(mode),
                            active,
                            &format!("{}: {}", mode.label(), mode.hint()),
                        )
                        .clicked()
                        {
                            self.prefs.select_mode = mode;
                            if !selecting {
                                self.spline.points.clear();
                                self.set_tool(Tool::Select);
                            }
                        }
                    }
                });
            });
        let offset = area.response.rect.min - viewport.min;
        if (offset.x - self.prefs.picker_pos[0]).abs() > 0.5
            || (offset.y - self.prefs.picker_pos[1]).abs() > 0.5
        {
            self.prefs.picker_pos = [offset.x, offset.y];
        }
    }
}

fn picker_button(ui: &mut egui::Ui, icon: &str, active: bool, tooltip: &str) -> egui::Response {
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
    // A small corner tick marks the active picker's kind is applied now.
    let _ = Pos2::ZERO;
    response.on_hover_text(tooltip)
}
