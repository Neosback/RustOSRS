//! The primary tool strip along the bottom: the editing tools. No tool active means selecting.
//!
//! This is the RSPSi / Displee "primary tool rail": it only chooses the active editing mode (paint a
//! tile, lay a path, sculpt heights); what a tool needs is in the left shelf (how it acts) and the
//! drawer above this strip (what it paints with). Picking things is the floating picker rail's job.

use super::EditorApp;
use super::workspace::{COMPACT_WIDTH, tool_icon};
use crate::paint::Tool;
use eframe::egui::{self, Stroke};

impl EditorApp {
    pub(super) fn tool_strip(&mut self, ui: &mut egui::Ui) {
        let compact = ui.ctx().content_rect().width() < COMPACT_WIDTH;
        let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
        egui::Panel::bottom("tool_strip")
            .resizable(false)
            .exact_size(36.0)
            .frame(
                egui::Frame::new()
                    .fill(ui.visuals().panel_fill)
                    .stroke(Stroke::new(1.0, line))
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for tool in Tool::ALL.into_iter().filter(|tool| *tool != Tool::Select) {
                        let text = if compact {
                            tool_icon(tool).to_owned()
                        } else {
                            format!("{}  {}", tool_icon(tool), tool.label())
                        };
                        let key = self
                            .settings
                            .keymap
                            .binding(tool.action())
                            .map(|b| b.describe());
                        let active = self.settings.prefs.tool == tool;
                        let tip = match (&key, active) {
                            (Some(key), true) => format!(
                                "{} ({key}): click to stop and go back to selecting",
                                tool.label()
                            ),
                            (Some(key), false) => {
                                format!("{} ({key}): tap to switch, hold to peek", tool.label())
                            }
                            (None, _) => tool.label().to_owned(),
                        };
                        if ui
                            .selectable_label(active, text)
                            .on_hover_text(tip)
                            .clicked()
                        {
                            if active {
                                self.interaction.clear_spline_points();
                                self.set_tool(Tool::Select);
                            } else {
                                self.set_tool(tool);
                            }
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !compact {
                            let hint = if self.settings.prefs.tool == Tool::Select {
                                "Click or drag to select \u{b7} pick a tool to edit"
                            } else {
                                "Esc goes back to selecting"
                            };
                            ui.weak(hint);
                        }
                    });
                });
            });
    }
}
