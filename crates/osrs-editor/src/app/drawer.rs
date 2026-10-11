//! The tool drawer along the bottom: the active tool's palette.
//!
//! * **Paint**: the tile painter. One tab per channel (overlay, underlay, shape, height, mask); the
//!   checkbox on a tab arms the channel, and unarmed channels keep the tile's own value.
//! * **Spline**: the path's width and edges, plus the overlay and underlay it is laid with.
//! * **Height**: sculpting (raise, lower, smooth, flatten, roughen) with strength and falloff.
//!
//! The drawer exists only for tools that have a palette; Select has none.

use super::EditorApp;
use super::widgets::{OverlayFill, chips, paint_tile, swatch};
use crate::heights::{Falloff, HeightOp};
use crate::icons;
use crate::paint::{
    DrawerTab, FLAG_BITS, Fill, MAX_ELEVATION, MAX_SHAPE, PaletteFilter, Tool, elevation,
    height_for_elevation,
};
use eframe::egui::{self, Color32, CornerRadius, Rect, Sense, Stroke, StrokeKind, Vec2};
use osrs_render::TextureAnimationVector;
use osrs_world::{OverlaySwatch, UnderlaySwatch};

const SWATCH: f32 = 38.0;
/// The colour the palettes draw placeholder overlays in (pure magenta without a texture).
const PLACEHOLDER: [u8; 3] = [64, 48, 72];

fn color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

impl EditorApp {
    // ---------------------------------------------------------------- textures

    /// The egui texture of a floor texture (made on first use), with its scroll animation.
    fn floor_texture(
        &mut self,
        ctx: &egui::Context,
        texture: u32,
    ) -> Option<(egui::TextureId, TextureAnimationVector)> {
        if let Some((handle, vector)) = self.texture_handles.get(&texture) {
            return Some((handle.id(), *vector));
        }
        let layer = self
            .floor_textures
            .as_ref()?
            .get(texture as usize)?
            .as_ref()?;
        let pixels: Vec<Color32> = layer
            .rgba
            .iter()
            .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect();
        let size = (layer.rgba.len() as f64).sqrt() as usize;
        let image = egui::ColorImage {
            size: [size, size],
            source_size: Vec2::new(size as f32, size as f32),
            pixels,
        };
        let handle = ctx.load_texture(
            format!("floor-texture-{texture}"),
            image,
            egui::TextureOptions {
                magnification: egui::TextureFilter::Linear,
                minification: egui::TextureFilter::Linear,
                wrap_mode: egui::TextureWrapMode::Repeat,
                mipmap_mode: None,
            },
        );
        let vector = TextureAnimationVector::from_direction_speed(
            layer.animation_direction,
            layer.animation_speed,
        );
        let id = handle.id();
        self.texture_handles.insert(texture, (handle, vector));
        Some((id, vector))
    }

    /// How an overlay swatch is filled: its texture (scrolling when animated), else its colour.
    pub(super) fn overlay_fill(
        &mut self,
        ctx: &egui::Context,
        swatch: &OverlaySwatch,
    ) -> OverlayFill {
        if let Some(texture) = swatch.texture
            && let Some((id, vector)) = self.floor_texture(ctx, texture)
        {
            let tick = (self.started.elapsed().as_secs_f32() * 50.0) as u32;
            let (u, v) = vector.uv_offset_at_tick(tick);
            return OverlayFill::Texture {
                id,
                offset: (u.rem_euclid(1.0), v.rem_euclid(1.0)),
            };
        }
        if swatch.rgb == [255, 0, 255] && swatch.texture.is_none() {
            return OverlayFill::Color(color(PLACEHOLDER));
        }
        OverlayFill::Color(color(swatch.rgb))
    }

    fn overlay_swatch(&self, id: u16) -> Option<OverlaySwatch> {
        let palette = self.palette.as_ref()?;
        let index = palette
            .overlays
            .binary_search_by_key(&id, |swatch| swatch.id)
            .ok()?;
        Some(palette.overlays[index])
    }

    fn underlay_color(&self, id: u16) -> Option<Color32> {
        let palette = self.palette.as_ref()?;
        let index = palette
            .underlays
            .binary_search_by_key(&id, |swatch| swatch.id)
            .ok()?;
        Some(color(palette.underlays[index].rgb))
    }

    // ------------------------------------------------------------------ drawer

    /// The drawer panel (only for tools with a palette).
    pub(super) fn tool_drawer(&mut self, ui: &mut egui::Ui) {
        if !(self.prefs.tool.has_drawer() && self.prefs.drawer_open) {
            return;
        }
        let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
        egui::Panel::bottom("tool_drawer")
            .resizable(true)
            .default_size((ui.ctx().content_rect().height() * 0.27).clamp(150.0, 210.0))
            .size_range(130.0..=380.0)
            .frame(
                egui::Frame::new()
                    .fill(ui.visuals().panel_fill)
                    .stroke(Stroke::new(1.0, line))
                    .inner_margin(egui::Margin::symmetric(10, 6)),
            )
            .show(ui, |ui| self.drawer(ui));
    }

    fn drawer(&mut self, ui: &mut egui::Ui) {
        let tool = self.prefs.tool;
        let tabs = tool.tabs();
        if !tabs.contains(&self.prefs.drawer_tab)
            && let Some(first) = tabs.first()
        {
            self.prefs.drawer_tab = *first;
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for tab in tabs {
                let with_checkbox = tab.is_channel() && tool != Tool::Height;
                let selected = self.prefs.drawer_tab == *tab;
                if channel_tab(ui, selected, with_checkbox, *tab, self) {
                    self.prefs.drawer_tab = *tab;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button(egui_phosphor::regular::CARET_DOWN)
                    .on_hover_text("Hide the drawer (Tab)")
                    .clicked()
                {
                    self.prefs.drawer_open = false;
                }
                if tool.uses_brush() {
                    self.brush_controls(ui, false, true);
                }
                let selected = self.selection_tiles.len();
                if tool == Tool::Paint && selected > 0 && !self.prefs.brush.is_empty() {
                    ui.add_space(8.0);
                    if ui
                        .button(format!("{}  Fill {selected} selected", icons::SELECT_TILES))
                        .on_hover_text("Apply the brush to every selected tile")
                        .clicked()
                    {
                        self.fill_selection();
                    }
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal_top(|ui| {
            if matches!(tool, Tool::Paint | Tool::Spline) {
                self.drawer_preview(ui);
                ui.separator();
            }
            ui.vertical(|ui| {
                ui.set_width(ui.available_width().max(200.0));
                match (tool, self.prefs.drawer_tab) {
                    (Tool::Height, _) => self.height_tool_options(ui),
                    (_, DrawerTab::Overlay) => self.overlay_palette(ui),
                    (_, DrawerTab::Underlay) => self.underlay_palette(ui),
                    (_, DrawerTab::Shape) => self.shape_picker(ui),
                    (_, DrawerTab::Height) => self.height_channel(ui),
                    (_, DrawerTab::Mask) => self.flag_options(ui),
                    (_, DrawerTab::Path) => self.path_options(ui),
                }
            });
        });
    }

    /// What one painted tile will look like.
    fn drawer_preview(&mut self, ui: &mut egui::Ui) {
        let brush = self.prefs.brush.clone();
        let underlay = match (brush.underlay.armed, brush.underlay.fill) {
            (true, Fill::Set(id)) => self.underlay_color(id),
            _ => None,
        };
        let (shape, rotation) = if brush.shape.armed && self.prefs.tool == Tool::Paint {
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
            response.on_hover_text("The armed colour channels on one tile");
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

    /// The filter chips, search box and a count; returns the search text.
    fn palette_header(&mut self, ui: &mut egui::Ui, shown: usize, textured: bool) -> String {
        ui.horizontal(|ui| {
            for filter in PaletteFilter::ALL {
                if filter == PaletteFilter::Textured && !textured {
                    continue;
                }
                if ui
                    .selectable_label(self.prefs.palette_filter == filter, filter.label())
                    .on_hover_text(match filter {
                        PaletteFilter::Common => "Floors that many tiles of the real map use",
                        PaletteFilter::AllUsed => "Every floor any tile of the real map uses",
                        PaletteFilter::Textured => "Overlays with a texture",
                    })
                    .clicked()
                {
                    self.prefs.palette_filter = filter;
                }
            }
            ui.add_space(6.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.palette_search)
                    .hint_text("id")
                    .desired_width(54.0),
            );
            ui.weak(format!("{shown} shown"));
        });
        self.palette_search.trim().to_owned()
    }

    fn overlay_palette(&mut self, ui: &mut egui::Ui) {
        if self.palette_loading(ui) {
            return;
        }
        let Some(palette) = self.palette.clone() else {
            return;
        };
        let filter = self.prefs.palette_filter;
        let search = self.palette_search.trim().to_owned();
        // Placeholder overlays (pure magenta, no texture) are the map's "draw nothing" marker; only
        // "All used" lists them.
        let placeholder = |s: &OverlaySwatch| s.rgb == [255, 0, 255] && s.texture.is_none();
        let mut swatches: Vec<OverlaySwatch> = palette
            .overlays
            .iter()
            .filter(|s| filter == PaletteFilter::AllUsed || !placeholder(s))
            .filter(|s| filter.keeps(s.uses, s.texture.is_some()))
            .filter(|s| search.is_empty() || s.id.to_string().contains(&search))
            .copied()
            .collect();
        swatches.sort_by_key(|s| (std::cmp::Reverse(s.uses), s.id));
        self.palette_header(ui, swatches.len(), true);
        let current = self.prefs.brush.overlay.fill;
        let mut picked = None;
        let ctx = ui.ctx().clone();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(5.0);
                    if none_swatch(ui, current == Fill::Clear)
                        .on_hover_text("No overlay: clears the overlay, shape and rotation")
                        .clicked()
                    {
                        picked = Some(Fill::Clear);
                    }
                    for sw in &swatches {
                        let fill = self.overlay_fill(&ctx, sw);
                        let tip = overlay_tip(sw);
                        let hides = sw.hide_underlay;
                        if swatch(ui, SWATCH, current == Fill::Set(sw.id), |painter, rect| {
                            paint_overlay_swatch(painter, rect, fill, hides);
                        })
                        .on_hover_text(tip)
                        .clicked()
                        {
                            picked = Some(Fill::Set(sw.id));
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
        let filter = match self.prefs.palette_filter {
            PaletteFilter::Textured => PaletteFilter::AllUsed,
            other => other,
        };
        let search = self.palette_search.trim().to_owned();
        let mut swatches: Vec<UnderlaySwatch> = palette
            .underlays
            .iter()
            .filter(|s| filter.keeps(s.uses, false))
            .filter(|s| search.is_empty() || s.id.to_string().contains(&search))
            .copied()
            .collect();
        swatches.sort_by_key(|s| (std::cmp::Reverse(s.uses), s.id));
        self.palette_header(ui, swatches.len(), false);
        let current = self.prefs.brush.underlay.fill;
        let mut picked = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(5.0);
                    if none_swatch(ui, current == Fill::Clear)
                        .on_hover_text("No underlay: the tile is hidden unless it has an overlay")
                        .clicked()
                    {
                        picked = Some(Fill::Clear);
                    }
                    for sw in &swatches {
                        if swatch(ui, SWATCH, current == Fill::Set(sw.id), |painter, rect| {
                            painter.rect_filled(rect, 3.0, color(sw.rgb));
                        })
                        .on_hover_text(format!("Underlay {}  \u{b7}  {} tiles", sw.id, sw.uses))
                        .clicked()
                        {
                            picked = Some(Fill::Set(sw.id));
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
        let over = OverlayFill::Color(Color32::from_rgb(214, 196, 150));
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

    /// The Paint tool's height channel: one height for every tile the brush paints.
    fn height_channel(&mut self, ui: &mut egui::Ui) {
        let channel = &mut self.prefs.brush.height;
        let mut value = elevation(channel.value);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.weak("Height");
            let slider = ui.add(
                egui::Slider::new(&mut value, 0..=MAX_ELEVATION)
                    .step_by(8.0)
                    .show_value(false),
            );
            let field = ui.add(
                egui::DragValue::new(&mut value)
                    .range(0..=MAX_ELEVATION)
                    .speed(1.0),
            );
            if slider.changed() || field.changed() {
                channel.value = height_for_elevation(value);
                channel.armed = true;
            }
            if ui.button("Sea level").clicked() {
                channel.value = 0;
                channel.armed = true;
            }
        });
        ui.add_space(4.0);
        ui.weak("Every tile the brush paints gets this height. Untick the channel to keep each tile's own.");
    }

    /// The Paint tool's mask channel: the flags every painted tile gets.
    fn flag_options(&mut self, ui: &mut egui::Ui) {
        let flags = &mut self.prefs.brush.flags;
        ui.add_space(6.0);
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
        ui.add_space(4.0);
        ui.weak("Every tile the brush paints gets exactly these flags. Untick the channel to keep each tile's own.");
    }

    /// The spline tool's path options.
    fn path_options(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.weak("Width");
            ui.add(egui::Slider::new(&mut self.prefs.spline.width, 1.0..=24.0).show_value(false));
            ui.add(
                egui::DragValue::new(&mut self.prefs.spline.width)
                    .range(1.0..=32.0)
                    .speed(0.1)
                    .max_decimals(1)
                    .suffix(" tiles"),
            );
        });
        ui.horizontal(|ui| {
            ui.weak("Edges");
            if let Some(hard) = chips(
                ui,
                self.prefs.spline.hard_edges,
                &[(false, "Smooth shapes"), (true, "Whole tiles")],
            ) {
                self.prefs.spline.hard_edges = hard;
            }
        });
        let points = self.spline.points.len();
        let tiles = self.spline_tile_count();
        ui.horizontal(|ui| {
            ui.weak(format!("{points} points  \u{b7}  {tiles} tiles"));
            if ui
                .add_enabled(points >= 1, egui::Button::new("Lay path"))
                .on_hover_text("Enter")
                .clicked()
            {
                self.commit_spline();
            }
            if ui
                .add_enabled(points >= 1, egui::Button::new("Clear"))
                .on_hover_text("Esc")
                .clicked()
            {
                self.spline.points.clear();
            }
        });
        ui.add_space(2.0);
        ui.weak("Click the terrain to add points. Drag a point to move it, Cmd + click removes it. The auto-tiler picks every tile's shape and rotation from the overlay tab's floor.");
    }

    /// The Height tool's options.
    fn height_tool_options(&mut self, ui: &mut egui::Ui) {
        let mut tool = self.prefs.height_tool.clone();
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            for op in HeightOp::ALL {
                if ui
                    .selectable_label(tool.op == op, op.label())
                    .on_hover_text(op.hint())
                    .clicked()
                {
                    tool.op = op;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.weak(if tool.op == HeightOp::Smooth {
                "Strength"
            } else {
                "Step"
            });
            ui.add(
                egui::Slider::new(&mut tool.strength, 8..=128)
                    .step_by(8.0)
                    .show_value(false),
            );
            ui.add(
                egui::DragValue::new(&mut tool.strength)
                    .range(8..=128)
                    .speed(1.0),
            );
            ui.add_space(10.0);
            ui.weak("Falloff");
            for falloff in Falloff::ALL {
                ui.selectable_value(&mut tool.falloff, falloff, falloff.label());
            }
        });
        if tool.op == HeightOp::Flatten {
            ui.horizontal(|ui| {
                ui.weak("Level to");
                if ui
                    .selectable_label(tool.flatten_to.is_none(), "Height under the cursor")
                    .clicked()
                {
                    tool.flatten_to = None;
                }
                if ui
                    .selectable_label(tool.flatten_to.is_some(), "A height")
                    .clicked()
                    && tool.flatten_to.is_none()
                {
                    tool.flatten_to = Some(0);
                }
                if let Some(target) = &mut tool.flatten_to {
                    let mut value = elevation(*target);
                    if ui
                        .add(
                            egui::DragValue::new(&mut value)
                                .range(0..=MAX_ELEVATION)
                                .speed(1.0),
                        )
                        .changed()
                    {
                        *target = height_for_elevation(value);
                    }
                }
            });
        }
        let op = tool.op;
        self.prefs.height_tool = tool;
        let selected = self.selection_tiles.len();
        ui.horizontal(|ui| {
            ui.weak(op.hint());
            if selected > 0
                && ui
                    .button(format!(
                        "{}  Apply to {selected} selected tiles",
                        icons::TOOL_HEIGHT
                    ))
                    .clicked()
            {
                self.height_on_selection();
            }
        });
    }
}

/// A drawer tab with its arm checkbox (when it has one). Returns `true` when the tab was clicked.
fn channel_tab(
    ui: &mut egui::Ui,
    selected: bool,
    with_checkbox: bool,
    tab: DrawerTab,
    app: &mut EditorApp,
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
                if with_checkbox {
                    let armed = app.prefs.brush.is_armed(tab);
                    let mut on = armed;
                    if ui
                        .checkbox(&mut on, "")
                        .on_hover_text(if armed {
                            "The brush sets this. Untick to keep each tile's own."
                        } else {
                            "Each tile keeps its own. Tick to paint this channel."
                        })
                        .changed()
                    {
                        app.prefs.brush.set_armed(tab, on);
                    }
                }
                let text = egui::RichText::new(tab.label()).color(if selected {
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

/// The "no floor" swatch: a struck-through square.
fn none_swatch(ui: &mut egui::Ui, selected: bool) -> egui::Response {
    swatch(ui, SWATCH, selected, |painter, rect| {
        painter.rect_filled(rect, 3.0, Color32::from_gray(38));
        painter.line_segment(
            [rect.left_bottom(), rect.right_top()],
            Stroke::new(1.5, Color32::from_rgb(220, 90, 90)),
        );
    })
}

fn overlay_tip(sw: &OverlaySwatch) -> String {
    let mut tip = format!("Overlay {}  \u{b7}  {} tiles", sw.id, sw.uses);
    if let Some(texture) = sw.texture {
        tip.push_str(&format!("\ntexture {texture}"));
    }
    if sw.rgb == [255, 0, 255] && sw.texture.is_none() {
        tip.push_str("\nplaceholder (no colour)");
    }
    if sw.hide_underlay {
        tip.push_str("\nhides the underlay");
    }
    tip
}

fn paint_overlay_swatch(painter: &egui::Painter, rect: Rect, fill: OverlayFill, hides: bool) {
    match fill {
        OverlayFill::Color(c) => {
            painter.rect_filled(rect, 3.0, c);
        }
        OverlayFill::Texture { id, offset } => {
            painter.rect_filled(rect, 3.0, Color32::from_gray(30));
            painter.image(
                id,
                rect,
                Rect::from_min_max(
                    egui::pos2(offset.0, offset.1),
                    egui::pos2(offset.0 + 1.0, offset.1 + 1.0),
                ),
                Color32::WHITE,
            );
        }
    }
    if hides {
        painter.circle_filled(
            rect.right_top() + Vec2::new(-5.0, 5.0),
            2.2,
            Color32::from_white_alpha(210),
        );
    }
}
