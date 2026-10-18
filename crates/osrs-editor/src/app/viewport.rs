//! The 3D viewport's own work: picking under the cursor, the selection outlines, render targets.

use super::*;

mod frame;
mod hover;
pub(super) use frame::{draw as frame, read_only};
mod outlines;
use outlines::{MAX_OUTLINED, queue_selection_outlines};

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
        queue_selection_outlines(self);
        // Camera drags (right and middle button) freeze picking; a left drag paints or selects.
        if response.dragged() && !response.dragged_by(egui::PointerButton::Primary) {
            return;
        }
        // The HUD sits over the viewport's corner; with the pointer on it the viewport is not hovered,
        // and dropping the pick there would hide the HUD, uncover the pointer and flicker. So the
        // world under the HUD keeps being picked.
        let over_hud = || {
            let pointer = response.ctx.input(|i| i.pointer.latest_pos())?;
            self.ui.hud_rect.filter(|hud| hud.contains(pointer))?;
            Some(pointer)
        };
        let scripted = self
            .interaction
            .scripted_pointer()
            .map(|pointer| match pointer {
                interaction_runtime::ScriptedPointer::Centre => rect.center(),
                interaction_runtime::ScriptedPointer::At(x, y) => {
                    egui::pos2(rect.min.x + x, rect.min.y + y)
                }
            });
        let Some(position) = response.hover_pos().or_else(over_hud).or(scripted) else {
            self.interaction.clear_hover();
            // The pointer left the viewport: a stroke or box in progress ends here.
            if !response.ctx.input(|i| i.pointer.primary_down()) {
                self.end_stroke();
                self.interaction.cancel_marquee();
            }
            return;
        };
        let ray = pick::ray_through_pixel(
            self.interaction.camera(),
            self.renderer.render_origin(),
            width as f32,
            height as f32,
            (position.x - rect.min.x) * pixels_per_point,
            (position.y - rect.min.y) * pixels_per_point,
        );
        let mut hover = pick::pick(
            self.world.infos(),
            &ray,
            self.interaction.view_plane(),
            self.renderer.roof_view(),
        );
        // Selecting tools read the tile an object stands on, not the ground the ray reaches behind it.
        if hover::anchors_on_objects(self)
            && let Some(object) = &hover.object
            && let Some(loc) = self
                .world
                .infos()
                .get(&object.region)
                .and_then(|info| info.locs.get(object.index))
        {
            hover.tile = Some(pick::TilePick { tile: loc.tile });
        }
        self.interaction.set_hover(hover);
        // The brush tools keep the left button for painting; Select clicks and boxes.
        let consumed = npc_panel::armed_input(self, response)
            || prefab_panel::armed_input(self, response)
            || sound_panel::marker_click(self, response, rect, pixels_per_point, width, height);
        if !consumed {
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
        }
        if response.secondary_clicked() && response.ctx.input(|i| i.modifiers.shift) {
            self.interaction.capture_context_pick();
            self.ui.context_menu = response.interact_pointer_pos();
        }
        let wanted: Vec<ObjectPick> = [
            self.interaction.hover().object.clone(),
            self.interaction.selected().clone(),
            self.interaction.context_pick().object.clone(),
        ]
        .into_iter()
        .flatten()
        .collect();
        for object in wanted {
            let key = (object.region, object.index);
            self.world.queue_outline(key);
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
            let Some(triangles) = self.world.outline(&(object.region, object.index)) else {
                return;
            };
            let to_screen = |point: [i32; 3]| {
                pick::project(
                    self.interaction.camera(),
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
        let colors = &self.settings.prefs.colors;
        // Everything selected together (walls, roof, furniture of a building) as one tinted
        // ghost of the real geometry: one mesh, no edges, so hundreds of objects stay cheap.
        if self.interaction.selection_objects().len() > 1 {
            let mut mesh = egui::epaint::Mesh::default();
            let fill = colors.get(ColorKey::SelectionFill).gamma_multiply(0.55);
            for object in self
                .interaction
                .selection_objects()
                .iter()
                .take(MAX_OUTLINED)
            {
                if let Some(triangles) = self.world.outline(&(object.region, object.index)) {
                    add_triangles(
                        self,
                        triangles,
                        &mut mesh,
                        fill,
                        rect,
                        pixels_per_point,
                        width,
                        height,
                    );
                }
            }
            painter.add(egui::Shape::mesh(mesh));
        }
        if hover::anchors_on_objects(self)
            && let Some(object) = self.interaction.hover().object.as_ref()
        {
            outline(
                object,
                colors.get(ColorKey::ObjectHoverFill),
                colors.get(ColorKey::ObjectHoverOutline),
            );
        }
        if let Some(object) = &self.interaction.selected() {
            outline(
                object,
                colors.get(ColorKey::SelectionFill),
                colors.get(ColorKey::SelectionOutline),
            );
        }
        // The hovered tile, outlined like RuneLite's tile indicator; with a brush tool, the tiles
        // the brush would paint (orange), or grey when nothing is armed.
        if self.settings.prefs.hover_tile
            && let Some(tile) = self.interaction.hover().tile
        {
            let brushing = self.settings.prefs.tool.uses_brush();
            let tiles = if brushing {
                self.settings.prefs.brush.footprint_tiles(tile.tile)
            } else {
                vec![tile.tile]
            };
            let (fill, edge) = if !brushing {
                (
                    colors.get(ColorKey::HoverTileFill),
                    colors.get(ColorKey::HoverTileOutline),
                )
            } else if self.settings.prefs.brush.is_empty() {
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
                let Some(corners) =
                    interact::tile_corners(self.world.infos(), tile, self.interaction.view_plane())
                else {
                    continue;
                };
                let points: Vec<egui::Pos2> = corners
                    .iter()
                    .filter_map(|corner| {
                        pick::project(
                            self.interaction.camera(),
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
        if self.settings.prefs.mouse_tooltips
            && self.settings.prefs.tool == crate::paint::Tool::Select
            && self.ui.context_menu.is_none()
            && let Some(pointer) = ui.input(|i| i.pointer.hover_pos())
            && rect.contains(pointer)
            && !ui.input(|i| i.pointer.any_down())
        {
            let entries = interact::entries(self.world.infos(), self.interaction.hover());
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
        let text = pick::summary(self.world.infos(), self.interaction.hover());
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

    /// Shared render composition for authoring and the isolated inspection viewport.
    pub(super) fn render_viewport_image(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        width: u32,
        height: u32,
    ) {
        use osrs_render::gpu::FrameParams;
        if self.world.is_ready() {
            self.ensure_targets(width, height);
            self.diagnostics.capture_context(|| {
                serde_json::json!({
                    "settings": self.settings.prefs,
                    "viewport_pixels": [width, height],
                    "render_origin": self.renderer.render_origin(),
                    "camera": {
                    "local_position": [self.interaction.camera().x, self.interaction.camera().y, self.interaction.camera().z],
                    "yaw": self.interaction.camera().yaw, "pitch": self.interaction.camera().pitch,
                    "fov_degrees": self.interaction.camera().fov_degrees,
                },
                "adapter": self.renderer.adapter_report(),
                "display": {
                    "pixels_per_point": ui.ctx().pixels_per_point(),
                    "render_scale": self.settings.prefs.render_scale,
                    "refresh_hz": null,
                    "refresh_hz_unavailable_reason": "eframe does not expose the monitor refresh rate; performance.py records the system value",
                },
                "gpu_pass_timers": self.renderer.gpu_pass_samples().keys().collect::<Vec<_>>(),
                "view_plane": self.interaction.view_plane(),
                    "owned_gpu_bytes": self.renderer.resource_bytes(),
                    "semantic_revision": self.session.revision(),
                "cache_build_and_profile": self.project.cache_build(),
                })
            });
            self.diagnostics.capture_camera(|| {
                let camera = self.interaction.camera();
                let origin = self.renderer.render_origin();
                [
                    camera.x.to_bits(),
                    camera.y.to_bits(),
                    camera.z.to_bits(),
                    camera.yaw.to_bits(),
                    camera.pitch.to_bits(),
                    camera.fov_degrees.to_bits(),
                    origin.0 as u32,
                    origin.1 as u32,
                    u32::from(self.interaction.view_plane()),
                ]
                .to_vec()
            });
            if self.settings.prefs.render_profile == ProfileChoice::Hd {
                self.update_hd_lighting();
            }
            let phase = Instant::now();
            self.renderer
                .set_hd_params(self.hd.params(self.settings.prefs.hd_params()));
            super::roof_view::apply(self);
            if let Some(mut frame) = self.renderer.encode_viewport(FrameParams {
                camera: self.interaction.camera().reference(width as f32),
                view_plane: self.interaction.view_plane(),
                // The client's `gameCycle & 127`, at 50 cycles per second.
                tick: ((self
                    .diagnostics
                    .scene_seconds(self.started.elapsed().as_secs_f32())
                    * 50.0) as u32)
                    & 127,
                brightness: self.settings.prefs.brightness,
                remove_color_banding: self.settings.prefs.remove_color_banding,
                bright_textures: self.settings.prefs.bright_textures,
                fog: self.settings.prefs.fog(),
                colorblind: self.settings.prefs.colorblind(),
                clear_color: self.settings.prefs.sky_color.map(f64::from),
                profile: self.settings.prefs.render_profile.render_profile(),
            }) {
                self.diagnostics.phase("encode", phase);
                self.diagnostics
                    .record_scene_encoding(frame.command_buffer.is_some());
                let phase = Instant::now();
                let gpu_scene_ms = self.renderer.submit_viewport(&mut frame);
                self.diagnostics.phase("submit", phase);
                self.diagnostics.set_gpu_scene_ms(gpu_scene_ms);
                self.diagnostics.capture_counters(|| {
                    let counters = self.renderer.take_counters();
                    vec![
                        ("draw_calls", u64::from(counters.draw_calls)),
                        ("vertices", counters.vertices),
                        ("upload_bytes", counters.upload_bytes),
                        ("shadow_redraws", u64::from(counters.shadow_redraws)),
                        ("visible_zones", u64::from(counters.visible_zones)),
                        ("resident_regions", self.world.resident_regions() as u64),
                        ("builds_in_flight", self.world.in_flight_count() as u64),
                    ]
                });
                self.diagnostics
                    .capture_gpu_samples(|| self.renderer.gpu_pass_samples());
                let probe = self.diagnostics.stats().gpu_probe();
                self.renderer.on_submitted_work_done(probe);
                ui.painter().image(
                    frame.texture_id,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }
    }

    pub(super) fn ensure_targets(&mut self, width: u32, height: u32) {
        self.renderer.ensure_targets(width, height);
    }
}

/// Project one object's outline triangles to the screen and add them to `mesh` in `fill`.
#[allow(clippy::too_many_arguments)]
fn add_triangles(
    app: &EditorApp,
    triangles: &[[[i32; 3]; 3]],
    mesh: &mut egui::epaint::Mesh,
    fill: egui::Color32,
    rect: egui::Rect,
    pixels_per_point: f32,
    width: u32,
    height: u32,
) {
    let to_screen = |point: [i32; 3]| {
        pick::project(
            app.interaction.camera(),
            app.renderer.render_origin(),
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
    for triangle in triangles {
        let (Some(a), Some(b), Some(c)) = (
            to_screen(triangle[0]),
            to_screen(triangle[1]),
            to_screen(triangle[2]),
        ) else {
            continue;
        };
        let base = u32::try_from(mesh.vertices.len()).unwrap_or(0);
        for position in [a, b, c] {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: position,
                uv: egui::epaint::WHITE_UV,
                color: fill,
            });
        }
        mesh.indices.extend([base, base + 1, base + 2]);
    }
}
