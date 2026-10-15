//! Where the camera is, and moving it: teleports, focus, rebasing the render origin.

use super::*;

impl EditorApp {
    /// World position of the camera in local units.
    pub(super) fn world_position(&self) -> (f64, f64) {
        let origin = self.renderer.render_origin();
        (
            f64::from(origin.0) + f64::from(self.interaction.camera().x),
            f64::from(origin.1) + f64::from(self.interaction.camera().z),
        )
    }

    /// Point the camera at the selected object (orbit pivot in orbit mode).
    pub(super) fn focus_selection(&mut self) {
        let Some(object) = &self.interaction.selected() else {
            return;
        };
        let Some(info) = self.world.infos().get(&object.region) else {
            return;
        };
        let Some(loc) = info.locs.get(object.index) else {
            return;
        };
        let origin = self.renderer.render_origin();
        let world = (
            (loc.tile.0 * 128 + 64) as f32,
            (loc.tile.1 * 128 + 64) as f32,
        );
        let ground = info
            .height_at(usize::from(loc.plane), world.0, world.1)
            .unwrap_or(0.0);
        let extent = loc
            .slots
            .iter()
            .flat_map(|slot| (0..3).map(move |a| (slot.max[a] - slot.min[a]) as f32))
            .fold(0.0_f32, f32::max);
        self.interaction.focus_camera(
            [
                world.0 - origin.0 as f32,
                ground - 64.0,
                world.1 - origin.1 as f32,
            ],
            (extent * 1.8).clamp(500.0, 4000.0),
        );
    }

    /// World tile under the camera.
    pub(super) fn camera_tile(&self) -> (i32, i32) {
        let (x, z) = self.world_position();
        ((x / 128.0).floor() as i32, (z / 128.0).floor() as i32)
    }

    pub(super) fn open_teleport(&mut self) {
        let tile = self.camera_tile();
        self.ui
            .teleport
            .prefill(tile, self.interaction.view_plane());
        self.ui.teleport.open = true;
    }

    /// Move the camera above `destination` and rebase the render origin onto it.
    pub(super) fn teleport_to(&mut self, destination: Destination) {
        let (tile_x, tile_y, plane) = match destination {
            Destination::Tile { x, y, plane } => (x, y, plane),
            Destination::Region { x, y } => {
                (x * 64 + 32, y * 64 + 32, self.interaction.view_plane())
            }
        };
        let world_x = tile_x * 128 + 64;
        let world_z = tile_y * 128 + 64;
        let origin = (
            (world_x as f32 / 1024.0).round() as i32 * 1024,
            (world_z as f32 / 1024.0).round() as i32 * 1024,
        );
        self.renderer.set_render_origin(origin);
        self.interaction.teleport_camera(
            [
                (world_x - origin.0) as f32,
                -(1800.0 + 240.0 * f32::from(plane)),
                (world_z - origin.1) as f32,
            ],
            plane,
        );
        self.world.mark_animation_dirty();
    }

    pub(super) fn camera_region(&self) -> (i32, i32) {
        let (x, z) = self.world_position();
        (
            (x / f64::from(REGION_UNITS)).floor() as i32,
            (z / f64::from(REGION_UNITS)).floor() as i32,
        )
    }

    /// Keep the render origin near the camera so f32 stays precise far from the map origin.
    pub(super) fn rebase_if_needed(&mut self) {
        let far = self
            .interaction
            .camera()
            .x
            .abs()
            .max(self.interaction.camera().z.abs());
        if far < REBASE_DISTANCE {
            return;
        }
        let (world_x, world_z) = self.world_position();
        let new_origin = (
            (world_x / 1024.0).round() as i32 * 1024,
            (world_z / 1024.0).round() as i32 * 1024,
        );
        let old = self.renderer.render_origin();
        self.renderer.set_render_origin(new_origin);
        self.interaction
            .shift_camera_origin([(old.0 - new_origin.0) as f32, (old.1 - new_origin.1) as f32]);
    }
}
