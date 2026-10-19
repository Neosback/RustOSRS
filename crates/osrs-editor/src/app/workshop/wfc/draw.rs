//! Read-only symbolic canvas shared by the WFC lens and trace display.
use super::*;
use eframe::egui::{Pos2, Rect, Stroke};

/// Draw `cells` (row by row from the south) with a colour per kind and the edges of each kind.
#[allow(clippy::type_complexity)]
pub(super) fn cells(
    ui: &mut egui::Ui,
    lens: &WfcLens,
    cells: &[Option<u16>],
    width: usize,
    height: usize,
    pick: Option<&mut Option<u16>>,
    heat: Option<(&[u16], Option<(u16, u16)>)>,
) {
    let cell = ((ui.available_width().min(420.0) - 8.0) / width as f32).clamp(8.0, 30.0);
    let size = Vec2::new(cell * width as f32, cell * height as f32);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    let origin = |x: usize, y: usize| {
        Pos2::new(
            rect.left() + x as f32 * cell,
            rect.top() + (height - 1 - y) as f32 * cell,
        )
    };
    for y in 0..height {
        for x in 0..width {
            let Some(kind) = cells[y * width + x] else {
                continue;
            };
            let r = Rect::from_min_size(origin(x, y), Vec2::splat(cell));
            let mut color = kind_color(kind);
            if let Some((options, _)) = heat {
                let n = options[y * width + x];
                color = if n <= 1 {
                    kind_color(kind)
                } else {
                    Color32::from_gray(200u8.saturating_sub((n.min(20) * 6) as u8))
                };
            }
            painter.rect_filled(r.shrink(0.5), 0.0, color);
            if cell >= 14.0 {
                painter.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    kind.to_string(),
                    egui::FontId::proportional(cell * 0.45),
                    Color32::from_black_alpha(160),
                );
            }
            let edges = lens.kinds[usize::from(kind)].edges;
            let (tl, tr, bl, br) = (
                r.left_top(),
                r.right_top(),
                r.left_bottom(),
                r.right_bottom(),
            );
            for (edge, a, b) in [
                (edges[0], tl, tr),
                (edges[1], tr, br),
                (edges[2], bl, br),
                (edges[3], tl, bl),
            ] {
                match edge {
                    WfcLensEdge::Wall => {
                        painter.line_segment([a, b], Stroke::new(3.0, Color32::from_gray(40)));
                    }
                    WfcLensEdge::Door => {
                        painter.line_segment(
                            [a, b],
                            Stroke::new(3.0, Color32::from_rgb(235, 130, 20)),
                        );
                    }
                    WfcLensEdge::Window => {
                        painter.line_segment([a, b], Stroke::new(3.0, Color32::LIGHT_BLUE));
                    }
                    WfcLensEdge::Unknown => {
                        painter.line_segment([a, b], Stroke::new(3.0, Color32::LIGHT_RED));
                    }
                    WfcLensEdge::Open => {}
                }
            }
        }
    }
    if let Some((_, Some((sx, sy)))) = heat {
        let r = Rect::from_min_size(origin(usize::from(sx), usize::from(sy)), Vec2::splat(cell));
        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(3.0, Color32::RED),
            egui::StrokeKind::Inside,
        );
    }
    if let (Some(pick), Some(pos)) = (pick, response.interact_pointer_pos())
        && response.clicked()
    {
        let x = ((pos.x - rect.left()) / cell) as usize;
        let y = height - 1 - (((pos.y - rect.top()) / cell) as usize).min(height - 1);
        if x < width {
            *pick = cells[y * width + x];
        }
    }
}
