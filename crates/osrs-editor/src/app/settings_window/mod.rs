//! The settings window: every persisted option in one place, in pages with a search box.
//!
//! One table ([`ROWS`]) declares each setting once: its page, section, label and search keywords.
//! The window, the search results and the profile gating all read that table; [`EditorApp::draw_row`]
//! is the only place a row's widget exists. Adding a setting means one `Row` variant, one table
//! entry and one match arm. The side panel (inspector) never shows settings.

use super::EditorApp;

mod catalog;
mod pages;
mod row_widgets;
mod rows;
#[cfg(test)]
mod tests;

use crate::keymap::Binding;
use crate::settings::{ProfileChoice, SettingsPage};
use SettingsPage::{Advanced, Colors, Controls, HdLighting, HdShadowsWater, Plugins};
use catalog::Applies::{Any, Hd, Reference};
use catalog::{PAGES, QUICK, Row, RowDef, matches};
use eframe::egui;
use egui_phosphor::regular as ph;
use osrs_hd::environment::FogDepthMode;
use pages::nav_item;
use rows::ROWS;

impl EditorApp {
    /// Open the settings window on `page`.
    pub(super) fn open_settings(&mut self, page: Option<SettingsPage>) {
        if let Some(page) = page {
            self.settings.prefs.settings_page = page;
        }
        self.ui.open_settings();
    }

    /// The side panel's Settings tab: the frequently used rows, stacked (label over control) to
    /// fit a narrow panel. They are drawn by [`Self::draw_row`], like the window's.
    pub(super) fn quick_settings_ui(&mut self, ui: &mut egui::Ui, groups: &[&str]) {
        let hd = self.settings.prefs.render_profile == ProfileChoice::Hd;
        // Leave room for the value box ("3x3 regions"); the cap stops a wide value from growing the panel.
        ui.spacing_mut().slider_width = (ui.available_width() - 120.0).clamp(80.0, 300.0);
        for (group, rows) in QUICK {
            if !groups.contains(group) {
                continue;
            }
            let shown: Vec<&RowDef> = rows
                .iter()
                .filter_map(|row| ROWS.iter().find(|def| def.row == *row))
                .filter(|def| match def.applies {
                    Any => true,
                    Reference => !hd,
                    Hd => hd,
                })
                .filter(|def| self.row_visible(def.row))
                .collect();
            if shown.is_empty() {
                continue;
            }
            crate::theme::section_heading(ui, group);
            for def in shown {
                let toggle = match def.row {
                    Row::AnimateObjects => Some(&mut self.settings.prefs.animate_objects),
                    Row::ShowMinimap => Some(&mut self.settings.prefs.show_minimap),
                    Row::StatsOverlay => Some(&mut self.settings.prefs.stats_overlay),
                    _ => None,
                };
                if let Some(value) = toggle {
                    crate::theme::toggle(ui, value, def.label);
                    continue;
                }
                ui.label(egui::RichText::new(def.label).weak());
                self.draw_row(ui, def.row);
            }
        }
        ui.add_space(10.0);
        ui.separator();
        if ui
            .button(format!("{}  All settings...", ph::GEAR))
            .on_hover_text("Every option, with search")
            .clicked()
        {
            self.open_settings(None);
            self.ui.settings_focus_search = true;
        }
    }

    /// Whether a row is shown at all (some only make sense next to another setting).
    fn row_visible(&self, row: Row) -> bool {
        match row {
            Row::StaticFogDepth => {
                self.settings.prefs.hd_environment.fog_depth_mode == FogDepthMode::Static
            }
            Row::ShadowQuality
            | Row::ShadowFiltering
            | Row::ShadowDistance
            | Row::ShadowTransparency
            | Row::TerrainShadows
            | Row::RoofShadows => self.settings.prefs.hd_shadows.enabled(),
            Row::ShadowMemory => {
                self.settings.prefs.hd_shadows.enabled() && self.renderer.hd_shadow_map_bytes() > 0
            }
            _ => true,
        }
    }

    /// The window. Called every frame; draws nothing while closed.
    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.ui.settings_open {
            self.ui.capturing_binding = None;
            return;
        }
        // A binding waiting for a key press: the first key pressed becomes the binding.
        if let Some(id) = self.ui.capturing_binding {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.ui.capturing_binding = None;
            } else if let Some(binding) = ctx.input(Binding::capture) {
                self.settings.keymap.set(id, binding);
                self.ui.capturing_binding = None;
            }
        }
        let esc = self.ui.capturing_binding.is_none();
        let ((), close) =
            super::modal::Modal::new("settings_window", format!("{} Settings", ph::GEAR))
                .size(780.0, 520.0)
                .esc_closes(esc)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(ph::MAGNIFYING_GLASS);
                        let search = ui.add(
                            egui::TextEdit::singleline(&mut self.ui.settings_search)
                                .hint_text("Search settings")
                                .desired_width(260.0),
                        );
                        if self.ui.settings_focus_search {
                            search.request_focus();
                            self.ui.settings_focus_search = false;
                        }
                        if !self.ui.settings_search.is_empty() && ui.small_button("clear").clicked()
                        {
                            self.ui.settings_search.clear();
                        }
                    });
                    ui.separator();
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(176.0);
                            ui.spacing_mut().item_spacing.y = 3.0;
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                            for (page, icon, title) in PAGES {
                                let selected = self.ui.settings_search.is_empty()
                                    && self.settings.prefs.settings_page == *page;
                                if nav_item(ui, selected, icon, title).clicked() {
                                    self.settings.prefs.settings_page = *page;
                                    self.ui.settings_search.clear();
                                }
                            }
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if self.ui.settings_search.trim().is_empty() {
                                        self.page_ui(ui, self.settings.prefs.settings_page);
                                    } else {
                                        self.search_ui(ui);
                                    }
                                });
                        });
                    });
                });
        if close {
            self.ui.settings_open = false;
        }
    }

    /// One page.
    fn page_ui(&mut self, ui: &mut egui::Ui, page: SettingsPage) {
        let title = PAGES
            .iter()
            .find(|(candidate, _, _)| *candidate == page)
            .map_or("", |(_, _, title)| *title);
        ui.heading(title);
        ui.add_space(4.0);
        match page {
            Controls => self.controls_page(ui),
            Colors => self.colors_page(ui),
            Plugins => self.plugin_options_page(ui),
            Advanced => {
                self.rows_ui_for_page(ui, page, "");
                self.advanced_page(ui);
            }
            _ => {
                if matches!(page, HdLighting | HdShadowsWater)
                    && self.settings.prefs.render_profile != ProfileChoice::Hd
                {
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(240, 180, 80),
                            "These options act on the HD profile.",
                        );
                        if ui.button("Switch to HD").clicked() {
                            self.settings.prefs.render_profile = ProfileChoice::Hd;
                        }
                    });
                    ui.add_space(4.0);
                }
                self.rows_ui_for_page(ui, page, "");
            }
        }
    }

    /// Search results across every page, with the page named on each section.
    fn search_ui(&mut self, ui: &mut egui::Ui) {
        let query = self.ui.settings_search.trim().to_lowercase();
        let found = ROWS
            .iter()
            .filter(|def| matches(def, &query) && self.row_visible(def.row))
            .count();
        ui.heading("Search results");
        if found == 0 {
            ui.weak("No setting matches.");
            return;
        }
        for (page, _, title) in PAGES {
            if ROWS
                .iter()
                .any(|def| def.page == *page && matches(def, &query) && self.row_visible(def.row))
            {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(*title).weak());
                self.rows_ui_for_page(ui, *page, &query);
            }
        }
    }

    fn rows_ui_for_page(&mut self, ui: &mut egui::Ui, page: SettingsPage, query: &str) {
        let rows: Vec<&RowDef> = ROWS
            .iter()
            .filter(|def| {
                def.page == page
                    && (query.is_empty() || matches(def, query))
                    && self.row_visible(def.row)
            })
            .collect();
        let mut sections: Vec<&str> = Vec::new();
        for def in &rows {
            if !sections.contains(&def.section) {
                sections.push(def.section);
            }
        }
        for section in sections {
            crate::theme::section_heading(ui, section);
            ui.spacing_mut().slider_width = 220.0;
            egui::Grid::new(("settings_grid", section, page as u8))
                .num_columns(2)
                .spacing([14.0, 7.0])
                .min_col_width(190.0)
                .show(ui, |ui| {
                    for def in rows.iter().filter(|def| def.section == section) {
                        self.draw_labelled_row(ui, def);
                    }
                });
        }
    }

    /// A label and its widget, greyed out when the setting acts on the other render profile.
    fn draw_labelled_row(&mut self, ui: &mut egui::Ui, def: &RowDef) {
        let profile_hd = self.settings.prefs.render_profile == ProfileChoice::Hd;
        let enabled = match def.applies {
            Any => true,
            Reference => !profile_hd,
            Hd => profile_hd,
        };
        let suffix = match (def.applies, enabled) {
            (Reference, false) => "  (Reference)",
            (Hd, false) => "  (HD)",
            _ => "",
        };
        ui.label(format!("{}{}", def.label, suffix));
        ui.add_enabled_ui(enabled, |ui| self.draw_row(ui, def.row));
        ui.end_row();
    }
}
