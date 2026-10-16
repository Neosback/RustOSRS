//! The spline tool's viewport overlay.

use super::*;

/// Colour of one preview vertex. A textured face shows its texture's average colour lit by the
/// vertex's lightness (an approximation of the textured raster, which samples the texture).
fn path_vertex_color(rgb: i32, texture_rgb: Option<i32>, alpha: u8) -> Color32 {
    let channel = |value: i32, shift: u32| ((value >> shift) & 0xff) as u32;
    match texture_rgb {
        Some(texture) => {
            let light = (channel(rgb, 16) + channel(rgb, 8) + channel(rgb, 0)) / 3;
            let lit = |shift: u32| (channel(texture, shift) * light / 128).min(255) as u8;
            Color32::from_rgba_unmultiplied(lit(16), lit(8), lit(0), alpha)
        }
        None => Color32::from_rgba_unmultiplied(
            channel(rgb, 16) as u8,
            channel(rgb, 8) as u8,
            channel(rgb, 0) as u8,
            alpha,
        ),
    }
}

impl EditorApp {
    /// The path preview: the fitted tiles (to the cursor's tile), the centre line, the points.
    pub(in crate::app) fn draw_spline(
        &self,
        painter: &egui::Painter,
        viewport: Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) {
        if self.settings.prefs.tool != Tool::Spline {
            return;
        }
        let plane = self.interaction.view_plane();
        let project = |p: [f64; 3]| self.screen_point(p, viewport, pixels_per_point, width, height);
        // A point on the ground at tile coordinates, lifted a little.
        let ground = |p: (f32, f32), lift: f64| -> Option<Pos2> {
            let tile = (p.0.floor() as i32, p.1.floor() as i32);
            let corners = interact::tile_corners(self.world.infos(), tile, plane)?;
            // Bilinear height inside the tile.
            let (u, v) = (f64::from(p.0 - p.0.floor()), f64::from(p.1 - p.1.floor()));
            let y = corners[0][1] * (1.0 - u) * (1.0 - v)
                + corners[1][1] * u * (1.0 - v)
                + corners[2][1] * u * v
                + corners[3][1] * (1.0 - u) * v;
            project([f64::from(p.0) * 128.0, y - lift, f64::from(p.1) * 128.0])
        };
        let cursor = super::pending_point(self);
        let outline = self.settings.prefs.colors.get(ColorKey::PathLine);
        let ghost = self.settings.prefs.colors.get(ColorKey::PathGhost);

        // The fitted tiles. While a replacement plan is running, retain the last tiles as a
        // faint spatial reference instead of flashing the whole preview off.
        let base_overlay_color = self.settings.prefs.colors.get(ColorKey::PathFill);
        let base_edge_color = self.settings.prefs.colors.get(ColorKey::PathEdge);
        if let Some(preview) = self.interaction.spline_preview() {
            let current = self.interaction.spline_preview_is_current();
            let fade = |color: Color32| {
                if current {
                    color
                } else {
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), color.a() / 3)
                }
            };
            let overlay_color = fade(base_overlay_color);
            let edge = Stroke::new(0.75, fade(base_edge_color));
            // The real builder output for the planned tiles, when it is for this preview.
            let real = self
                .interaction
                .spline_appearance()
                .filter(|a| a.key == preview.key && !a.tiles.is_empty());
            let alpha = if current { 235 } else { 80 };
            if let Some(real) = real {
                let mut mesh = egui::Mesh::default();
                for tile in &real.tiles {
                    for triangle in &tile.triangles {
                        let points = triangle.vertices.map(|vertex| {
                            let [x, y, z] = vertex.position;
                            let world = [
                                f64::from(x) + f64::from(real.origin.0) * 128.0,
                                f64::from(y) - 4.0,
                                f64::from(z) + f64::from(real.origin.1) * 128.0,
                            ];
                            (project(world), vertex.rgb)
                        });
                        if points.iter().any(|(position, _)| position.is_none()) {
                            continue;
                        }
                        let base = mesh.vertices.len() as u32;
                        for (position, rgb) in points {
                            mesh.colored_vertex(
                                position.unwrap_or_default(),
                                path_vertex_color(rgb, triangle.texture_rgb, alpha),
                            );
                        }
                        mesh.add_triangle(base, base + 1, base + 2);
                    }
                }
                painter.add(egui::Shape::mesh(mesh));
            } else {
                for paint in &preview.paints {
                    let tile = (paint.tile_x, paint.tile_y);
                    let Some(corners) = interact::tile_corners(self.world.infos(), tile, plane)
                    else {
                        continue;
                    };
                    // Map a tile-local (u, v) in 0..1 to the tile's four corners (bilinear).
                    let at = |u: f32, v: f32| -> Option<Pos2> {
                        let (u, v) = (f64::from(u), f64::from(v));
                        let lerp = |a: [f64; 3], b: [f64; 3], t: f64| {
                            [
                                a[0] + (b[0] - a[0]) * t,
                                a[1] + (b[1] - a[1]) * t,
                                a[2] + (b[2] - a[2]) * t,
                            ]
                        };
                        let south = lerp(corners[0], corners[1], u);
                        let north = lerp(corners[3], corners[2], u);
                        project(lerp(south, north, v))
                    };
                    for triangle in terrain_path::overlay_triangles(paint.shape, paint.orientation)
                    {
                        if let (Some(a), Some(b), Some(c)) = (
                            at(triangle[0][0], triangle[0][1]),
                            at(triangle[1][0], triangle[1][1]),
                            at(triangle[2][0], triangle[2][1]),
                        ) {
                            painter.add(egui::Shape::convex_polygon(
                                vec![a, b, c],
                                overlay_color,
                                edge,
                            ));
                        }
                    }
                }
            }
        }

        // The centre line the path was fitted along.
        let drawn: Vec<(f32, f32)> = match self.interaction.current_spline_preview() {
            Some(preview) if !preview.line.is_empty() => preview
                .line
                .iter()
                .map(|p| (p.0 as f32, p.1 as f32))
                .collect(),
            _ => terrain_path::smooth(
                &self
                    .interaction
                    .spline_points()
                    .iter()
                    .map(|p| (f64::from(p.0), f64::from(p.1)))
                    .collect::<Vec<_>>(),
            )
            .iter()
            .map(|p| (p.0 as f32, p.1 as f32))
            .collect(),
        };
        let screen: Vec<Pos2> = drawn.iter().filter_map(|p| ground(*p, 10.0)).collect();
        if screen.len() >= 2 {
            painter.add(egui::Shape::line(
                screen.clone(),
                Stroke::new(4.5, Color32::from_black_alpha(110)),
            ));
            painter.add(egui::Shape::line(screen, Stroke::new(2.5, outline)));
        }
        // Where the next click lands.
        if let Some(at) = cursor
            && let Some(b) = ground(at, 10.0)
        {
            painter.circle_stroke(b, 5.0, Stroke::new(1.5, ghost));
        }
        // The points, numbered in the order they were clicked.
        for (index, point) in self.interaction.spline_points().iter().enumerate() {
            if let Some(position) = ground(*point, 12.0) {
                painter.circle_filled(position, 6.5, Color32::WHITE);
                painter.circle_stroke(position, 7.5, Stroke::new(2.0, outline));
                painter.text(
                    position + egui::vec2(11.0, -11.0),
                    egui::Align2::LEFT_CENTER,
                    (index + 1).to_string(),
                    egui::FontId::proportional(11.5),
                    Color32::WHITE,
                );
            }
        }
    }
}
