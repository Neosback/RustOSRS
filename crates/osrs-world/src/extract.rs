//! Scene -> renderer geometry extraction.
//!
//! Walks the assembled scene the way `SceneUploader.uploadZoneTile` does (terrain paint/model,
//! boundaries, wall decorations, floor decorations, game objects, then the linked-below bridge
//! tile) and emits zone-local packed geometry. Draw positions follow the pinned client:
//! boundary, floor decoration, and game object at their storage centers, wall decorations at the
//! center plus the nudged offset, game objects with their 256-JAU instance rotation when diagonal.

use crate::{AnimatedInstance, LocRenderable, TextureTable, WorldLoc, WorldScene};
use osrs_core::{
    coords::{SceneTile, StoragePlane},
    lighting::ReferenceLitModel,
};
use osrs_render::{ModelPlacement, SceneGeometry, ZONE_LOCAL_UNITS};
use osrs_scene::placement::PlacementKind;

/// Inclusive-exclusive tile range `[min, max)` in scene coordinates that a window owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnedTiles {
    pub min: (u32, u32),
    pub max: (u32, u32),
}

impl OwnedTiles {
    fn contains(self, x: u32, y: u32) -> bool {
        x >= self.min.0 && x < self.max.0 && y >= self.min.1 && y < self.max.1
    }
}

/// Extract all static geometry of `world`. Zones are keyed by world zone coordinates.
pub fn extract_scene_geometry(world: &WorldScene) -> SceneGeometry {
    extract_owned_geometry(world, None)
}

/// Extract the geometry of the tiles in `owned` (or every tile when `None`).
///
/// Windows used for streaming overlap their neighbours so every owned tile has full blend and
/// normal-merge context; only the owned tiles' terrain and the locs anchored on them are emitted.
pub fn extract_owned_geometry(world: &WorldScene, owned: Option<OwnedTiles>) -> SceneGeometry {
    let mut geometry = SceneGeometry::default();
    let (zone_base_x, zone_base_z) = (world.window.base_x / 8, world.window.base_y / 8);
    let in_range = |x: u32, y: u32| owned.is_none_or(|range| range.contains(x, y));

    // Terrain: every storage tile, then its linked-below (bridge) tile.
    for plane_index in 0..4_u8 {
        let Some(plane) = StoragePlane::new(plane_index) else {
            continue;
        };
        for x in 0..world.scene.width() {
            for y in 0..world.scene.height() {
                if !in_range(x, y) {
                    continue;
                }
                let Some(tile) = world.scene.tile(plane, SceneTile::new(x, y)) else {
                    continue;
                };
                let mut sources = vec![(tile, plane_index)];
                if let Some(below) = tile.linked_below() {
                    sources.push((below, 0));
                }
                for (source, level) in sources {
                    if let Some(surface) = &source.terrain {
                        let (scene_zone_x, scene_zone_z) = (x as i32 / 8, y as i32 / 8);
                        let zone = geometry
                            .zone_mut(zone_base_x + scene_zone_x, zone_base_z + scene_zone_z);
                        let origin = (
                            scene_zone_x * ZONE_LOCAL_UNITS,
                            scene_zone_z * ZONE_LOCAL_UNITS,
                        );
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

    // Locs.
    for loc in &world.locs {
        if loc.renderables.is_empty() || !in_range(loc.tile.x, loc.tile.y) {
            continue;
        }
        let (level, min_plane) = effective_level(world, loc.plane, loc.tile);
        let (scene_zone_x, scene_zone_z) = (loc.tile.x as i32 / 8, loc.tile.y as i32 / 8);
        let origin = (
            scene_zone_x * ZONE_LOCAL_UNITS,
            scene_zone_z * ZONE_LOCAL_UNITS,
        );
        let slots = loc_slots(loc, origin);
        if slots
            .iter()
            .all(|slot| lit_model(world, slot.renderable).is_none())
        {
            continue;
        }
        let zone = geometry.zone_mut(zone_base_x + scene_zone_x, zone_base_z + scene_zone_z);
        let builder = zone.group_mut(level, min_plane);
        for slot in slots {
            if let Some(model) = lit_model(world, slot.renderable) {
                builder.push_model(
                    model,
                    ModelPlacement {
                        x: slot.x,
                        y: slot.y,
                        z: slot.z,
                        orientation: slot.rotation,
                    },
                );
            }
        }
    }
    geometry
}

/// Level and minimum plane of the tile a loc ended up on after bridge relinking.
pub(crate) fn effective_level(world: &WorldScene, source: StoragePlane, tile: SceneTile) -> (u8, u8) {
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

pub(crate) fn lit_model(world: &WorldScene, renderable: LocRenderable) -> Option<&ReferenceLitModel> {
    match renderable {
        LocRenderable::Lit(index) => world.lit.get(index),
        LocRenderable::ModelData(id) => world.finalizer.lit_model(id),
        LocRenderable::Animated(_) | LocRenderable::Omitted => None,
    }
}

/// One drawn model slot of a loc, positioned relative to its owning zone origin.
pub(crate) struct Slot {
    pub(crate) renderable: LocRenderable,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) z: i32,
    pub(crate) rotation: u16,
}

/// Draw positions of a loc's renderables (boundary/floor decoration/game object at the storage
/// center, wall decorations at the center plus the nudged offset).
pub(crate) fn loc_slots(loc: &WorldLoc, origin: (i32, i32)) -> Vec<Slot> {
    let center = loc.plan.storage_center;
    let base_x = center.x.units() - origin.0;
    let base_y = center.y.units();
    let base_z = center.z.units() - origin.1;
    let slot = |renderable, x, y, z, rotation| Slot {
        renderable,
        x,
        y,
        z,
        rotation,
    };
    match loc.plan.kind {
        PlacementKind::Boundary(_) => loc
            .renderables
            .iter()
            .map(|renderable| slot(*renderable, base_x, base_y, base_z, 0))
            .collect(),
        PlacementKind::FloorDecoration(_) => loc
            .renderables
            .first()
            .map(|renderable| slot(*renderable, base_x, base_y, base_z, 0))
            .into_iter()
            .collect(),
        PlacementKind::GameObject(game) => loc
            .renderables
            .first()
            .map(|renderable| slot(*renderable, base_x, base_y, base_z, game.insertion_flag))
            .into_iter()
            .collect(),
        PlacementKind::WallDecoration(decor) => {
            let (nudge_x, nudge_z) = match decor.orientation_flag {
                1 => (1, 0),
                2 => (0, -1),
                4 => (-1, 0),
                8 => (0, 1),
                _ => (0, 0),
            };
            let mut slots = Vec::new();
            if let Some(renderable) = loc.renderables.first() {
                slots.push(slot(
                    *renderable,
                    base_x + decor.offset_x + nudge_x,
                    base_y,
                    base_z + decor.offset_z + nudge_z,
                    0,
                ));
            }
            // The second slot is drawn at the plain center, and only for the 256 form.
            if decor.orientation_flag == 256
                && let Some(renderable) = loc.renderables.get(1)
            {
                slots.push(slot(*renderable, base_x, base_y, base_z, 0));
            }
            slots
        }
    }
}

/// Animated model instances of the tiles in `owned` (or every tile when `None`).
pub fn extract_animated_instances(
    world: &WorldScene,
    owned: Option<OwnedTiles>,
) -> Vec<AnimatedInstance> {
    let (zone_base_x, zone_base_z) = (world.window.base_x / 8, world.window.base_y / 8);
    let mut instances = Vec::new();
    for loc in &world.locs {
        if owned.is_some_and(|range| !range.contains(loc.tile.x, loc.tile.y)) {
            continue;
        }
        let (level, min_plane) = effective_level(world, loc.plane, loc.tile);
        let (scene_zone_x, scene_zone_z) = (loc.tile.x as i32 / 8, loc.tile.y as i32 / 8);
        let origin = (
            scene_zone_x * ZONE_LOCAL_UNITS,
            scene_zone_z * ZONE_LOCAL_UNITS,
        );
        for slot in loc_slots(loc, origin) {
            let LocRenderable::Animated(index) = slot.renderable else {
                continue;
            };
            let Some(model) = world.animated.get(index) else {
                continue;
            };
            instances.push(AnimatedInstance {
                model: model.clone(),
                zone: (zone_base_x + scene_zone_x, zone_base_z + scene_zone_z),
                level,
                min_plane,
                local: (slot.x, slot.y, slot.z),
                rotation: slot.rotation,
            });
        }
    }
    instances
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
