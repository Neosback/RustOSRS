//! Minimap HUD: the compass (and, below, the minimap proper).

use super::EditorApp;
use crate::assets::sprite;
use eframe::egui;

impl EditorApp {
    /// Compass HUD: the game's compass sprite, turned with the camera, in the viewport's corner.
    pub(super) fn compass_hud(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        if !self.prefs.show_compass {
            return;
        }
        let yaw = self.camera.yaw;
        let Some(texture) = self.assets.sprite(sprite::COMPASS) else {
            return;
        };
        let texture_id = texture.id();
        let size = texture.size_vec2() * 2.0;
        let center = rect.right_top() + egui::vec2(-size.x * 0.5 - 14.0, size.y * 0.5 + 14.0);
        let response = ui.interact(
            egui::Rect::from_center_size(center, size),
            egui::Id::new("compass_hud"),
            egui::Sense::click(),
        );
        // A sprite turned about its centre: build the rotated quad by hand.
        let (sin, cos) = yaw.sin_cos();
        let corner = |x: f32, y: f32| {
            let (dx, dy) = (x * size.x * 0.5, y * size.y * 0.5);
            center + egui::vec2(dx * cos + dy * sin, -dx * sin + dy * cos)
        };
        let mut mesh = egui::epaint::Mesh::with_texture(texture_id);
        let uv = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        for (index, (x, y)) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .into_iter()
            .enumerate()
        {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: corner(x, y),
                uv: egui::pos2(uv[index].0, uv[index].1),
                color: egui::Color32::WHITE,
            });
        }
        mesh.indices.extend([0, 1, 2, 0, 2, 3]);
        ui.painter()
            .with_clip_rect(rect)
            .add(egui::Shape::mesh(mesh));
        if response.on_hover_text("Face north").clicked() {
            self.camera.yaw = 0.0;
        }
    }
}
