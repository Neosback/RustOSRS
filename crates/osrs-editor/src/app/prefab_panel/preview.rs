//! A small top-down picture of a prefab: ground colours, walls as edges, objects as squares.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use osrs_edit::Prefab;

/// A colour that stays the same for the same id.
fn colour_for(id: u32, saturation: f32, value: f32) -> Color32 {
    let hue = (id.wrapping_mul(2_654_435_761) >> 8) as f32 / (1u32 << 24) as f32;
    egui::ecolor::Hsva::new(hue, saturation, value, 1.0).into()
}

/// Paint plane 0 (and the plane above it, faintly) of `prefab` into a square of `side` points.
pub(in crate::app) fn show(ui: &mut egui::Ui, prefab: &Prefab, side: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
    let (w, h) = (f32::from(prefab.width), f32::from(prefab.height));
    let scale = (side - 4.0) / w.max(h);
    let origin = Pos2::new(
        rect.center().x - w * scale / 2.0,
        rect.center().y + h * scale / 2.0,
    );
    // Tile (x, y) has its south-west corner at `origin + (x, -y) * scale`.
    let corner = |x: f32, y: f32| Pos2::new(origin.x + x * scale, origin.y - y * scale);
    for tile in prefab.tiles.iter().filter(|t| t.plane == 0) {
        let (x, y) = (f32::from(tile.x), f32::from(tile.y));
        let colour = match (&tile.overlay, tile.underlay) {
            (Some(overlay), _) => colour_for(overlay.id, 0.35, 0.7),
            (None, Some(underlay)) => colour_for(underlay, 0.45, 0.45),
            (None, None) => continue,
        };
        painter.rect_filled(
            Rect::from_two_pos(corner(x, y), corner(x + 1.0, y + 1.0)),
            0.0,
            colour,
        );
    }
    let wall = Stroke::new(
        (scale * 0.28).clamp(1.0, 3.0),
        Color32::from_rgb(235, 225, 200),
    );
    for object in prefab.objects.iter().filter(|o| o.plane == 0) {
        let (x, y) = (f32::from(object.x), f32::from(object.y));
        match object.kind {
            // Walls sit on the edge the orientation names: west, north, east, south.
            0..=3 => {
                let (a, b) = match object.orientation & 3 {
                    0 => (corner(x, y), corner(x, y + 1.0)),
                    1 => (corner(x, y + 1.0), corner(x + 1.0, y + 1.0)),
                    2 => (corner(x + 1.0, y), corner(x + 1.0, y + 1.0)),
                    _ => (corner(x, y), corner(x + 1.0, y)),
                };
                painter.line_segment([a, b], wall);
            }
            4..=9 => {
                painter.circle_filled(
                    corner(x + 0.5, y + 0.5),
                    (scale * 0.15).max(0.8),
                    Color32::from_rgb(240, 170, 90),
                );
            }
            12..=21 => {
                painter.rect_stroke(
                    Rect::from_two_pos(corner(x, y), corner(x + 1.0, y + 1.0)),
                    0.0,
                    Stroke::new(0.6, Color32::from_rgb(190, 90, 80)),
                    egui::StrokeKind::Inside,
                );
            }
            _ => {
                painter.rect_filled(
                    Rect::from_center_size(
                        corner(x + 0.5, y + 0.5),
                        Vec2::splat((scale * 0.6).max(1.5)),
                    ),
                    0.0,
                    colour_for(object.id, 0.5, 0.95),
                );
            }
        }
    }
    painter.rect_stroke(
        Rect::from_two_pos(corner(0.0, 0.0), corner(w, h)),
        0.0,
        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );
    response
}
