//! The workspace chrome around the viewport (the Displee-style layout):
//!
//! * a **rail** of category icons on the dock edge; each swaps the dock panel's contents,
//! * the **dock panel** (inspector, view, graphics, plugins),
//! * a bottom **tool strip** (one active tool) and a **drawer** holding that tool's palette: the
//!   tile painter's channel tabs, each armed or not,
//! * the **quick tools** HUD (Space) and the **tile info** HUD.
//!
//! Everything is context aware: the drawer exists only for tools with a palette, the brush
//! controls only for tools that brush, the tile HUD only over terrain, and the quick HUD offers
//! what the active tool can use.

use super::EditorApp;
use crate::icons;
use crate::paint::{
    Brush, BrushShape, DrawerTab, FLAG_BITS, Fill, FlagMode, HeightMode, MAX_SHAPE, TileValues,
    Tool,
};
use crate::settings::{InspectTab, RailTab};
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use osrs_core::coords::SceneTile;
use osrs_scene::terrain::{
    ShapedTerrainInput, ShapedTerrainSurface, TerrainColorSource, TerrainCorners,
};
use osrs_world::{OverlaySwatch, UnderlaySwatch};

/// Window width (points) under which labels collapse to icons and the dock narrows.
pub const COMPACT_WIDTH: f32 = 1100.0;
const RAIL_WIDTH: f32 = 44.0;

fn color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// The tool's icon.
pub fn tool_icon(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => icons::TOOL_SELECT,
        Tool::Paint => icons::TOOL_PAINT,
        Tool::Height => icons::TOOL_HEIGHT,
        Tool::Flags => icons::TOOL_FLAGS,
    }
}

fn rail_icon(tab: RailTab) -> &'static str {
    match tab {
        RailTab::Inspect => icons::INSPECT,
        RailTab::View => icons::VIEW,
        RailTab::Graphics => icons::GRAPHICS,
        RailTab::Plugins => icons::PLUGINS,
    }
}

/// Polygons of one overlay shape on a unit square (`y` up), with whether each belongs to the overlay
/// (otherwise the underlay). Built from the real terrain tables so the thumbnails cannot drift.
fn shape_polygons(shape: u8, rotation: u8) -> Vec<([[f32; 2]; 3], bool)> {
    let tile = SceneTile::new(0, 0);
    let flat = TerrainCorners::new(0, 0, 0, 0);
    let Ok(surface) = ShapedTerrainSurface::build(ShapedTerrainInput {
        shape,
        rotation: rotation & 3,
        texture_id: None,
        tile,
        heights: flat,
        underlay_colors: flat,
        overlay_colors: flat,
        underlay_rgb: 0,
        overlay_rgb: 0,
    }) else {
        return Vec::new();
    };
    let point = |index: usize| {
        let position = surface.vertices[index].position;
        [
            position.x.units() as f32 / 128.0,
            position.z.units() as f32 / 128.0,
        ]
    };
    surface
        .faces
        .iter()
        .map(|face| {
            (
                [
                    point(face.indices[0]),
                    point(face.indices[1]),
                    point(face.indices[2]),
                ],
                face.color_source == TerrainColorSource::Overlay,
            )
        })
        .collect()
}

/// Draw one tile: the underlay everywhere, the overlay in the shape's overlay faces.
fn paint_tile(
    painter: &egui::Painter,
    rect: Rect,
    shape: u8,
    rotation: u8,
    underlay: Option<Color32>,
    overlay: Option<Color32>,
) {
    let under = underlay.unwrap_or(Color32::from_gray(70));
    painter.rect_filled(rect, 3.0, under);
    let Some(over) = overlay else {
        return;
    };
    let at = |p: [f32; 2]| {
        Pos2::new(
            rect.left() + p[0] * rect.width(),
            rect.bottom() - p[1] * rect.height(),
        )
    };
    if shape == 0 {
        painter.rect_filled(rect, 3.0, over);
        return;
    }
    for (triangle, is_overlay) in shape_polygons(shape, rotation) {
        if is_overlay {
            painter.add(egui::Shape::convex_polygon(
                triangle.iter().map(|p| at(*p)).collect(),
                over,
                Stroke::NONE,
            ));
        }
    }
}

/// A square swatch button; returns its response.
fn swatch(
    ui: &mut egui::Ui,
    size: f32,
    selected: bool,
    fill: impl FnOnce(&egui::Painter, Rect),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if ui.is_rect_visible(rect) {
        fill(ui.painter(), rect);
        let accent = ui.visuals().selection.stroke.color;
        if selected {
            ui.painter().rect_stroke(
                rect.expand(1.0),
                4.0,
                Stroke::new(2.0, accent),
                StrokeKind::Outside,
            );
        } else if response.hovered() {
            ui.painter().rect_stroke(
                rect,
                3.0,
                Stroke::new(1.0, Color32::from_white_alpha(140)),
                StrokeKind::Inside,
            );
        }
    }
    response
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

    fn underlay_color(&self, id: u16) -> Option<Color32> {
        let palette = self.palette.as_ref()?;
        palette
            .underlays
            .binary_search_by_key(&id, |swatch| swatch.id)
            .ok()
            .map(|index| color(palette.underlays[index].rgb))
    }

    fn overlay_color(&self, id: u16) -> Option<Color32> {
        let palette = self.palette.as_ref()?;
        palette
            .overlays
            .binary_search_by_key(&id, |swatch| swatch.id)
            .ok()
            .map(|index| color(palette.overlays[index].rgb))
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
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(self.prefs.rail_tab.label())
                            .strong()
                            .size(15.0),
                    );
                    if self.prefs.rail_tab == RailTab::Inspect {
                        ui.add_space(8.0);
                        for (tab, label) in
                            [(InspectTab::Object, "Object"), (InspectTab::Tile, "Tile")]
                        {
                            if tab_label(ui, self.prefs.inspect_tab == tab, label).clicked() {
                                self.prefs.inspect_tab = tab;
                            }
                        }
                    }
                });
                ui.add_space(2.0);
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.prefs.rail_tab {
                        RailTab::Inspect => match self.prefs.inspect_tab {
                            InspectTab::Object => self.selection_section(ui),
                            InspectTab::Tile => self.tile_section(ui),
                        },
                        RailTab::View => {
                            self.quick_settings_ui(ui, &["Camera", "World", "Interface"])
                        }
                        RailTab::Graphics => self.quick_settings_ui(ui, &["Graphics"]),
                        RailTab::Plugins => self.plugin_sections(ui),
                    });
            });
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

    fn plugin_sections(&mut self, ui: &mut egui::Ui) {
        let mut any = false;
        self.run_plugins(|plugin, cx| {
            if plugin.has_properties() {
                any = true;
                crate::theme::section_heading(ui, plugin.title());
                plugin.properties(ui, cx);
            }
        });
        if !any {
            ui.weak("No plugin has options to show.");
        }
    }

    /// The hovered tile (or the last one hovered) as a card, with a way to take its values.
    fn tile_section(&mut self, ui: &mut egui::Ui) {
        let Some(tile) = self
            .hover
            .tile
            .map(|pick| pick.tile)
            .or(self.last_hover_tile)
        else {
            empty_state(
                ui,
                icons::TOOL_PAINT,
                "No tile",
                "Move the cursor over the terrain.",
            );
            return;
        };
        let Some(values) = crate::pick::tile_values(&self.infos, tile, self.view_plane) else {
            empty_state(
                ui,
                icons::TOOL_PAINT,
                "No tile data",
                "That area is not loaded.",
            );
            return;
        };
        ui.label(egui::RichText::new(format!("Tile {}, {}", tile.0, tile.1)).heading());
        ui.weak(format!(
            "plane {}  \u{b7}  region {}",
            self.view_plane,
            crate::hud::region_id(tile.0 >> 6, tile.1 >> 6)
        ));
        crate::theme::section_heading(ui, "Terrain");
        egui::Grid::new("tile_fields")
            .num_columns(2)
            .spacing([14.0, 5.0])
            .show(ui, |ui| {
                let row = |ui: &mut egui::Ui, label: &str, value: String| {
                    ui.weak(label);
                    ui.label(value);
                    ui.end_row();
                };
                row(
                    ui,
                    "Overlay",
                    if values.overlay == 0 {
                        "none".into()
                    } else {
                        values.overlay.to_string()
                    },
                );
                row(
                    ui,
                    "Underlay",
                    if values.underlay == 0 {
                        "none".into()
                    } else {
                        values.underlay.to_string()
                    },
                );
                row(
                    ui,
                    "Shape",
                    format!("{} \u{b7} rotation {}", values.shape, values.rotation),
                );
                row(ui, "Height", values.height.to_string());
                row(ui, "Flags", format!("{:#04x}", values.flags));
            });
        ui.add_space(8.0);
        if ui
            .button(format!("{}  Use as brush", icons::EYEDROPPER))
            .on_hover_text("Copy this tile's values into the brush (I)")
            .clicked()
        {
            self.eyedrop(values);
        }
    }

    /// Fill the brush from a tile and switch to the painter.
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
            std::time::Instant::now(),
        ));
    }

    pub(super) fn set_tool(&mut self, tool: Tool) {
        self.prefs.tool = tool;
        if let Some(tab) = tool.drawer_tab() {
            let tabs = DrawerTab::for_tool(tool);
            if !tabs.contains(&self.prefs.drawer_tab) {
                self.prefs.drawer_tab = tab;
            }
            self.prefs.drawer_open = true;
        }
    }

    // ------------------------------------------------------------- tool strip

    /// The tool strip (always) and the drawer above it (when the tool has a palette).
    pub(super) fn tool_bars(&mut self, ui: &mut egui::Ui) {
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
                    for tool in Tool::ALL {
                        let selected = self.prefs.tool == tool;
                        let text = if compact {
                            tool_icon(tool).to_owned()
                        } else {
                            format!("{}  {}", tool_icon(tool), tool.label())
                        };
                        let key = self.keymap.binding(tool.action()).map(|b| b.describe());
                        let hint = key.map_or_else(
                            || tool.label().to_owned(),
                            |key| format!("{} ({key})", tool.label()),
                        );
                        if ui
                            .selectable_label(selected, text)
                            .on_hover_text(hint)
                            .clicked()
                        {
                            self.set_tool(tool);
                        }
                    }
                    if self.prefs.tool != Tool::Select {
                        ui.separator();
                        self.brush_controls(ui, compact);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.prefs.tool.has_drawer() {
                            let (icon, tip) = if self.prefs.drawer_open {
                                (egui_phosphor::regular::CARET_DOWN, "Hide the drawer (Tab)")
                            } else {
                                (egui_phosphor::regular::CARET_UP, "Show the drawer (Tab)")
                            };
                            if ui.small_button(icon).on_hover_text(tip).clicked() {
                                self.prefs.drawer_open = !self.prefs.drawer_open;
                            }
                        }
                        if !compact {
                            crate::theme::key_hint(ui, "Space", "quick tools");
                        }
                    });
                });
            });

        if self.prefs.tool.has_drawer() && self.prefs.drawer_open {
            egui::Panel::bottom("tool_drawer")
                .resizable(true)
                .default_size((ui.ctx().content_rect().height() * 0.26).clamp(140.0, 196.0))
                .size_range(130.0..=360.0)
                .frame(
                    egui::Frame::new()
                        .fill(ui.visuals().panel_fill)
                        .stroke(Stroke::new(1.0, line))
                        .inner_margin(egui::Margin::symmetric(10, 6)),
                )
                .show(ui, |ui| self.drawer(ui));
        }
    }

    /// Brush size and footprint, shown for the tools that brush.
    fn brush_controls(&mut self, ui: &mut egui::Ui, compact: bool) {
        let brush = &mut self.prefs.brush;
        if !compact {
            ui.weak("Brush");
        }
        if ui
            .small_button("\u{2212}")
            .on_hover_text("Smaller ([)")
            .clicked()
        {
            brush.radius = brush.radius.saturating_sub(1);
        }
        ui.label(format!(
            "{}\u{d7}{}",
            u32::from(brush.radius) * 2 + 1,
            u32::from(brush.radius) * 2 + 1
        ))
        .on_hover_text("Brush footprint in tiles");
        if ui.small_button("+").on_hover_text("Bigger (])").clicked() {
            brush.radius = (brush.radius + 1).min(16);
        }
        for (shape, icon, tip) in [
            (BrushShape::Square, icons::BRUSH_SQUARE, "Square footprint"),
            (BrushShape::Circle, icons::BRUSH_CIRCLE, "Round footprint"),
        ] {
            if ui
                .selectable_label(brush.footprint == shape, icon)
                .on_hover_text(tip)
                .clicked()
            {
                brush.footprint = shape;
            }
        }
    }

    // ----------------------------------------------------------------- drawer

    fn drawer(&mut self, ui: &mut egui::Ui) {
        let tool = self.prefs.tool;
        let tabs = DrawerTab::for_tool(tool);
        if !tabs.contains(&self.prefs.drawer_tab)
            && let Some(first) = tabs.first()
        {
            self.prefs.drawer_tab = *first;
        }
        // Header: channel tabs (a checkbox arms the channel), then what the brush will do.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for tab in tabs {
                let armed = self.prefs.brush.is_armed(*tab);
                let selected = self.prefs.drawer_tab == *tab;
                if channel_tab(
                    ui,
                    selected,
                    armed,
                    tab.label(),
                    &mut self.prefs.brush,
                    *tab,
                ) {
                    self.prefs.drawer_tab = *tab;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(format!("{}  Eyedropper", icons::EYEDROPPER))
                    .on_hover_text("Hover a tile and press I, or use the tile's context menu")
                    .clicked()
                {
                    self.notice = Some((
                        "Hover a tile and press I (or Shift + right click > Eyedrop tile)"
                            .to_owned(),
                        std::time::Instant::now(),
                    ));
                }
                let armed = self.prefs.brush.armed_channels();
                let summary = if armed.is_empty() {
                    "Nothing armed: the brush changes nothing".to_owned()
                } else {
                    format!(
                        "Paints {}  \u{b7}  keeps the rest",
                        armed.join(", ").to_lowercase()
                    )
                };
                ui.label(
                    egui::RichText::new(summary)
                        .size(12.0)
                        .color(if armed.is_empty() {
                            ui.visuals().warn_fg_color
                        } else {
                            ui.visuals().widgets.noninteractive.fg_stroke.color
                        }),
                );
            });
        });
        ui.add_space(4.0);
        ui.horizontal_top(|ui| {
            self.drawer_preview(ui);
            ui.separator();
            ui.vertical(|ui| {
                ui.set_width((ui.available_width()).max(200.0));
                match self.prefs.drawer_tab {
                    DrawerTab::Overlay => self.overlay_palette(ui),
                    DrawerTab::Underlay => self.underlay_palette(ui),
                    DrawerTab::Shape => self.shape_picker(ui),
                    DrawerTab::Height => self.height_options(ui),
                    DrawerTab::Mask => self.flag_options(ui),
                }
            });
        });
    }

    /// What one painted tile will look like.
    fn drawer_preview(&mut self, ui: &mut egui::Ui) {
        let brush = &self.prefs.brush;
        let underlay = match (brush.underlay.armed, brush.underlay.fill) {
            (true, Fill::Set(id)) => self.underlay_color(id),
            _ => None,
        };
        let (shape, rotation) = if brush.shape.armed {
            (brush.shape.shape, brush.shape.rotation)
        } else {
            (0, 0)
        };
        let overlay = match (brush.overlay.armed, brush.overlay.fill) {
            (true, Fill::Set(id)) => self.overlay_color(id),
            _ => None,
        };
        ui.vertical(|ui| {
            ui.set_width(76.0);
            ui.weak("Preview");
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(72.0), Sense::hover());
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
            response.on_hover_text("Preview of the armed colour channels on a tile");
        });
    }

    fn palette_loading(&self, ui: &mut egui::Ui) -> bool {
        if self.palette.is_some() {
            return false;
        }
        ui.horizontal(|ui| {
            if self.palette_rx.is_some() {
                ui.spinner();
                ui.weak("Loading floor colours...");
            } else {
                ui.weak("Floor colours could not be read from the cache.");
            }
        });
        true
    }

    fn overlay_palette(&mut self, ui: &mut egui::Ui) {
        if self.palette_loading(ui) {
            return;
        }
        let Some(palette) = self.palette.clone() else {
            return;
        };
        let current = self.prefs.brush.overlay.fill;
        let mut picked = None;
        ui.horizontal(|ui| {
            ui.weak(match current {
                Fill::Set(id) => format!("Overlay {id}"),
                Fill::Clear => "No overlay (clears it)".to_owned(),
            });
        });
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                    if swatch(ui, 26.0, current == Fill::Clear, |painter, rect| {
                        painter.rect_filled(rect, 3.0, Color32::from_gray(38));
                        painter.line_segment(
                            [rect.left_bottom(), rect.right_top()],
                            Stroke::new(1.5, Color32::from_rgb(220, 90, 90)),
                        );
                    })
                    .on_hover_text("No overlay: clears the tile's overlay, shape and rotation")
                    .clicked()
                    {
                        picked = Some(Fill::Clear);
                    }
                    // Placeholder overlays (pure magenta, no texture) are drawn dim and last.
                    let placeholder = |swatch: &&OverlaySwatch| {
                        swatch.rgb == [255, 0, 255] && swatch.texture.is_none()
                    };
                    let real = palette
                        .overlays
                        .iter()
                        .filter(|swatch| !placeholder(swatch));
                    let unused = palette.overlays.iter().filter(placeholder);
                    for OverlaySwatch {
                        id,
                        rgb,
                        texture,
                        hide_underlay,
                    } in real.chain(unused)
                    {
                        let rgb = &if *rgb == [255, 0, 255] && texture.is_none() {
                            [64, 48, 72]
                        } else {
                            *rgb
                        };
                        let tip = match texture {
                            Some(texture) => format!("Overlay {id}  \u{b7}  texture {texture}"),
                            None if *rgb == [64, 48, 72] => {
                                format!("Overlay {id}  \u{b7}  placeholder (no colour)")
                            }
                            None => format!("Overlay {id}"),
                        };
                        if swatch(ui, 26.0, current == Fill::Set(*id), |painter, rect| {
                            painter.rect_filled(rect, 3.0, color(*rgb));
                            if texture.is_some() {
                                painter.text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "\u{25a6}",
                                    egui::FontId::proportional(12.0),
                                    Color32::from_black_alpha(110),
                                );
                            }
                            if *hide_underlay {
                                painter.circle_filled(
                                    rect.right_top() + Vec2::new(-4.0, 4.0),
                                    2.0,
                                    Color32::from_white_alpha(200),
                                );
                            }
                        })
                        .on_hover_text(tip)
                        .clicked()
                        {
                            picked = Some(Fill::Set(*id));
                        }
                    }
                });
            });
        if let Some(fill) = picked {
            self.prefs.brush.overlay.fill = fill;
            self.prefs.brush.overlay.armed = true;
        }
    }

    fn underlay_palette(&mut self, ui: &mut egui::Ui) {
        if self.palette_loading(ui) {
            return;
        }
        let Some(palette) = self.palette.clone() else {
            return;
        };
        let current = self.prefs.brush.underlay.fill;
        let mut picked = None;
        ui.weak(match current {
            Fill::Set(id) => format!("Underlay {id}"),
            Fill::Clear => "No underlay (clears it)".to_owned(),
        });
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                    if swatch(ui, 26.0, current == Fill::Clear, |painter, rect| {
                        painter.rect_filled(rect, 3.0, Color32::from_gray(38));
                        painter.line_segment(
                            [rect.left_bottom(), rect.right_top()],
                            Stroke::new(1.5, Color32::from_rgb(220, 90, 90)),
                        );
                    })
                    .on_hover_text("No underlay: the tile is hidden unless it has an overlay")
                    .clicked()
                    {
                        picked = Some(Fill::Clear);
                    }
                    for UnderlaySwatch { id, rgb } in &palette.underlays {
                        if swatch(ui, 26.0, current == Fill::Set(*id), |painter, rect| {
                            painter.rect_filled(rect, 3.0, color(*rgb));
                        })
                        .on_hover_text(format!("Underlay {id}"))
                        .clicked()
                        {
                            picked = Some(Fill::Set(*id));
                        }
                    }
                });
            });
        if let Some(fill) = picked {
            self.prefs.brush.underlay.fill = fill;
            self.prefs.brush.underlay.armed = true;
        }
    }

    fn shape_picker(&mut self, ui: &mut egui::Ui) {
        // Fixed, contrasting colours: the thumbnails show geometry, not the chosen floor.
        let under = Color32::from_rgb(58, 98, 66);
        let over = Color32::from_rgb(214, 196, 150);
        let (mut shape, mut rotation) = (
            self.prefs.brush.shape.shape,
            self.prefs.brush.shape.rotation,
        );
        let mut picked = false;
        egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.weak("Shape");
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::splat(5.0);
                            ui.set_max_width(330.0);
                            for id in 0..=MAX_SHAPE {
                                if swatch(ui, 40.0, shape == id, |painter, rect| {
                                    paint_tile(
                                        painter,
                                        rect,
                                        id,
                                        rotation,
                                        Some(under),
                                        Some(over),
                                    );
                                })
                                .on_hover_text(if id == 0 {
                                    "Shape 0: full tile".to_owned()
                                } else {
                                    format!("Shape {id}")
                                })
                                .clicked()
                                {
                                    shape = id;
                                    picked = true;
                                }
                            }
                        });
                    });
                    ui.add_space(16.0);
                    ui.vertical(|ui| {
                        ui.weak("Rotation");
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::splat(5.0);
                            for (index, name) in
                                ["North", "East", "South", "West"].into_iter().enumerate()
                            {
                                let index = index as u8;
                                ui.vertical(|ui| {
                                    if swatch(ui, 40.0, rotation == index, |painter, rect| {
                                        paint_tile(
                                            painter,
                                            rect,
                                            if shape == 0 { 3 } else { shape },
                                            index,
                                            Some(under),
                                            Some(over),
                                        );
                                    })
                                    .clicked()
                                    {
                                        rotation = index;
                                        picked = true;
                                    }
                                    ui.label(egui::RichText::new(name).size(11.0));
                                });
                            }
                        });
                    });
                });
            });
        self.prefs.brush.shape.shape = shape;
        self.prefs.brush.shape.rotation = rotation;
        if picked {
            self.prefs.brush.shape.armed = true;
        }
    }

    fn height_options(&mut self, ui: &mut egui::Ui) {
        let height = &mut self.prefs.brush.height;
        ui.weak("How the brush changes tile heights inside its footprint");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for mode in HeightMode::ALL {
                if ui
                    .selectable_label(height.mode == mode, mode.label())
                    .clicked()
                {
                    height.mode = mode;
                    height.armed = true;
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let label = match height.mode {
                HeightMode::Set | HeightMode::Flatten => "Height",
                HeightMode::Raise | HeightMode::Lower => "Step",
                HeightMode::Smooth => "Strength",
            };
            ui.weak(label);
            if ui
                .add(
                    egui::DragValue::new(&mut height.amount)
                        .speed(1.0)
                        .range(-2048..=2048),
                )
                .changed()
            {
                height.armed = true;
            }
            ui.weak("client units (128 = one tile)");
        });
    }

    fn flag_options(&mut self, ui: &mut egui::Ui) {
        let flags = &mut self.prefs.brush.flags;
        ui.horizontal(|ui| {
            ui.weak("Mode");
            for mode in FlagMode::ALL {
                if ui
                    .selectable_label(flags.mode == mode, mode.label())
                    .clicked()
                {
                    flags.mode = mode;
                    flags.armed = true;
                }
            }
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for (bit, name, tip) in FLAG_BITS {
                let mut on = flags.bits & bit != 0;
                if ui.checkbox(&mut on, name).on_hover_text(tip).changed() {
                    flags.bits = if on {
                        flags.bits | bit
                    } else {
                        flags.bits & !bit
                    };
                    flags.armed = true;
                }
            }
        });
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
                                if quick_tool_button(
                                    ui,
                                    tool,
                                    self.prefs.tool == tool,
                                    self.keymap.binding(tool.action()).map(|b| b.describe()),
                                )
                                .clicked()
                                {
                                    self.set_tool(tool);
                                    close = true;
                                }
                            }
                        });
                        if self.prefs.tool == Tool::Paint {
                            crate::theme::section_heading(ui, "Brush paints");
                            ui.horizontal_wrapped(|ui| {
                                for tab in DrawerTab::ALL {
                                    let mut armed = self.prefs.brush.is_armed(tab);
                                    if ui.toggle_value(&mut armed, tab.label()).changed() {
                                        self.prefs.brush.set_armed(tab, armed);
                                    }
                                }
                            });
                        }
                        if self.prefs.tool != Tool::Select {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.weak("Brush");
                                self.brush_controls(ui, true);
                                if let Some(tile) = self.hover.tile.map(|pick| pick.tile)
                                    && let Some(values) =
                                        crate::pick::tile_values(&self.infos, tile, self.view_plane)
                                    && ui
                                        .button(format!("{}  Eyedrop", icons::EYEDROPPER))
                                        .clicked()
                                {
                                    self.eyedrop(values);
                                    close = true;
                                }
                            });
                        }
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

    // ---------------------------------------------------------------- tile HUD

    /// The tile under the cursor, only while the cursor is over terrain.
    pub(super) fn tile_info_lines(&self) -> Option<Vec<(String, String)>> {
        let tile = self.hover.tile.map(|pick| pick.tile)?;
        let values = crate::pick::tile_values(&self.infos, tile, self.view_plane)?;
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
                format!("{}  \u{b7}  flags {:#04x}", values.height, values.flags),
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

    /// Tool, drawer and brush shortcuts.
    pub(super) fn tool_actions(&mut self, ctx: &egui::Context) {
        let pressed = |id: &str| ctx.input(|i| self.keymap.pressed(i, id));
        let tool = Tool::ALL.into_iter().find(|tool| pressed(tool.action()));
        let quick = pressed("quick.tools");
        let drawer = pressed("drawer.toggle");
        let smaller = pressed("brush.smaller");
        let bigger = pressed("brush.bigger");
        let eyedrop = pressed("tool.eyedrop");
        if let Some(tool) = tool {
            self.set_tool(tool);
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
            && let Some(values) = crate::pick::tile_values(&self.infos, tile, self.view_plane)
        {
            self.eyedrop(values);
        }
    }
}

// ---------------------------------------------------------------- small widgets

/// A dock tab: text with an accent underline when selected.
fn tab_label(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        egui::TextStyle::Body.resolve(ui.style()),
        Color32::PLACEHOLDER,
    );
    let size = Vec2::new(galley.size().x + 12.0, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let accent = ui.visuals().selection.stroke.color;
    let tint = if selected {
        ui.visuals().text_color()
    } else if response.hovered() {
        ui.visuals().text_color().gamma_multiply(0.85)
    } else {
        ui.visuals().widgets.noninteractive.fg_stroke.color
    };
    ui.painter().galley(
        Pos2::new(rect.left() + 6.0, rect.center().y - galley.size().y / 2.0),
        galley,
        tint,
    );
    if selected {
        ui.painter().rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left() + 4.0, rect.bottom() - 2.0),
                Pos2::new(rect.right() - 4.0, rect.bottom()),
            ),
            1.0,
            accent,
        );
    }
    response
}

/// A drawer tab with its arm checkbox. Returns `true` when the tab itself was clicked.
fn channel_tab(
    ui: &mut egui::Ui,
    selected: bool,
    armed: bool,
    label: &str,
    brush: &mut Brush,
    tab: DrawerTab,
) -> bool {
    let accent = ui.visuals().selection.stroke.color;
    let fill = if selected {
        ui.visuals().widgets.inactive.bg_fill
    } else {
        Color32::TRANSPARENT
    };
    let mut clicked = false;
    egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius {
            nw: 6,
            ne: 6,
            sw: 0,
            se: 0,
        })
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let mut on = armed;
                if ui
                    .checkbox(&mut on, "")
                    .on_hover_text(if armed {
                        "Armed: the brush sets this. Untick to keep the tile's own value."
                    } else {
                        "Not armed: the tile keeps its own value. Tick to paint this channel."
                    })
                    .changed()
                {
                    brush.set_armed(tab, on);
                }
                let text = egui::RichText::new(label).color(if selected {
                    accent
                } else {
                    ui.visuals().text_color()
                });
                if ui
                    .add(egui::Label::new(text).sense(Sense::click()))
                    .clicked()
                {
                    clicked = true;
                }
            });
        });
    clicked
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

fn empty_state(ui: &mut egui::Ui, icon: &str, title: &str, hint: &str) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(icon)
                .size(28.0)
                .color(ui.visuals().widgets.noninteractive.fg_stroke.color),
        );
        ui.label(egui::RichText::new(title).strong());
        ui.weak(hint);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_and_rotation_has_faces_inside_the_tile() {
        for shape in 1..=MAX_SHAPE {
            for rotation in 0..4 {
                let faces = shape_polygons(shape, rotation);
                assert!(!faces.is_empty(), "shape {shape} rotation {rotation}");
                for (triangle, _) in faces {
                    for point in triangle {
                        assert!((0.0..=1.0).contains(&point[0]) && (0.0..=1.0).contains(&point[1]));
                    }
                }
            }
        }
    }

    #[test]
    fn shaped_tiles_have_both_an_underlay_and_an_overlay_part() {
        for shape in 1..=MAX_SHAPE {
            let faces = shape_polygons(shape, 0);
            assert!(
                faces.iter().any(|(_, overlay)| *overlay),
                "shape {shape} has no overlay face"
            );
        }
    }
}

impl EditorApp {
    /// Apply the brush around `tile`. Until the edit backend lands this reports what it would do.
    pub(super) fn apply_brush_at(&mut self, tile: (i32, i32)) {
        let brush = &self.prefs.brush;
        if brush.is_empty() {
            self.notice = Some((
                "Nothing armed: tick a channel in the drawer".to_owned(),
                std::time::Instant::now(),
            ));
            return;
        }
        let tiles = brush.footprint_tiles(tile);
        self.notice = Some((
            format!(
                "Brush would change {} on {} tile(s)",
                brush.armed_channels().join(", ").to_lowercase(),
                tiles.len()
            ),
            std::time::Instant::now(),
        ));
    }
}
