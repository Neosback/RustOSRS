//! Scene -> HD geometry: the HD counterpart of [`crate::extract_owned_geometry`].
//!
//! Terrain goes through the ported `ProceduralGenerator` and `SceneUploader` terrain emission;
//! static loc models through the model-override pipeline. The result has the same zone and level
//! structure as the Reference geometry but carries the HD vertex stream.

use super::{
    HdContext,
    model::{ModelSite, model_faces},
    procedural::{ProceduralData, ProceduralInput, SceneTileInput, TileInput},
    terrain::{TerrainEmitter, TileSettings},
};
use crate::{
    WorldScene,
    extract::{OwnedTiles, effective_level, lit_model, loc_slots},
};
use osrs_core::coords::{SceneTile, StoragePlane};
use osrs_render::{ModelPlacement, SceneGeometry, ZONE_LOCAL_UNITS};
use osrs_scene::SemanticTile;

/// `TILE_FLAG_VIS_BELOW`: the floor of this tile is visible from the level below.
const TILE_FLAG_VIS_BELOW: u8 = 4;

fn tile_input<'a>(
    world: &WorldScene,
    tile: &'a SemanticTile,
    x: i32,
    y: i32,
) -> Option<TileInput<'a>> {
    let level = tile.original_plane().index().get();
    let (lx, ly) = (x as usize, y as usize);
    tile.terrain.as_ref().map(|surface| TileInput {
        x,
        y,
        level,
        surface,
        overlay_id: i32::from(world.load.overlay_raw(usize::from(level), lx, ly) & 0x7FFF),
        underlay_id: i32::from(world.load.underlay(usize::from(level), lx, ly)),
    })
}

/// Terrain of `world` as the procedural generator's input.
pub fn procedural_input(world: &WorldScene) -> ProceduralInput<'_> {
    let (width, height) = (world.scene.width() as i32, world.scene.height() as i32);
    let mut tiles = Vec::new();
    for plane_index in 0..4_u8 {
        let Some(plane) = StoragePlane::new(plane_index) else {
            continue;
        };
        for x in 0..width {
            for y in 0..height {
                let Some(tile) = world.scene.tile(plane, SceneTile::new(x as u32, y as u32)) else {
                    continue;
                };
                let main = tile_input(world, tile, x, y);
                let bridge = tile.linked_below().and_then(|below| tile_input(world, below, x, y));
                if main.is_some() {
                    tiles.push(SceneTileInput {
                        plane: plane_index,
                        tile: main,
                        bridge,
                    });
                }
            }
        }
    }
    ProceduralInput {
        size: (width, height),
        base: (world.window.base_x, world.window.base_y),
        tiles,
    }
}

/// Extract the HD geometry of the tiles in `owned` (or every tile when `None`).
pub fn extract_owned_geometry_hd(
    world: &WorldScene,
    owned: Option<OwnedTiles>,
    hd: &HdContext,
) -> SceneGeometry {
    let input = procedural_input(world);
    let data = ProceduralData::generate(&input, hd);
    let emitter = TerrainEmitter {
        ctx: hd,
        data: &data,
        base: input.base,
    };
    let mut geometry = SceneGeometry::default();
    let (zone_base_x, zone_base_z) = (world.window.base_x / 8, world.window.base_y / 8);
    let in_range = |x: u32, y: u32| owned.is_none_or(|range| range.contains(x, y));

    // Terrain: every storage tile, then its linked-below (bridge) tile.
    for slot in &input.tiles {
        let Some(main) = slot.tile else { continue };
        if !in_range(main.x as u32, main.y as u32) {
            continue;
        }
        let Some(semantic) = world.scene.tile(
            StoragePlane::new(slot.plane).unwrap_or_else(|| unreachable_plane()),
            SceneTile::new(main.x as u32, main.y as u32),
        ) else {
            continue;
        };
        let (scene_zone_x, scene_zone_z) = (main.x / 8, main.y / 8);
        let zone_origin = (scene_zone_x * ZONE_LOCAL_UNITS, scene_zone_z * ZONE_LOCAL_UNITS);
        let sources = [
            (Some(main), slot.plane, semantic.min_plane()),
            (
                slot.bridge,
                0,
                semantic.linked_below().map_or(0, SemanticTile::min_plane),
            ),
        ];
        for (tile, level, min_plane) in sources {
            let Some(tile) = tile else { continue };
            let visible_below = world
                .load
                .settings(usize::from(tile.level), tile.x as usize, tile.y as usize)
                & TILE_FLAG_VIS_BELOW
                != 0;
            let faces = emitter.tile_faces(
                tile,
                TileSettings {
                    visible_below,
                    zone_origin,
                },
            );
            if faces.is_empty() {
                continue;
            }
            let builder = geometry
                .zone_mut(zone_base_x + scene_zone_x, zone_base_z + scene_zone_z)
                .group_mut(level, min_plane);
            builder.push_hd_faces(&faces, [0, 0, 0]);
        }
    }

    // Locs.
    for loc in &world.locs {
        if loc.renderables.is_empty() || !in_range(loc.tile.x, loc.tile.y) {
            continue;
        }
        let (level, min_plane) = effective_level(world, loc.plane, loc.tile);
        let (scene_zone_x, scene_zone_z) = (loc.tile.x as i32 / 8, loc.tile.y as i32 / 8);
        let origin = (scene_zone_x * ZONE_LOCAL_UNITS, scene_zone_z * ZONE_LOCAL_UNITS);
        let world_tile = (
            world.window.base_x + loc.tile.x as i32,
            world.window.base_y + loc.tile.y as i32,
            i32::from(loc.plane.index().get()),
        );
        for slot in loc_slots(loc, origin) {
            let Some(model) = lit_model(world, slot.renderable) else {
                continue;
            };
            let faces = model_faces(
                hd,
                model,
                ModelSite {
                    object_id: loc.object_id.get(),
                    world: world_tile,
                    placement: ModelPlacement {
                        x: slot.x,
                        y: slot.y,
                        z: slot.z,
                        orientation: slot.rotation,
                    },
                    zone_origin_world: (
                        (zone_base_x + scene_zone_x) * ZONE_LOCAL_UNITS,
                        (zone_base_z + scene_zone_z) * ZONE_LOCAL_UNITS,
                    ),
                },
            );
            if faces.is_empty() {
                continue;
            }
            geometry
                .zone_mut(zone_base_x + scene_zone_x, zone_base_z + scene_zone_z)
                .group_mut(level, min_plane)
                .push_hd_faces(&faces, [slot.x, slot.y, slot.z]);
        }
    }
    geometry
}

fn unreachable_plane() -> StoragePlane {
    StoragePlane::new(0).unwrap_or_else(|| unreachable!("plane 0 exists"))
}
