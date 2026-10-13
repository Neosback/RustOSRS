//! Floor palettes: overlay and underlay swatches, textures, and their loading state.

use super::*;

/// The "no floor" swatch: a struck-through square.
pub(super) fn none_swatch(ui: &mut egui::Ui, size: f32, selected: bool) -> egui::Response {
    swatch(ui, size, selected, |painter, rect| {
        painter.rect_filled(rect, 3.0, Color32::from_gray(38));
        painter.line_segment(
            [rect.left_bottom(), rect.right_top()],
            Stroke::new(1.5, Color32::from_rgb(220, 90, 90)),
        );
    })
}

pub(super) fn overlay_tip(sw: &OverlaySwatch) -> String {
    let mut tip = format!("Overlay {}  \u{b7}  {} tiles", sw.id, sw.uses);
    if let Some(texture) = sw.texture {
        tip.push_str(&format!("\ntexture {texture}"));
    }
    if sw.rgb == [255, 0, 255] && sw.texture.is_none() {
        tip.push_str("\ntransparent: draws nothing (the map's placeholder)");
    }
    if sw.hide_underlay {
        tip.push_str("\nhides the underlay");
    }
    tip
}

pub(super) fn paint_overlay_swatch(
    painter: &egui::Painter,
    rect: Rect,
    fill: OverlayFill,
    hides: bool,
) {
    match fill {
        OverlayFill::Color(c) => {
            painter.rect_filled(rect, 3.0, c);
        }
        OverlayFill::Transparent { light, dark } => {
            crate::app::widgets::paint_checker(painter, rect, light, dark, 4);
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

impl EditorApp {
    /// The egui texture of a floor texture (made on first use), with its scroll animation.
    pub(super) fn floor_texture(
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
    pub(in crate::app) fn overlay_fill(
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
            return OverlayFill::Transparent {
                light: self.prefs.colors.get(crate::style::ColorKey::CheckerLight),
                dark: self.prefs.colors.get(crate::style::ColorKey::CheckerDark),
            };
        }
        OverlayFill::Color(color(swatch.rgb))
    }

    pub(in crate::app) fn overlay_swatch(&self, id: u16) -> Option<OverlaySwatch> {
        let palette = self.palette.as_ref()?;
        let index = palette
            .overlays
            .binary_search_by_key(&id, |swatch| swatch.id)
            .ok()?;
        Some(palette.overlays[index])
    }

    pub(in crate::app) fn underlay_color(&self, id: u16) -> Option<Color32> {
        let palette = self.palette.as_ref()?;
        let index = palette
            .underlays
            .binary_search_by_key(&id, |swatch| swatch.id)
            .ok()?;
        Some(color(palette.underlays[index].rgb))
    }

    pub(super) fn palette_loading(&self, ui: &mut egui::Ui) -> bool {
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
    pub(super) fn palette_header(
        &mut self,
        ui: &mut egui::Ui,
        shown: usize,
        textured: bool,
    ) -> String {
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
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (level, label, tip) in [
                    (2_u8, "L", "Large swatches"),
                    (1, "M", "Medium swatches"),
                    (0, "S", "Small swatches"),
                ] {
                    if ui
                        .selectable_label(self.prefs.swatch_size == level, label)
                        .on_hover_text(tip)
                        .clicked()
                    {
                        self.prefs.swatch_size = level;
                    }
                }
            });
        });
        self.palette_search.trim().to_owned()
    }

    pub(super) fn overlay_palette(&mut self, ui: &mut egui::Ui, popup: bool) {
        if self.palette_loading(ui) {
            return;
        }
        let Some(palette) = self.palette.clone() else {
            return;
        };
        let size = SWATCH_SIZES[usize::from(self.prefs.swatch_size).min(2)];
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
            .max_height(if popup { 300.0 } else { f32::INFINITY })
            .auto_shrink([popup, popup])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(5.0);
                    if none_swatch(ui, size, current == Fill::Clear)
                        .on_hover_text("No overlay: clears the overlay, shape and rotation")
                        .clicked()
                    {
                        picked = Some(Fill::Clear);
                    }
                    for sw in &swatches {
                        let fill = self.overlay_fill(&ctx, sw);
                        let tip = overlay_tip(sw);
                        let hides = sw.hide_underlay;
                        if swatch(ui, size, current == Fill::Set(sw.id), |painter, rect| {
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
        if popup && picked.is_some() {
            ui.close();
        }
        if let Some(fill) = picked {
            self.prefs.brush.overlay.fill = fill;
            self.prefs.brush.overlay.armed = true;
        }
    }

    pub(super) fn underlay_palette(&mut self, ui: &mut egui::Ui, popup: bool) {
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
        let size = SWATCH_SIZES[usize::from(self.prefs.swatch_size).min(2)];
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
            .max_height(if popup { 300.0 } else { f32::INFINITY })
            .auto_shrink([popup, popup])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(5.0);
                    if none_swatch(ui, size, current == Fill::Clear)
                        .on_hover_text("No underlay: the tile is hidden unless it has an overlay")
                        .clicked()
                    {
                        picked = Some(Fill::Clear);
                    }
                    for sw in &swatches {
                        if swatch(ui, size, current == Fill::Set(sw.id), |painter, rect| {
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
        if popup && picked.is_some() {
            ui.close();
        }
        if let Some(fill) = picked {
            self.prefs.brush.underlay.fill = fill;
            self.prefs.brush.underlay.armed = true;
        }
    }
}
