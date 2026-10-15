//! The dock: the rail of category icons and the panel it swaps.

use super::*;

pub(super) fn rail_icon(tab: RailTab) -> &'static str {
    match tab {
        RailTab::Inspect => icons::INSPECT,
        RailTab::View => icons::VIEW,
        RailTab::Graphics => icons::GRAPHICS,
    }
}

impl EditorApp {
    /// Dock edge: the category rail, then the panel it controls.
    pub(in crate::app) fn dock(&mut self, ui: &mut egui::Ui) {
        let screen = ui.ctx().content_rect().width();
        let compact = screen < COMPACT_WIDTH;
        let rail = if self.settings.prefs.dock_right {
            egui::Panel::right("rail")
        } else {
            egui::Panel::left("rail")
        };
        let inset = ui.visuals().extreme_bg_color;
        let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
        rail.resizable(false)
            .exact_size(RAIL_WIDTH)
            .frame(
                egui::Frame::new()
                    .fill(inset)
                    .stroke(Stroke::new(1.0, line))
                    .inner_margin(egui::Margin::symmetric(0, 6)),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for tab in RailTab::ALL {
                    let active =
                        self.settings.prefs.properties_panel && self.settings.prefs.rail_tab == tab;
                    if self
                        .rail_button(ui, rail_icon(tab), tab.label(), active)
                        .clicked()
                    {
                        if active {
                            self.settings.prefs.properties_panel = false;
                        } else {
                            self.settings.prefs.rail_tab = tab;
                            self.settings.prefs.properties_panel = true;
                        }
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    if self
                        .rail_button(ui, icons::SETTINGS, "All settings (Cmd+,)", false)
                        .clicked()
                    {
                        self.open_settings(None);
                        self.ui.settings_focus_search = true;
                    }
                });
            });
        if !self.settings.prefs.properties_panel {
            return;
        }
        if self.settings.prefs.dock_floating {
            self.floating_dock(ui.ctx());
            return;
        }
        let panel = if self.settings.prefs.dock_right {
            egui::Panel::right("dock")
        } else {
            egui::Panel::left("dock")
        };
        let width = if compact { 264.0 } else { 312.0 };
        panel
            .resizable(true)
            .default_size(width)
            .size_range(220.0..=(screen * 0.45).max(260.0))
            .show(ui, |ui| self.dock_contents(ui));
    }

    /// The panel as a window you can drag anywhere and resize (the rail stays on the edge).
    pub(super) fn floating_dock(&mut self, ctx: &egui::Context) {
        let [x, y] = self.settings.prefs.dock_float_pos;
        let [w, h] = self.settings.prefs.dock_float_size;
        let mut open = true;
        let response = egui::Window::new(self.settings.prefs.rail_tab.label())
            .id(egui::Id::new("dock_window"))
            .open(&mut open)
            .default_pos(egui::pos2(x, y))
            .default_size([w, h])
            .min_size([260.0, 240.0])
            .collapsible(false)
            .show(ctx, |ui| self.dock_contents(ui));
        if let Some(response) = response {
            let rect = response.response.rect;
            self.settings.prefs.dock_float_pos = [rect.min.x, rect.min.y];
            self.settings.prefs.dock_float_size = [rect.width(), rect.height()];
        }
        if !open {
            self.settings.prefs.properties_panel = false;
        }
    }

    /// The panel's header (title, tabs, side and pop-out buttons) and body.
    pub(super) fn dock_contents(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if !self.settings.prefs.dock_floating {
                ui.label(
                    egui::RichText::new(self.settings.prefs.rail_tab.label())
                        .strong()
                        .size(15.0),
                );
            }
            if self.settings.prefs.rail_tab == RailTab::View {
                ui.add_space(8.0);
                for tab in ViewTab::ALL {
                    if tab_label(ui, self.settings.prefs.view_tab == tab, tab.label()).clicked() {
                        self.settings.prefs.view_tab = tab;
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (icon, tip) = if self.settings.prefs.dock_floating {
                    (icons::DOCK_IN, "Dock the panel back to the edge")
                } else {
                    (icons::POP_OUT, "Pop the panel out into a window")
                };
                if ui
                    .add(egui::Button::new(icon).small().frame(false))
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.settings.prefs.dock_floating = !self.settings.prefs.dock_floating;
                }
                if !self.settings.prefs.dock_floating
                    && ui
                        .add(egui::Button::new(icons::SWAP_SIDE).small().frame(false))
                        .on_hover_text("Move the panel to the other side")
                        .clicked()
                {
                    self.settings.prefs.dock_right = !self.settings.prefs.dock_right;
                }
            });
        });
        ui.add_space(2.0);
        ui.separator();
        match self.settings.prefs.rail_tab {
            // The inspector splits the panel itself (tile above, object below).
            RailTab::Inspect => self.inspector(ui),
            _ => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.settings.prefs.rail_tab {
                        RailTab::View | RailTab::Graphics => match self.settings.prefs.view_tab {
                            ViewTab::Scene => {
                                self.quick_settings_ui(ui, &["Camera", "World", "Interface"]);
                            }
                            ViewTab::Graphics => {
                                self.quick_settings_ui(ui, &["Graphics"]);
                            }
                            ViewTab::Overlays => self.overlays_tab(ui),
                        },
                        RailTab::Inspect => {}
                    });
            }
        }
    }

    pub(super) fn rail_button(
        &self,
        ui: &mut egui::Ui,
        icon: &str,
        tooltip: &str,
        active: bool,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(RAIL_WIDTH, 40.0), Sense::click());
        let visuals = ui.visuals();
        let accent = visuals.selection.stroke.color;
        if active {
            ui.painter().rect_filled(
                rect,
                0.0,
                visuals.widgets.inactive.bg_fill.gamma_multiply(0.6),
            );
            let bar = if self.settings.prefs.dock_right {
                Rect::from_min_max(
                    Pos2::new(rect.right() - 2.5, rect.top() + 6.0),
                    Pos2::new(rect.right(), rect.bottom() - 6.0),
                )
            } else {
                Rect::from_min_max(
                    Pos2::new(rect.left(), rect.top() + 6.0),
                    Pos2::new(rect.left() + 2.5, rect.bottom() - 6.0),
                )
            };
            ui.painter().rect_filled(bar, 1.5, accent);
        } else if response.hovered() {
            ui.painter().rect_filled(
                rect,
                0.0,
                visuals.widgets.hovered.weak_bg_fill.gamma_multiply(0.5),
            );
        }
        let tint = if active {
            accent
        } else if response.hovered() {
            visuals.text_color()
        } else {
            visuals.weak_text_color()
        };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(19.0),
            tint,
        );
        response.on_hover_text(tooltip)
    }

    /// The View panel's Overlays tab: what the viewport shows on top of the world.
    pub(super) fn overlays_tab(&mut self, ui: &mut egui::Ui) {
        crate::theme::section_heading(ui, "Heads-up");
        crate::theme::toggle(ui, &mut self.settings.prefs.tile_info_hud, "Tile info")
            .on_hover_text("The tile under the cursor, in the corner of the viewport");
        crate::theme::toggle(ui, &mut self.settings.prefs.stats_overlay, "Statistics")
            .on_hover_text("Frame rate, timings and the camera tile");
        crate::theme::toggle(
            ui,
            &mut self.settings.prefs.show_minimap,
            "Minimap, compass and globe",
        );
        crate::theme::toggle(
            ui,
            &mut self.settings.prefs.map_icons_3d,
            "Map icons in the world",
        )
        .on_hover_text("Bank, shop and altar icons floating over their tiles");
        crate::theme::toggle(
            ui,
            &mut self.settings.prefs.mouse_tooltips,
            "Action tooltip",
        )
        .on_hover_text("The game's action text next to the cursor");
        crate::theme::section_heading(ui, "Indicators");
        crate::theme::toggle(
            ui,
            &mut self.settings.prefs.hover_tile,
            "Tile outline and brush footprint",
        )
        .on_hover_text("Outline the tile (or the tiles the brush would paint) under the cursor");
        if self.plugin_enabled("grid") {
            crate::theme::section_heading(ui, "Grid");
        }
        self.run_plugins(|plugin, cx| {
            if plugin.name() == "grid" {
                plugin.properties(ui, cx);
            }
        });
        ui.add_space(8.0);
        ui.separator();
        if ui
            .button(format!("{}  Colours...", icons::THEME))
            .on_hover_text("Colour and opacity of the outlines, grid and HUD")
            .clicked()
        {
            self.open_settings(Some(crate::settings::SettingsPage::Colors));
        }
    }
}
