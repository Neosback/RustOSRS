//! Drawing a path lesson as a picture: the ground, buildings, doors and blocked spots, with the
//! real paths, the drawn paths and the model's chances laid over it.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use osrs_lab::PathScene;

#[derive(Default)]
pub(super) struct Layers<'a> {
    /// Show the real paths (brown).
    pub(super) real: bool,
    /// The model's chance that each tile is path, drawn as red.
    pub(super) chances: Option<&'a [f32]>,
    /// Tiles the AI drew (blue).
    pub(super) drawn: Option<&'a [bool]>,
}

const GROUND: Color32 = Color32::from_rgb(112, 150, 98);
const BUILDING: Color32 = Color32::from_rgb(150, 150, 160);
const BLOCKED: Color32 = Color32::from_rgb(58, 72, 96);
const OTHER_FLOOR: Color32 = Color32::from_rgb(196, 182, 142);
const REAL: Color32 = Color32::from_rgb(168, 112, 56);
const DRAWN: Color32 = Color32::from_rgb(40, 110, 235);

/// Draw `scene` about `side` points wide. Returns the tile the pointer is over, if any.
pub(super) fn show(
    ui: &mut egui::Ui,
    scene: &PathScene,
    side: f32,
    layers: &Layers<'_>,
) -> Option<(usize, usize)> {
    let cell = (side / scene.width.max(scene.height) as f32).clamp(2.0, 12.0);
    let size = Vec2::new(cell * scene.width as f32, cell * scene.height as f32);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let corner = |x: usize, y: usize| {
        Pos2::new(
            rect.left() + x as f32 * cell,
            rect.top() + (scene.height - 1 - y) as f32 * cell,
        )
    };
    for y in 0..scene.height {
        for x in 0..scene.width {
            let a = y * scene.width + x;
            let color = if scene.inside[a] {
                BUILDING
            } else if !scene.walkable[a] {
                BLOCKED
            } else if !scene.candidate[a] {
                OTHER_FLOOR
            } else {
                GROUND
            };
            let r = Rect::from_min_size(corner(x, y), Vec2::splat(cell));
            painter.rect_filled(r, 0.0, color);
            if let Some(chances) = layers.chances
                && scene.candidate[a]
            {
                let alpha = (chances[a].clamp(0.0, 1.0) * 200.0) as u8;
                painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(235, 40, 40, alpha));
            }
            if layers.real && scene.mask[a] {
                painter.rect_filled(
                    r,
                    0.0,
                    Color32::from_rgba_unmultiplied(REAL.r(), REAL.g(), REAL.b(), 190),
                );
            }
            if let Some(drawn) = layers.drawn
                && drawn[a]
            {
                painter.rect_filled(r.shrink(cell * 0.22), 0.0, DRAWN);
            }
        }
    }
    for &(x, y) in &scene.door_fronts {
        painter.circle_filled(
            Rect::from_min_size(corner(x, y), Vec2::splat(cell)).center(),
            (cell * 0.38).max(2.0),
            Color32::from_rgb(255, 140, 20),
        );
    }
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, Color32::from_gray(30)),
        egui::StrokeKind::Inside,
    );
    let pos = response.hover_pos()?;
    let x = ((pos.x - rect.left()) / cell) as usize;
    let y_from_top = ((pos.y - rect.top()) / cell) as usize;
    (x < scene.width && y_from_top < scene.height).then(|| (x, scene.height - 1 - y_from_top))
}

/// What the colours mean, one line each.
pub(super) fn legend(ui: &mut egui::Ui, with_real: bool, with_drawn: bool, with_chances: bool) {
    let item = |ui: &mut egui::Ui, color: Color32, text: &str| {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, color);
            ui.label(text);
        });
    };
    item(ui, GROUND, "open ground");
    item(ui, BUILDING, "inside a building");
    item(ui, BLOCKED, "blocked (water, trees, rocks, walls)");
    item(ui, OTHER_FLOOR, "another kind of floor (plaza, shore)");
    item(ui, Color32::from_rgb(255, 140, 20), "just outside a door");
    if with_real {
        item(ui, REAL, "the real path");
    }
    if with_drawn {
        item(ui, DRAWN, "what the AI drew");
    }
    if with_chances {
        item(
            ui,
            Color32::from_rgb(235, 40, 40),
            "how sure the AI is that a path goes here (redder = surer)",
        );
    }
}
