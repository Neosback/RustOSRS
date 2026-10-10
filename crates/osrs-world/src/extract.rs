//! Scene -> renderer geometry extraction.
//!
//! Walks the assembled scene the way `SceneUploader.uploadZoneTile` does (terrain paint/model,
//! boundaries, wall decorations, floor decorations, game objects, then the linked-below bridge
//! tile) and emits zone-local packed geometry. Draw positions follow the pinned client:
//! boundary, floor decoration, and game object at their storage centers, wall decorations at the
//! center plus the nudged offset, game objects with their 256-JAU instance rotation when diagonal.

use crate::{LocRenderable, TextureTable, WorldScene};
use osrs_core::{
    coords::{SceneTile, StoragePlane},
    lighting::ReferenceLitModel,
};
use osrs_render::{GeometryBuilder, ModelPlacement, SceneGeometry, ZONE_LOCAL_UNITS};
use osrs_scene::placement::PlacementKind;

/// Extract all static geometry of `world`.
pub fn extract_scene_geometry(world: &WorldScene) -> SceneGeometry {
    let mut geometry = SceneGeometry::default();
    let scene_tiles = world.scene.width() as i32;

    // Terrain: every storage tile, then its linked-below (bridge) tile.
    for plane_index in 0..4_u8 {
        let Some(plane) = StoragePlane::new(plane_index) else {
            continue;
        };
        for x in 0..world.scene.width() {
            for y in 0..world.scene.height() {
                let Some(tile) = world.scene.tile(plane, SceneTile::new(x, y)) else {
                    continue;
                };
                let mut sources = vec![(tile, plane_index)];
                if let Some(below) = tile.linked_below() {
                    sources.push((below, 0));
                }
                for (source, level) in sources {
                    if let Some(surface) = &source.terrain {
                        let (zone_x, zone_z) = (x as i32 / 8, y as i32 / 8);
                        let zone = geometry.zone_mut(zone_x, zone_z);
                        let origin = zone.origin();
                        zone.group_mut(level, source.min_plane()).push_terrain(
                            surface,
                            (x as i32, y as i32),
                            origin,
                        );
                    }
                }
            }
        }
    }
    let _ = scene_tiles;

    // Locs.
    for loc in &world.locs {
        if loc.renderables.is_empty() {
            continue;
        }
        let (level, min_plane) = effective_level(world, loc.plane, loc.tile);
        let (zone_x, zone_z) = (loc.tile.x as i32 / 8, loc.tile.y as i32 / 8);
        let zone = geometry.zone_mut(zone_x, zone_z);
        let origin = zone.origin();
        let builder = zone.group_mut(level, min_plane);

        let center = loc.plan.storage_center;
        let base_x = center.x.units() - origin.0;
        let base_y = center.y.units();
        let base_z = center.z.units() - origin.1;

        match loc.plan.kind {
            PlacementKind::Boundary(_) => {
                for renderable in &loc.renderables {
                    emit(builder, world, *renderable, base_x, base_y, base_z, 0);
                }
            }
            PlacementKind::FloorDecoration(_) => {
                if let Some(renderable) = loc.renderables.first() {
                    emit(builder, world, *renderable, base_x, base_y, base_z, 0);
                }
            }
            PlacementKind::GameObject(game) => {
                if let Some(renderable) = loc.renderables.first() {
                    emit(
                        builder,
                        world,
                        *renderable,
                        base_x,
                        base_y,
                        base_z,
                        game.insertion_flag,
                    );
                }
            }
            PlacementKind::WallDecoration(decor) => {
                let (nudge_x, nudge_z) = match decor.orientation_flag {
                    1 => (1, 0),
                    2 => (0, -1),
                    4 => (-1, 0),
                    8 => (0, 1),
                    _ => (0, 0),
                };
                if let Some(renderable) = loc.renderables.first() {
                    emit(
                        builder,
                        world,
                        *renderable,
                        base_x + decor.offset_x + nudge_x,
                        base_y,
                        base_z + decor.offset_z + nudge_z,
                        0,
                    );
                }
                // The second slot is drawn at the plain center, and only for the 256 form.
                if decor.orientation_flag == 256
                    && let Some(renderable) = loc.renderables.get(1)
                {
                    emit(builder, world, *renderable, base_x, base_y, base_z, 0);
                }
            }
        }
    }
    let _ = ZONE_LOCAL_UNITS;
    geometry
}

/// Level and minimum plane of the tile a loc ended up on after bridge relinking.
fn effective_level(world: &WorldScene, source: StoragePlane, tile: SceneTile) -> (u8, u8) {
    let bridge = world.load.settings(1, tile.x as usize, tile.y as usize) & 2 != 0;
    let plane = source.index().get();
    let level = if bridge && plane > 0 {
        plane - 1
    } else {
        plane
    };
    let min_plane = StoragePlane::new(level)
        .and_then(|storage| world.scene.tile(storage, tile))
        .map_or(level, |semantic| semantic.min_plane());
    (level, min_plane)
}

fn lit_model(world: &WorldScene, renderable: LocRenderable) -> Option<&ReferenceLitModel> {
    match renderable {
        LocRenderable::Lit(index) => world.lit.get(index),
        LocRenderable::ModelData(id) => world.finalizer.lit_model(id),
        LocRenderable::Animated => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn emit(
    builder: &mut GeometryBuilder,
    world: &WorldScene,
    renderable: LocRenderable,
    x: i32,
    y: i32,
    z: i32,
    orientation: u16,
) {
    if let Some(model) = lit_model(world, renderable) {
        builder.push_model(
            model,
            ModelPlacement {
                x,
                y,
                z,
                orientation,
            },
        );
    }
}

/// Convert decoded texture images into renderer texture layers (layer index = texture id).
pub fn texture_layers(table: &TextureTable) -> Vec<Option<osrs_render::gpu::TextureLayer>> {
    table
        .images
        .iter()
        .map(|image| {
            image
                .as_ref()
                .map(|texture| osrs_render::gpu::TextureLayer {
                    rgba: texture.rgba.clone(),
                    animation_direction: texture.animation_direction,
                    animation_speed: texture.animation_speed,
                })
        })
        .collect()
}
