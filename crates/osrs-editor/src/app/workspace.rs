//! The workspace chrome around the viewport (the Displee-style layout):
//!
//! * a **rail** of category icons on the dock edge; each swaps the dock panel's contents,
//! * the **dock panel** (inspector, view, graphics, plugins),
//! * the **quick tools** HUD (Space) and the **tile info** HUD,
//! * tool switching: keys that tap-to-switch and hold-to-peek, and `Esc` back to Select.
//!
//! The tool drawer is in `drawer.rs`, the floating toolbar in `floating.rs`. Everything is context
//! aware: the drawer exists only for tools with a palette, the tile HUD only over terrain, and the
//! quick HUD offers what the active tool can use.

use super::EditorApp;
use super::widgets::tab_label;
use crate::icons;
use crate::paint::{SelectMode, TileValues, Tool};
use crate::settings::{RailTab, ViewTab};
use eframe::egui::{self, Pos2, Rect, Sense, Stroke, Vec2};
use std::time::Duration;
use web_time::Instant;

/// Window width (points) under which labels collapse to icons and the dock narrows.
pub const COMPACT_WIDTH: f32 = 1100.0;
const RAIL_WIDTH: f32 = 44.0;
/// Holding a tool key longer than this peeks at the tool and returns on release.
const PEEK_AFTER: Duration = Duration::from_millis(280);

/// The tool's icon.
pub fn tool_icon(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => icons::TOOL_SELECT,
        Tool::Paint => icons::TOOL_PAINT,
        Tool::Spline => icons::TOOL_SPLINE,
        Tool::Height => icons::TOOL_HEIGHT,
        Tool::Objects => icons::TOOL_OBJECTS,
    }
}

pub fn select_mode_icon(mode: SelectMode) -> &'static str {
    match mode {
        SelectMode::Auto => icons::SELECT_AUTO,
        SelectMode::Objects => icons::SELECT_OBJECTS,
        SelectMode::Tiles => icons::SELECT_TILES,
    }
}

fn rail_icon(tab: RailTab) -> &'static str {
    match tab {
        RailTab::Inspect => icons::INSPECT,
        RailTab::View => icons::VIEW,
        RailTab::Graphics => icons::GRAPHICS,
    }
}

impl EditorApp {
    // ---------------------------------------------------------------- palette

    /// Take the floor palette from its loader thread once it arrives.
    pub(super) fn poll_palette(&mut self) {
        if let Some(receiver) = &self.palette_rx
            && let Ok(result) = receiver.try_recv()
        {
            self.palette = result.ok().map(std::sync::Arc::new);
            self.palette_rx = None;
        }
    }

    // ------------------------------------------------------------------ rail

    /// Dock edge: the category rail, then the panel it controls.
    pub(super) fn dock(&mut self, ui: &mut egui::Ui) {
        let screen = ui.ctx().content_rect().width();
        let compact = screen < COMPACT_WIDTH;
        let rail = if self.prefs.dock_right {
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
                    let active = self.prefs.properties_panel && self.prefs.rail_tab == tab;
                    if self
                        .rail_button(ui, rail_icon(tab), tab.label(), active)
                        .clicked()
                    {
                        if active {
                            self.prefs.properties_panel = false;
                        } else {
                            self.prefs.rail_tab = tab;
                            self.prefs.properties_panel = true;
                        }
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    if self
                        .rail_button(ui, icons::SETTINGS, "All settings (Cmd+,)", false)
                        .clicked()
                    {
                        self.open_settings(None);
                        self.settings_focus_search = true;
                    }
                });
            });
        if !self.prefs.properties_panel {
            return;
        }
        if self.prefs.dock_floating {
            self.floating_dock(ui.ctx());
            return;
        }
        let panel = if self.prefs.dock_right {
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
    fn floating_dock(&mut self, ctx: &egui::Context) {
        let [x, y] = self.prefs.dock_float_pos;
        let [w, h] = self.prefs.dock_float_size;
        let mut open = true;
        let response = egui::Window::new(self.prefs.rail_tab.label())
            .id(egui::Id::new("dock_window"))
            .open(&mut open)
            .default_pos(egui::pos2(x, y))
            .default_size([w, h])
            .min_size([260.0, 240.0])
            .collapsible(false)
            .show(ctx, |ui| self.dock_contents(ui));
        if let Some(response) = response {
            let rect = response.response.rect;
            self.prefs.dock_float_pos = [rect.min.x, rect.min.y];
            self.prefs.dock_float_size = [rect.width(), rect.height()];
        }
        if !open {
            self.prefs.properties_panel = false;
        }
    }

    /// The panel's header (title, tabs, side and pop-out buttons) and body.
    fn dock_contents(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if !self.prefs.dock_floating {
                ui.label(
                    egui::RichText::new(self.prefs.rail_tab.label())
                        .strong()
                        .size(15.0),
                );
            }
            if self.prefs.rail_tab == RailTab::View {
                ui.add_space(8.0);
                for tab in ViewTab::ALL {
                    if tab_label(ui, self.prefs.view_tab == tab, tab.label()).clicked() {
                        self.prefs.view_tab = tab;
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (icon, tip) = if self.prefs.dock_floating {
                    (icons::DOCK_IN, "Dock the panel back to the edge")
                } else {
                    (icons::POP_OUT, "Pop the panel out into a window")
                };
                if ui
                    .add(egui::Button::new(icon).small().frame(false))
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.prefs.dock_floating = !self.prefs.dock_floating;
                }
                if !self.prefs.dock_floating
                    && ui
                        .add(egui::Button::new(icons::SWAP_SIDE).small().frame(false))
                        .on_hover_text("Move the panel to the other side")
                        .clicked()
                {
                    self.prefs.dock_right = !self.prefs.dock_right;
                }
            });
        });
        ui.add_space(2.0);
        ui.separator();
        match self.prefs.rail_tab {
            // The inspector splits the panel itself (tile above, object below).
            RailTab::Inspect => self.inspector(ui),
            _ => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.prefs.rail_tab {
                        RailTab::View | RailTab::Graphics => match self.prefs.view_tab {
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

    fn rail_button(
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
            let bar = if self.prefs.dock_right {
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
            visuals.widgets.noninteractive.fg_stroke.color
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
    fn overlays_tab(&mut self, ui: &mut egui::Ui) {
        crate::theme::section_heading(ui, "Heads-up");
        crate::theme::toggle(ui, &mut self.prefs.tile_info_hud, "Tile info")
            .on_hover_text("The tile under the cursor, in the corner of the viewport");
        crate::theme::toggle(ui, &mut self.prefs.stats_overlay, "Statistics")
            .on_hover_text("Frame rate, timings and the camera tile");
        crate::theme::toggle(
            ui,
            &mut self.prefs.show_minimap,
            "Minimap, compass and globe",
        );
        crate::theme::toggle(ui, &mut self.prefs.map_icons_3d, "Map icons in the world")
            .on_hover_text("Bank, shop and altar icons floating over their tiles");
        crate::theme::toggle(ui, &mut self.prefs.mouse_tooltips, "Action tooltip")
            .on_hover_text("The game's action text next to the cursor");
        crate::theme::section_heading(ui, "Indicators");
        crate::theme::toggle(
            ui,
            &mut self.prefs.hover_tile,
            "Tile outline and brush footprint",
        )
        .on_hover_text("Outline the tile (or the tiles the brush would paint) under the cursor");
        crate::theme::section_heading(ui, "Grid");
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

    /// Fill the brush from a tile (`I`, Alt + click, or the tile's context menu).
    pub(super) fn eyedrop(&mut self, values: TileValues) {
        self.prefs.brush.sample(values);
        if self.prefs.tool == Tool::Select {
            self.set_tool(Tool::Paint);
        }
        self.notice = Some((
            format!(
                "Brush: overlay {}, underlay {}",
                values.overlay, values.underlay
            ),
            Instant::now(),
        ));
    }

    pub(super) fn set_tool(&mut self, tool: Tool) {
        if self.prefs.tool == tool {
            return;
        }
        self.stroke = None;
        self.marquee = None;
        self.prefs.tool = tool;
        let tabs = tool.tabs();
        if let Some(first) = tabs.first() {
            if !tabs.contains(&self.prefs.drawer_tab) {
                self.prefs.drawer_tab = *first;
            }
            self.prefs.drawer_open = true;
        }
    }

    // -------------------------------------------------------------- quick HUD

    /// Space: a small panel at the cursor with what the active tool can use.
    pub(super) fn quick_tools_hud(&mut self, ctx: &egui::Context) {
        let Some(origin) = self.quick_open else {
            return;
        };
        let screen = ctx.content_rect();
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let width = 292.0;
        let position = Pos2::new(
            (origin.x - width / 2.0)
                .clamp(screen.left() + 8.0, (screen.right() - width - 8.0).max(8.0)),
            (origin.y - 40.0).clamp(screen.top() + 40.0, (screen.bottom() - 300.0).max(48.0)),
        );
        let area = egui::Area::new(egui::Id::new("quick_tools"))
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(width - 20.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Quick tools").strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.weak("Esc closes");
                                },
                            );
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            for tool in Tool::ALL {
                                let key = self.keymap.binding(tool.action()).map(|b| b.describe());
                                if quick_tool_button(ui, tool, self.prefs.tool == tool, key)
                                    .clicked()
                                {
                                    self.set_tool(tool);
                                    close = true;
                                }
                            }
                        });
                        self.quick_tool_options(ui);
                        crate::theme::section_heading(ui, "View");
                        ui.horizontal(|ui| {
                            ui.weak("Plane");
                            for plane in 0..4_u8 {
                                ui.selectable_value(&mut self.view_plane, plane, plane.to_string());
                            }
                            ui.separator();
                            ui.selectable_value(
                                &mut self.camera.mode,
                                crate::camera::CameraMode::Fly,
                                "Fly",
                            );
                            ui.selectable_value(
                                &mut self.camera.mode,
                                crate::camera::CameraMode::Orbit,
                                "Orbit",
                            );
                        });
                    });
            });
        // A click outside closes it.
        if ctx.input(|i| i.pointer.any_pressed())
            && let Some(pointer) = ctx.input(|i| i.pointer.interact_pos())
            && !area.response.rect.expand(4.0).contains(pointer)
        {
            close = true;
        }
        if close {
            self.quick_open = None;
        }
    }

    /// The quick HUD's section for the active tool.
    fn quick_tool_options(&mut self, ui: &mut egui::Ui) {
        match self.prefs.tool {
            Tool::Select => {
                crate::theme::section_heading(ui, "Select");
                ui.horizontal(|ui| {
                    for mode in SelectMode::ALL {
                        let text = format!("{}  {}", select_mode_icon(mode), mode.label());
                        ui.selectable_value(&mut self.prefs.select_mode, mode, text);
                    }
                });
            }
            Tool::Paint => {
                crate::theme::section_heading(ui, "Brush paints");
                ui.horizontal_wrapped(|ui| {
                    for tab in Tool::Paint.tabs() {
                        let mut armed = self.prefs.brush.is_armed(*tab);
                        if ui.toggle_value(&mut armed, tab.label()).changed() {
                            self.prefs.brush.set_armed(*tab, armed);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.weak("Brush");
                    self.brush_controls(ui, true, false);
                });
            }
            Tool::Spline => {
                crate::theme::section_heading(ui, "Path");
                ui.horizontal(|ui| {
                    ui.weak("Width");
                    ui.add(
                        egui::DragValue::new(&mut self.prefs.spline.width)
                            .range(1.0..=32.0)
                            .speed(0.1)
                            .suffix(" tiles"),
                    );
                    for style in crate::autotile::EdgeStyle::ALL {
                        ui.selectable_value(&mut self.prefs.spline.style, style, style.label());
                    }
                });
            }
            Tool::Height => {
                crate::theme::section_heading(ui, "Height");
                let current = self.prefs.height_tool.op;
                let options: Vec<_> = crate::heights::HeightOp::ALL
                    .iter()
                    .map(|op| (*op, op.label()))
                    .collect();
                if let Some(op) = super::widgets::chips(ui, current, &options) {
                    self.prefs.height_tool.op = op;
                }
                ui.horizontal(|ui| {
                    ui.weak("Brush");
                    self.brush_controls(ui, true, false);
                });
            }
            Tool::Objects => {
                crate::theme::section_heading(ui, "Objects");
                self.object_drawer(ui);
            }
        }
    }

    // ---------------------------------------------------------------- tile HUD

    /// The tile under the cursor, only while the cursor is over terrain.
    pub(super) fn tile_info_lines(&self) -> Option<Vec<(String, String)>> {
        let tile = self.hover.tile.map(|pick| pick.tile)?;
        let values = self.tile_now(tile)?;
        let mut lines = vec![
            (
                "Tile".to_owned(),
                format!("{}, {}  \u{b7}  plane {}", tile.0, tile.1, self.view_plane),
            ),
            (
                "Region".to_owned(),
                crate::hud::region_id(tile.0 >> 6, tile.1 >> 6).to_string(),
            ),
            (
                "Overlay".to_owned(),
                if values.overlay == 0 {
                    "none".to_owned()
                } else {
                    format!(
                        "{}  \u{b7}  shape {}  rot {}",
                        values.overlay, values.shape, values.rotation
                    )
                },
            ),
            (
                "Underlay".to_owned(),
                if values.underlay == 0 {
                    "none".to_owned()
                } else {
                    values.underlay.to_string()
                },
            ),
            (
                "Height".to_owned(),
                format!(
                    "{}  \u{b7}  flags {:#04x}",
                    crate::paint::elevation(values.height),
                    values.flags
                ),
            ),
        ];
        if let Some(object) = &self.hover.object
            && let Some(loc) = self
                .infos
                .get(&object.region)
                .and_then(|info| info.locs.get(object.index))
        {
            lines.push((
                "Object".to_owned(),
                format!(
                    "{} ({})",
                    loc.definition.name.as_deref().unwrap_or("?"),
                    loc.definition.identity.id.get()
                ),
            ));
        }
        Some(lines)
    }

    // ---------------------------------------------------------------- actions

    /// A held tool key returns to the previous tool when released (called every frame, even
    /// while a text field has focus, so a held key can never strand the tool).
    pub(super) fn tool_release(&mut self, ctx: &egui::Context) {
        let Some((previous, key, since)) = self.peek else {
            return;
        };
        if ctx.input(|i| i.key_down(key)) {
            return;
        }
        self.peek = None;
        if since.elapsed() > PEEK_AFTER {
            self.set_tool(previous);
        }
    }

    /// Tool, drawer and brush shortcuts.
    ///
    /// A tool key *taps* (switches and stays; pressing the same key again returns to Select) or
    /// *holds* (peeks at the tool and returns to the previous one on release).
    pub(super) fn tool_actions(&mut self, ctx: &egui::Context) {
        let pressed = |id: &str| ctx.input(|i| self.keymap.pressed(i, id));
        let tool = Tool::ALL.into_iter().find(|tool| pressed(tool.action()));
        let quick = pressed("quick.tools");
        let drawer = pressed("drawer.toggle");
        let smaller = pressed("brush.smaller");
        let bigger = pressed("brush.bigger");
        let eyedrop = pressed("tool.eyedrop");
        if let Some(tool) = tool {
            let key = self.keymap.binding(tool.action()).map(|b| b.key);
            if tool == self.prefs.tool && tool != Tool::Select {
                self.set_tool(Tool::Select);
                self.peek = None;
            } else {
                let previous = self.prefs.tool;
                self.set_tool(tool);
                self.peek = key.map(|key| (previous, key, Instant::now()));
            }
        }
        if quick {
            self.quick_open = match self.quick_open {
                Some(_) => None,
                None => Some(
                    ctx.input(|i| i.pointer.hover_pos())
                        .unwrap_or_else(|| ctx.content_rect().center()),
                ),
            };
        }
        if drawer && self.prefs.tool.has_drawer() {
            self.prefs.drawer_open = !self.prefs.drawer_open;
        }
        if smaller {
            self.prefs.brush.radius = self.prefs.brush.radius.saturating_sub(1);
        }
        if bigger {
            self.prefs.brush.radius = (self.prefs.brush.radius + 1).min(16);
        }
        if eyedrop
            && let Some(tile) = self.hover.tile.map(|pick| pick.tile)
            && let Some(values) = self.tile_now(tile)
        {
            self.eyedrop(values);
        }
    }

    /// Esc: back to Select; in Select it clears the selection.
    pub(super) fn escape_action(&mut self) {
        if self.prefs.tool != Tool::Select {
            self.spline.points.clear();
            self.set_tool(Tool::Select);
        } else {
            self.clear_selection();
        }
    }

    /// Brush size and footprint (in the drawer header and the quick HUD). `right_to_left` is for
    /// a right-aligned header, where widgets are added from the right.
    pub(super) fn brush_controls(&mut self, ui: &mut egui::Ui, compact: bool, right_to_left: bool) {
        #[derive(Clone, Copy)]
        enum Piece {
            Label,
            Smaller,
            Size,
            Bigger,
            Footprint(crate::paint::BrushShape),
        }
        let mut pieces = Vec::new();
        if !compact {
            pieces.push(Piece::Label);
        }
        pieces.extend([
            Piece::Smaller,
            Piece::Size,
            Piece::Bigger,
            Piece::Footprint(crate::paint::BrushShape::Square),
            Piece::Footprint(crate::paint::BrushShape::Circle),
        ]);
        if right_to_left {
            pieces.reverse();
        }
        let brush = &mut self.prefs.brush;
        for piece in pieces {
            match piece {
                Piece::Label => {
                    ui.weak("Brush");
                }
                Piece::Smaller => {
                    if ui
                        .small_button("\u{2212}")
                        .on_hover_text("Smaller ([)")
                        .clicked()
                    {
                        brush.radius = brush.radius.saturating_sub(1);
                    }
                }
                Piece::Size => {
                    let side = u32::from(brush.radius) * 2 + 1;
                    ui.label(format!("{side}\u{d7}{side}"))
                        .on_hover_text("Brush footprint in tiles");
                }
                Piece::Bigger => {
                    if ui.small_button("+").on_hover_text("Bigger (])").clicked() {
                        brush.radius = (brush.radius + 1).min(16);
                    }
                }
                Piece::Footprint(shape) => {
                    let (icon, tip) = match shape {
                        crate::paint::BrushShape::Square => {
                            (icons::BRUSH_SQUARE, "Square footprint")
                        }
                        crate::paint::BrushShape::Circle => {
                            (icons::BRUSH_CIRCLE, "Round footprint")
                        }
                    };
                    if ui
                        .selectable_label(brush.footprint == shape, icon)
                        .on_hover_text(tip)
                        .clicked()
                    {
                        brush.footprint = shape;
                    }
                }
            }
        }
    }
}

fn quick_tool_button(
    ui: &mut egui::Ui,
    tool: Tool,
    selected: bool,
    key: Option<String>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(60.0, 58.0), Sense::click());
    let visuals = ui.visuals();
    let fill = if selected {
        visuals.selection.bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        visuals.widgets.inactive.bg_fill
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    let tint = if selected {
        visuals.selection.stroke.color
    } else {
        visuals.text_color()
    };
    ui.painter().text(
        rect.center() - Vec2::new(0.0, 8.0),
        egui::Align2::CENTER_CENTER,
        tool_icon(tool),
        egui::FontId::proportional(21.0),
        tint,
    );
    ui.painter().text(
        rect.center() + Vec2::new(0.0, 14.0),
        egui::Align2::CENTER_CENTER,
        tool.label(),
        egui::FontId::proportional(12.0),
        tint,
    );
    if let Some(key) = key {
        ui.painter().text(
            rect.right_top() + Vec2::new(-6.0, 7.0),
            egui::Align2::RIGHT_CENTER,
            key,
            egui::FontId::monospace(10.0),
            visuals.widgets.noninteractive.fg_stroke.color,
        );
    }
    response
}
