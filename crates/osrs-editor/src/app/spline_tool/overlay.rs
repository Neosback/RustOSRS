//! The spline tool's viewport overlay.

use super::*;

impl EditorApp {
    /// The path preview: the planned tiles, the centre line, a ghost segment to the cursor, the
    /// control points.
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
        let cursor = self.interaction.hover().tile.map(|pick| pick.at);
        let outline = self.settings.prefs.colors.get(ColorKey::PathLine);
        let ghost = self.settings.prefs.colors.get(ColorKey::PathGhost);

        if self.interaction.spline_points().is_empty() {
            if let Some(at) = cursor
                && let Some(position) = ground(at, 8.0)
            {
                painter.circle_stroke(position, 9.0, Stroke::new(2.0, ghost));
                painter.text(
                    position + egui::vec2(14.0, -12.0),
                    egui::Align2::LEFT_CENTER,
                    "Click to start the path",
                    egui::FontId::proportional(12.0),
                    Color32::WHITE,
                );
            }
            return;
        }

        // The planned tiles. While a replacement plan is running, retain the last tiles as a
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
            let tile_polygon = |tile: (i32, i32), shape: u8, rotation: u8| {
                let Some(corners) = interact::tile_corners(self.world.infos(), tile, plane) else {
                    return;
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
                for triangle in terrain_path::overlay_triangles(shape, rotation) {
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
            };
            for paint in &preview.paints {
                tile_polygon((paint.tile_x, paint.tile_y), paint.shape, paint.orientation);
            }
        }

        // The centre line.
        let drawn: Vec<(f32, f32)> = match self.interaction.current_spline_preview() {
            Some(preview) if !preview.lines.is_empty() => {
                preview.lines.iter().flatten().copied().collect()
            }
            _ => terrain_path::smooth_catmull_rom(self.interaction.spline_points()),
        };
        let screen: Vec<Pos2> = drawn.iter().filter_map(|p| ground(*p, 10.0)).collect();
        if screen.len() >= 2 {
            painter.add(egui::Shape::line(
                screen.clone(),
                Stroke::new(4.5, Color32::from_black_alpha(110)),
            ));
            painter.add(egui::Shape::line(screen, Stroke::new(2.5, outline)));
        }
        // A ghost stretch from the last point to the cursor while no point is being dragged.
        if self.interaction.spline_drag().is_none()
            && let (Some(last), Some(at)) = (self.interaction.spline_points().last(), cursor)
            && let (Some(a), Some(b)) = (ground(*last, 10.0), ground(at, 10.0))
        {
            painter.add(egui::Shape::dashed_line(
                &[a, b],
                Stroke::new(1.5, ghost),
                6.0,
                5.0,
            ));
            painter.circle_stroke(b, 5.0, Stroke::new(1.5, ghost));
        }
        // The control points.
        for (index, point) in self.interaction.spline_points().iter().enumerate() {
            if let Some(position) = ground(*point, 12.0) {
                let active = self.interaction.spline_drag() == Some(index);
                painter.circle_filled(position, if active { 8.0 } else { 6.5 }, Color32::WHITE);
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
