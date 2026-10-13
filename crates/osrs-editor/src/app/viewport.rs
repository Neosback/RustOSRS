//! The 3D viewport's own work: picking under the cursor, the selection outlines, render targets.

use super::*;

impl EditorApp {
    /// Hover / click / right-click picking against the resident regions' inspection data.
    pub(super) fn update_picking(
        &mut self,
        response: &egui::Response,
        rect: egui::Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) {
        // Camera drags (right and middle button) freeze picking; a left drag paints or selects.
        if response.dragged() && !response.dragged_by(egui::PointerButton::Primary) {
            return;
        }
        // The HUD sits over the viewport's corner; with the pointer on it the viewport is not hovered,
        // and dropping the pick there would hide the HUD, uncover the pointer and flicker. So the
        // world under the HUD keeps being picked.
        let over_hud = || {
            let pointer = response.ctx.input(|i| i.pointer.latest_pos())?;
            self.hud_rect.filter(|hud| hud.contains(pointer))?;
            Some(pointer)
        };
        let Some(position) = response.hover_pos().or_else(over_hud) else {
            self.hover = Pick::default();
            // The pointer left the viewport: a stroke or box in progress ends here.
            if !response.ctx.input(|i| i.pointer.primary_down()) {
                self.end_stroke();
                self.marquee = None;
            }
            return;
        };
        let ray = pick::ray_through_pixel(
            &self.camera,
            self.renderer.render_origin(),
            width as f32,
            height as f32,
            (position.x - rect.min.x) * pixels_per_point,
            (position.y - rect.min.y) * pixels_per_point,
        );
        self.hover = pick::pick(&self.infos, &ray, self.view_plane);
        // Selecting tools read the tile an object stands on, not the ground the ray reaches behind it.
        if matches!(
            self.prefs.tool,
            crate::paint::Tool::Select | crate::paint::Tool::Objects
        ) && let Some(object) = &self.hover.object
            && let Some(loc) = self
                .infos
                .get(&object.region)
                .and_then(|info| info.locs.get(object.index))
        {
            self.hover.tile = Some(pick::TilePick {
                tile: loc.tile,
                at: (loc.tile.0 as f32 + 0.5, loc.tile.1 as f32 + 0.5),
            });
        }
        if let Some(pick) = self.hover.tile {
            self.last_hover_tile = Some(pick.tile);
        }
        // The brush tools keep the left button for painting; Select clicks and boxes.
        self.brush_input(&response.ctx, response);
        self.spline_input(&response.ctx, response);
        self.selection_input(
            &response.ctx,
            response,
            rect,
            pixels_per_point,
            width,
            height,
        );
        if response.secondary_clicked() && response.ctx.input(|i| i.modifiers.shift) {
            self.context_pick = self.hover.clone();
            self.context_menu = response.interact_pointer_pos();
        }
        let wanted: Vec<ObjectPick> = [
            self.hover.object.clone(),
            self.selected.clone(),
            self.context_pick.object.clone(),
        ]
        .into_iter()
        .flatten()
        .collect();
        for object in wanted {
            let key = (object.region, object.index);
            if self.outlines.contains_key(&key) || !self.outline_requested.insert(key) {
                continue;
            }
            if let Some(loc) = self
                .infos
                .get(&object.region)
                .and_then(|info| info.locs.get(object.index))
            {
                self.world
                    .request_outline(object.region, object.index, loc.clone());
            }
        }
    }

    pub(super) fn draw_selection(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) {
        let painter = ui.painter().with_clip_rect(rect);
        let outline = |object: &ObjectPick, fill: egui::Color32, edge: egui::Color32| {
            let Some(triangles) = self.outlines.get(&(object.region, object.index)) else {
                return;
            };
            let to_screen = |point: [i32; 3]| {
                pick::project(
                    &self.camera,
                    self.renderer.render_origin(),
                    width as f32,
                    height as f32,
                    [
                        f64::from(point[0]),
                        f64::from(point[1]),
                        f64::from(point[2]),
                    ],
                )
                .map(|p| {
                    egui::pos2(
                        rect.min.x + p[0] / pixels_per_point,
                        rect.min.y + p[1] / pixels_per_point,
                    )
                })
            };
            let mut mesh = egui::epaint::Mesh::default();
            for triangle in triangles.iter() {
                let (Some(a), Some(b), Some(c)) = (
                    to_screen(triangle[0]),
                    to_screen(triangle[1]),
                    to_screen(triangle[2]),
                ) else {
                    continue;
                };
                let base = mesh.vertices.len() as u32;
                for position in [a, b, c] {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: position,
                        uv: egui::epaint::WHITE_UV,
                        color: fill,
                    });
                }
                mesh.indices.extend([base, base + 1, base + 2]);
                painter.line_segment([a, b], egui::Stroke::new(0.75, edge));
                painter.line_segment([b, c], egui::Stroke::new(0.75, edge));
                painter.line_segment([c, a], egui::Stroke::new(0.75, edge));
            }
            painter.add(egui::Shape::mesh(mesh));
        };
        let colors = &self.prefs.colors;
        if let Some(object) = &self.hover.object {
            outline(
                object,
                colors.get(ColorKey::ObjectHoverFill),
                colors.get(ColorKey::ObjectHoverOutline),
            );
        }
        if let Some(object) = &self.selected {
            outline(
                object,
                colors.get(ColorKey::SelectionFill),
                colors.get(ColorKey::SelectionOutline),
            );
        }
        // The hovered tile, outlined like RuneLite's tile indicator; with a brush tool, the tiles
        // the brush would paint (orange), or grey when nothing is armed.
        if self.prefs.hover_tile
            && let Some(tile) = &self.hover.tile
        {
            let brushing = self.prefs.tool != crate::paint::Tool::Select;
            let tiles = if brushing {
                self.prefs.brush.footprint_tiles(tile.tile)
            } else {
                vec![tile.tile]
            };
            let (fill, edge) = if !brushing {
                (
                    colors.get(ColorKey::HoverTileFill),
                    colors.get(ColorKey::HoverTileOutline),
                )
            } else if self.prefs.brush.is_empty() {
                (
                    colors.get(ColorKey::BrushEmptyFill),
                    colors.get(ColorKey::BrushEmptyOutline),
                )
            } else {
                (
                    colors.get(ColorKey::BrushFill),
                    colors.get(ColorKey::BrushOutline),
                )
            };
            for tile in tiles {
                let Some(corners) = interact::tile_corners(&self.infos, tile, self.view_plane)
                else {
                    continue;
                };
                let points: Vec<egui::Pos2> = corners
                    .iter()
                    .filter_map(|corner| {
                        pick::project(
                            &self.camera,
                            self.renderer.render_origin(),
                            width as f32,
                            height as f32,
                            *corner,
                        )
                    })
                    .map(|p| {
                        egui::pos2(
                            rect.min.x + p[0] / pixels_per_point,
                            rect.min.y + p[1] / pixels_per_point,
                        )
                    })
                    .collect();
                if points.len() == 4 {
                    painter.add(egui::Shape::convex_polygon(
                        points,
                        fill,
                        egui::Stroke::new(1.5, edge),
                    ));
                }
            }
        }
        // The in-game action text next to the cursor (RuneLite's mouse tooltips).
        if self.prefs.mouse_tooltips
            && self.prefs.tool == crate::paint::Tool::Select
            && self.context_menu.is_none()
            && let Some(pointer) = ui.input(|i| i.pointer.hover_pos())
            && rect.contains(pointer)
            && !ui.input(|i| i.pointer.any_down())
        {
            let entries = interact::entries(&self.infos, &self.hover);
            if let Some(top) = entries.first() {
                // Menu entries plus the always-present Cancel.
                let extra = interact::more_options(entries.len() + 1);
                let job = interact::entry_job(top, 13.5, extra.as_deref());
                let galley = painter.layout_job(job);
                let size = galley.size() + egui::vec2(12.0, 6.0);
                let mut at = pointer + egui::vec2(14.0, 18.0);
                if at.x + size.x > rect.right() {
                    at.x = pointer.x - size.x - 10.0;
                }
                if at.y + size.y > rect.bottom() {
                    at.y = pointer.y - size.y - 10.0;
                }
                let tip = egui::Rect::from_min_size(at, size);
                painter.rect_filled(tip, 3.0, egui::Color32::from_black_alpha(190));
                painter.rect_stroke(
                    tip,
                    3.0,
                    egui::Stroke::new(1.0, egui::Color32::from_white_alpha(40)),
                    egui::StrokeKind::Inside,
                );
                painter.galley(at + egui::vec2(6.0, 3.0), galley, egui::Color32::WHITE);
            }
        }
        let text = pick::summary(&self.infos, &self.hover);
        if !text.is_empty() {
            painter.text(
                rect.left_bottom() + egui::vec2(8.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                text,
                egui::FontId::monospace(13.0),
                egui::Color32::WHITE,
            );
        }
    }

    pub(super) fn ensure_targets(&mut self, width: u32, height: u32) {
        self.renderer.ensure_targets(width, height);
    }
}
