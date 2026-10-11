//! Right-hand inspector: the selected object, and the sections plugins add. It holds no
//! settings; those live in the settings window (`settings_window.rs`).

use super::EditorApp;
use crate::pick::{self, Pick};
use crate::settings::PanelTab;
use eframe::egui;

fn section(ui: &mut egui::Ui, title: &str, default_open: bool, body: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(egui::RichText::new(title).strong())
        .default_open(default_open)
        .show(ui, body);
}

impl EditorApp {
    /// Side panel: a tab strip (Object, Tools) over one scrolling page, docked on the right or
    /// left (Settings > Interface).
    pub(super) fn properties_panel(&mut self, ui: &mut egui::Ui) {
        if !self.prefs.properties_panel {
            return;
        }
        let panel = if self.prefs.dock_right {
            egui::Panel::right("side_panel")
        } else {
            egui::Panel::left("side_panel")
        };
        panel.resizable(true).default_size(300.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                for (tab, label) in [
                    (PanelTab::Object, "Object"),
                    (PanelTab::Settings, "Settings"),
                    (PanelTab::Tools, "Tools"),
                ] {
                    ui.selectable_value(&mut self.prefs.side_tab, tab, label);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(crate::icons::SETTINGS)
                        .on_hover_text("All settings (search every option)")
                        .clicked()
                    {
                        self.open_settings(None);
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| match self.prefs.side_tab {
                PanelTab::Object | PanelTab::View | PanelTab::Shading => self.selection_section(ui),
                PanelTab::Settings => self.quick_settings_ui(ui),
                PanelTab::Tools => {
                    self.run_plugins(|plugin, cx| {
                        if plugin.has_properties() {
                            section(ui, plugin.title(), true, |ui| plugin.properties(ui, cx));
                        }
                    });
                }
            });
        });
    }

    fn selection_section(&mut self, ui: &mut egui::Ui) {
        let Some(object) = self.selected.clone() else {
            ui.weak("Click an object in the viewport to inspect it.");
            return;
        };
        let text = pick::describe_object(&self.infos, &object);
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(text).monospace().size(12.0)).wrap());
            });
        ui.horizontal_wrapped(|ui| {
            let pick = Pick {
                object: Some(object),
                ..Pick::default()
            };
            ui.menu_button("Copy", |ui| self.copy_menu(ui, &pick));
            self.run_plugins(|plugin, cx| plugin.selection_ui(ui, cx));
            if ui.button("Deselect").clicked() {
                self.selected = None;
            }
        });
    }
}
