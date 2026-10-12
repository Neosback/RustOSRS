//! Small widgets shared by the dock, the drawer and the floating toolbar.

use crate::autotile::shape_triangles;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

/// How the overlay part of a tile is filled.
#[derive(Clone, Copy)]
pub enum OverlayFill {
    Color(Color32),
    /// A texture, shifted by `offset` (animated textures scroll).
    Texture {
        id: egui::TextureId,
        offset: (f32, f32),
    },
    /// An overlay that draws nothing (the map's placeholder): shown as a checkerboard, the usual
    /// sign for "transparent", with the two square colours from the colour settings.
    Transparent {
        light: Color32,
        dark: Color32,
    },
}

/// A checkerboard of `cells` x `cells` squares filling `rect`.
pub fn paint_checker(
    painter: &egui::Painter,
    rect: Rect,
    light: Color32,
    dark: Color32,
    cells: u32,
) {
    painter.rect_filled(rect, 3.0, dark);
    let size = rect.width() / cells as f32;
    for y in 0..cells {
        for x in 0..cells {
            if (x + y) % 2 == 0 {
                let square = Rect::from_min_size(
                    Pos2::new(rect.left() + x as f32 * size, rect.top() + y as f32 * size),
                    Vec2::splat(size),
                );
                painter.rect_filled(square, 0.0, light);
            }
        }
    }
}

/// Draw one tile: the underlay everywhere, the overlay in the shape's overlay faces.
pub fn paint_tile(
    painter: &egui::Painter,
    rect: Rect,
    shape: u8,
    rotation: u8,
    underlay: Option<Color32>,
    overlay: Option<OverlayFill>,
) {
    let under = underlay.unwrap_or(Color32::from_gray(70));
    painter.rect_filled(rect, 3.0, under);
    let Some(fill) = overlay else {
        return;
    };
    let uv_of = |p: [f32; 2], offset: (f32, f32)| Pos2::new(p[0] + offset.0, 1.0 - p[1] + offset.1);
    let at = |p: [f32; 2]| {
        Pos2::new(
            rect.left() + p[0] * rect.width(),
            rect.bottom() - p[1] * rect.height(),
        )
    };
    if shape == 0 {
        match fill {
            OverlayFill::Color(color) => {
                painter.rect_filled(rect, 3.0, color);
            }
            OverlayFill::Texture { id, offset } => {
                painter.image(
                    id,
                    rect,
                    Rect::from_min_max(
                        Pos2::new(offset.0, offset.1),
                        Pos2::new(offset.0 + 1.0, offset.1 + 1.0),
                    ),
                    Color32::WHITE,
                );
            }
            OverlayFill::Transparent { light, dark } => {
                paint_checker(painter, rect, light, dark, 4)
            }
        }
        return;
    }
    for (triangle, is_overlay) in shape_triangles(shape, rotation) {
        if !is_overlay {
            continue;
        }
        match fill {
            OverlayFill::Color(color) => {
                painter.add(egui::Shape::convex_polygon(
                    triangle.iter().map(|p| at(*p)).collect(),
                    color,
                    Stroke::NONE,
                ));
            }
            OverlayFill::Transparent { light, .. } => {
                // A shaped piece of "nothing": the light square colour, translucent.
                painter.add(egui::Shape::convex_polygon(
                    triangle.iter().map(|p| at(*p)).collect(),
                    light.gamma_multiply(0.7),
                    Stroke::NONE,
                ));
            }
            OverlayFill::Texture { id, offset } => {
                let mut mesh = egui::epaint::Mesh::with_texture(id);
                for p in triangle {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: at(p),
                        uv: uv_of(p, offset),
                        color: Color32::WHITE,
                    });
                }
                mesh.indices.extend([0, 1, 2]);
                painter.add(egui::Shape::mesh(mesh));
            }
        }
    }
}

/// A square swatch button; `fill` paints its contents. Returns its response.
pub fn swatch(
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

/// A dock tab: text with an accent underline when selected.
pub fn tab_label(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response {
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

/// A centred icon, title and hint for a panel with nothing to show.
pub fn empty_state(ui: &mut egui::Ui, icon: &str, title: &str, hint: &str) {
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

/// A chip row: one selectable label per option; returns the clicked index.
pub fn chips<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    current: T,
    options: &[(T, &str)],
) -> Option<T> {
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        for (value, label) in options {
            if ui.selectable_label(current == *value, *label).clicked() {
                picked = Some(*value);
            }
        }
    });
    picked
}
