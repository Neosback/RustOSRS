//! The tool drawer along the bottom: the active tool's palette.
//!
//! * **Paint**: the tile painter. One tab per channel (overlay, underlay, shape, height, mask); the
//!   checkbox on a tab arms the channel, and unarmed channels keep the tile's own value.
//! * **Spline**: the path's width and edges, plus the overlay it is laid with.
//! * **Height**: sculpting (raise, lower, smooth, flatten, roughen) with strength and falloff.
//!
//! The drawer exists only for tools that have a palette; Select has none.

/// Panel width from which the Paint tab strip and its buttons share one row.
const PAINT_TABS_WIDE: f32 = 620.0;
/// Panel width from which the Paint preview sits beside the palette instead of above it.
const PAINT_BODY_SIDE_BY_SIDE: f32 = 340.0;

/// The palette or picker of the selected Paint tab.
fn paint_tab_body(app: &mut EditorApp, ui: &mut egui::Ui) {
    match app.settings.prefs.drawer_tab {
        DrawerTab::Overlay => app.overlay_palette(ui, false),
        DrawerTab::Underlay => app.underlay_palette(ui, false),
        DrawerTab::Shape => app.shape_picker(ui),
        DrawerTab::Height => app.height_channel(ui),
        DrawerTab::Mask => app.flag_options(ui),
    }
}

/// Collapse chevron and "Fill selected", in tab-strip order (`strip`) or on their own row.
fn paint_drawer_buttons(app: &mut EditorApp, ui: &mut egui::Ui, strip: bool) {
    app.drawer_collapse(ui);
    let selected = app.interaction.selection_tiles().len();
    if selected > 0 && !app.settings.prefs.brush.is_empty() {
        if strip {
            ui.add_space(8.0);
        }
        if ui
            .button(format!("{}  Fill {selected} selected", icons::SELECT_TILES))
            .on_hover_text("Apply the brush to every selected tile")
            .clicked()
        {
            app.fill_selection();
        }
    }
}

mod channels;
mod palettes;
mod tabs;

use palettes::paint_overlay_swatch;
use tabs::channel_tab;

use super::EditorApp;
use super::spline_tool::sections::SplineSections;
use super::widgets::{OverlayFill, paint_tile, swatch};
use crate::heights::HeightOp;
use crate::icons;
use crate::paint::{
    DrawerTab, FLAG_BITS, Fill, MAX_ELEVATION, MAX_SHAPE, PaletteFilter, Tool, elevation,
    height_for_elevation,
};
use eframe::egui::{self, Color32, CornerRadius, Rect, Sense, Stroke, StrokeKind, Vec2};
use osrs_world::{OverlaySwatch, UnderlaySwatch};

/// Palette swatch sizes (small, medium, large) and the preview tile beside them.
const SWATCH_SIZES: [f32; 3] = [40.0, 54.0, 70.0];
const PREVIEW: f32 = 96.0;

fn color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

impl EditorApp {
    // ---------------------------------------------------------------- textures

    // ------------------------------------------------------------------ drawer

    /// The drawer panel above the tool strip (none while selecting). Each tool keeps its own height:
    /// the painter wants room for palettes, the spline and height tools only a row or two.
    pub(super) fn tool_drawer(&mut self, ui: &mut egui::Ui) {
        let tool = self.settings.prefs.tool;
        if !(tool.has_drawer() && self.settings.prefs.drawer_open) {
            return;
        }
        let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
        let (default, min, max) = match tool {
            Tool::Paint => (
                (ui.ctx().content_rect().height() * 0.27).clamp(150.0, 210.0),
                130.0,
                380.0,
            ),
            Tool::Objects => (120.0, 60.0, 360.0),
            Tool::Spline => (260.0, 120.0, 460.0),
            _ => (104.0, 84.0, 200.0),
        };
        egui::Panel::bottom(egui::Id::new(("tool_drawer", tool as u8)))
            .resizable(true)
            .default_size(default)
            .size_range(min..=max)
            .frame(
                egui::Frame::new()
                    .fill(ui.visuals().panel_fill)
                    .stroke(Stroke::new(1.0, line))
                    .inner_margin(egui::Margin::symmetric(10, 6)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(("tool_drawer_scroll", tool as u8))
                    .show(ui, |ui| self.drawer(ui));
            });
    }

    fn drawer(&mut self, ui: &mut egui::Ui) {
        match self.settings.prefs.tool {
            Tool::Paint => self.paint_drawer(ui),
            Tool::Spline => self.spline_drawer(ui, SplineSections::FULL),
            Tool::Height => self.height_drawer(ui),
            Tool::Objects => self.object_drawer(ui),
            Tool::Select => {}
        }
    }

    /// The collapse chevron every drawer header ends with.
    pub(super) fn drawer_collapse(&mut self, ui: &mut egui::Ui) {
        if ui
            .small_button(egui_phosphor::regular::CARET_DOWN)
            .on_hover_text("Hide the drawer (Tab)")
            .clicked()
        {
            self.settings.prefs.drawer_open = false;
        }
    }

    /// The tile painter: one tab per channel, a checkbox arming each, palettes below.
    fn paint_drawer(&mut self, ui: &mut egui::Ui) {
        let tabs = Tool::Paint.tabs();
        if !tabs.contains(&self.settings.prefs.drawer_tab)
            && let Some(first) = tabs.first()
        {
            self.settings.prefs.drawer_tab = *first;
        }
        let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
        let content_fill = ui.visuals().widgets.inactive.bg_fill;
        let mut selected_tab = None;
        // Tabs plus the collapse and fill buttons need about this much; below it the buttons get
        // their own wrapping row and the tabs wrap too instead of running off the panel.
        let wide = ui.available_width() >= PAINT_TABS_WIDE;
        if !wide {
            ui.horizontal_wrapped(|ui| paint_drawer_buttons(self, ui, false));
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for tab in tabs {
                let selected = self.settings.prefs.drawer_tab == *tab;
                let (clicked, rect) = channel_tab(ui, selected, *tab, self);
                if selected {
                    selected_tab = Some(rect);
                }
                if clicked {
                    self.settings.prefs.drawer_tab = *tab;
                }
            }
            if wide {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    paint_drawer_buttons(self, ui, true)
                });
            }
        });
        ui.add_space(-ui.spacing().item_spacing.y);
        // The panel under the tabs; the selected tab opens into it.
        let panel = egui::Frame::new()
            .fill(content_fill)
            .stroke(Stroke::new(1.0, line))
            .corner_radius(CornerRadius {
                nw: 0,
                ne: 8,
                sw: 8,
                se: 8,
            })
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                // The preview beside the controls needs about 320 points; narrower panels stack
                // them so nothing is cut off.
                if ui.available_width() < PAINT_BODY_SIDE_BY_SIDE {
                    ui.vertical(|ui| {
                        self.drawer_preview(ui);
                        paint_tab_body(self, ui);
                    });
                } else {
                    ui.horizontal_top(|ui| {
                        self.drawer_preview(ui);
                        ui.separator();
                        ui.vertical(|ui| {
                            ui.set_width(ui.available_width().max(200.0));
                            paint_tab_body(self, ui);
                        });
                    });
                }
            });
        if let Some(tab) = selected_tab {
            // Erase the panel's top border under the selected tab so the two read as one piece.
            let top = panel.response.rect.top();
            ui.painter().rect_filled(
                Rect::from_min_max(
                    egui::pos2(tab.left() + 1.0, top - 1.0),
                    egui::pos2(tab.right() - 1.0, top + 1.5),
                ),
                0.0,
                content_fill,
            );
        }
    }

    /// The Height tool's drawer: what it does (the shelf has size, strength and falloff).
    fn height_drawer(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Sculpt").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.drawer_collapse(ui);
            });
        });
        ui.add_space(2.0);
        self.height_tool_options(ui);
    }

    /// The Spline workflow, drawn from the shared sections. The drawer shows every section under
    /// its own title row; compact surfaces (the quick HUD) pass fewer. Pure widgets emit actions
    /// that are applied here to their owners.
    pub(in crate::app) fn spline_drawer(&mut self, ui: &mut egui::Ui, sections: SplineSections) {
        use super::spline_tool::sections::{self, SplineAction, SplineView};
        let ctx = ui.ctx().clone();
        if sections.help {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Path").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.drawer_collapse(ui)
                });
            });
        }
        self.spline_update();
        let view = SplineView {
            points: self.interaction.spline_points().len(),
            status: self.interaction.spline_status(),
            can_lay: self.interaction.spline_can_lay(),
        };
        let mut spline_settings = self.settings.prefs.spline.clone();
        let actions = sections::show(ui, sections, &mut spline_settings, &view, |ui| {
            let overlay = match self.settings.prefs.brush.overlay.fill {
                Fill::Set(id) => self
                    .overlay_swatch(id)
                    .map(|swatch| self.overlay_fill(&ctx, &swatch)),
                Fill::Clear => None,
            };
            let response = swatch(ui, 32.0, false, |painter, rect| match overlay {
                Some(fill) => paint_overlay_swatch(painter, rect, fill, false),
                None => {
                    painter.rect_filled(rect, 3.0, Color32::from_gray(38));
                    painter.line_segment(
                        [rect.left_bottom(), rect.right_top()],
                        Stroke::new(1.5, Color32::from_rgb(220, 90, 90)),
                    );
                }
            })
            .on_hover_text("Choose the path overlay");
            egui::Popup::from_toggle_button_response(&response)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    let screen = ui.ctx().content_rect();
                    ui.set_width((screen.width() - 32.0).clamp(120.0, 560.0));
                    ui.set_max_height((screen.height() - 120.0).clamp(120.0, 420.0));
                    self.overlay_palette(ui, true);
                });
            if let Fill::Set(id) = self.settings.prefs.brush.overlay.fill {
                ui.weak(format!("Overlay #{id}"));
            }
            ui.checkbox(
                &mut self.settings.prefs.brush.overlay.armed,
                "Paint overlay",
            );
        });
        self.settings.prefs.spline = spline_settings;
        for action in actions {
            match action {
                SplineAction::Lay => self.commit_spline(),
                SplineAction::RemoveLast => self.interaction.pop_spline_point(),
                SplineAction::Clear => self.interaction.clear_spline_points(),
                SplineAction::CancelPlan => self.interaction.cancel_spline_plan(),
                SplineAction::ResumePlan => self.interaction.resume_spline_plan(),
                SplineAction::CopyDetails => {
                    ctx.copy_text(self.spline_report());
                    self.ui.notify("Copied the path details".to_owned());
                }
            }
        }
    }

    /// What one painted tile will look like.
    fn drawer_preview(&mut self, ui: &mut egui::Ui) {
        let brush = self.settings.prefs.brush.clone();
        let underlay = match (brush.underlay.armed, brush.underlay.fill) {
            (true, Fill::Set(id)) => self.underlay_color(id),
            _ => None,
        };
        let (shape, rotation) = if brush.shape.armed && self.settings.prefs.tool == Tool::Paint {
            (brush.shape.shape, brush.shape.rotation)
        } else {
            (0, 0)
        };
        let overlay = match (brush.overlay.armed, brush.overlay.fill) {
            (true, Fill::Set(id)) => self
                .overlay_swatch(id)
                .map(|swatch| self.overlay_fill(ui.ctx(), &swatch)),
            _ => None,
        };
        ui.vertical(|ui| {
            ui.set_width(PREVIEW + 4.0);
            ui.weak("Preview");
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(PREVIEW), Sense::hover());
            paint_tile(ui.painter(), rect, shape, rotation, underlay, overlay);
            ui.painter().rect_stroke(
                rect,
                3.0,
                Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                StrokeKind::Inside,
            );
            if underlay.is_none() && overlay.is_none() {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "keeps\nthe tile",
                    egui::FontId::proportional(11.5),
                    Color32::from_white_alpha(180),
                );
            }
            response.on_hover_text("The armed colour channels on one tile");
        });
    }
}
