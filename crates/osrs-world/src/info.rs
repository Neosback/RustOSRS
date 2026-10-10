//! Inspection data for the editor's picker: per-tile terrain facts and per-loc placement facts
//! of one streamed region, in world coordinates.

use crate::{
    LocRenderable, WorldScene,
    extract::{effective_level, lit_model, loc_slots},
};
use osrs_core::{
    coords::{RegionCoord, SceneTile, StoragePlane},
    definitions::ObjectDefinition,
    lighting::ReferenceLitModel,
};
use osrs_scene::{placement::PlacementPlan, terrain::TerrainSurface};
use std::{collections::HashMap, sync::Arc};

const REGION_TILES: usize = 64;
const PLANES: usize = 4;
const CORNERS: usize = REGION_TILES + 1;
/// Terrain colour sentinel the builder uses for a hidden face.
const SKIPPED: i32 = 12_345_678;

/// What the terrain builder produced for one tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SurfaceKind {
    #[default]
    None,
    /// `SceneTilePaint`.
    Flat,
    /// `SceneTileModel`.
    Shaped,
}

/// Terrain facts of one tile on one storage plane.
#[derive(Debug, Clone, Copy, Default)]
pub struct TileInfo {
    /// Corner heights `[sw, se, ne, nw]`.
    pub heights: [i32; 4],
    pub underlay: u16,
    pub overlay_raw: u16,
    pub shape: u8,
    pub rotation: u8,
    pub settings: u8,
    pub shadow: u8,
    pub min_plane: u8,
    pub surface: SurfaceKind,
    pub faces: u8,
    pub skipped_faces: u8,
    pub has_linked_below: bool,
}

/// One drawn model slot of a loc.
#[derive(Debug, Clone)]
pub struct SlotInfo {
    pub kind: &'static str,
    /// World-space axis-aligned bounds `[x, height, z]` in local units (128 per tile).
    pub min: [i32; 3],
    pub max: [i32; 3],
    /// Model origin `[x, height, z]` in world local units.
    pub origin: [i32; 3],
    /// Extra instance rotation in JAU.
    pub rotation: u16,
    pub vertices: usize,
    pub faces: usize,
}

/// One placed loc.
#[derive(Debug, Clone)]
pub struct LocInfo {
    pub definition: Arc<ObjectDefinition>,
    pub loc_type: u8,
    pub orientation: u8,
    /// Source (map file) plane.
    pub plane: u8,
    /// Draw level after bridge relinking, and the tile's minimum plane.
    pub level: u8,
    pub min_plane: u8,
    /// World tile of the placement anchor.
    pub tile: (i32, i32),
    pub plan: PlacementPlan,
    pub slots: Vec<SlotInfo>,
}

/// Everything the picker needs for one region.
#[derive(Debug)]
pub struct RegionInfo {
    pub region: RegionCoord,
    /// `plane * 64 * 64 + local_x * 64 + local_y`.
    tiles: Vec<TileInfo>,
    /// Corner heights, `plane * 65 * 65 + local_x * 65 + local_y`.
    heights: Vec<i32>,
    pub locs: Vec<LocInfo>,
    by_tile: HashMap<(i32, i32), Vec<u32>>,
}

impl RegionInfo {
    fn local(&self, tile: (i32, i32)) -> Option<(usize, usize)> {
        let local_x = tile.0 - self.region.x * 64;
        let local_y = tile.1 - self.region.y * 64;
        ((0..64).contains(&local_x) && (0..64).contains(&local_y))
            .then_some((local_x as usize, local_y as usize))
    }

    pub fn tile(&self, plane: usize, tile: (i32, i32)) -> Option<&TileInfo> {
        let (x, y) = self.local(tile)?;
        self.tiles
            .get((plane * REGION_TILES + x) * REGION_TILES + y)
    }

    /// Terrain height at a corner of the region (`x`, `y` in `0..=64`).
    pub fn corner_height(&self, plane: usize, x: usize, y: usize) -> i32 {
        self.heights[(plane * CORNERS + x) * CORNERS + y]
    }

    /// Bilinear terrain height at a world position in local units, on `plane`.
    pub fn height_at(&self, plane: usize, world_x: f32, world_z: f32) -> Option<f32> {
        let local_x = world_x / 128.0 - (self.region.x * 64) as f32;
        let local_y = world_z / 128.0 - (self.region.y * 64) as f32;
        if !(0.0..64.0).contains(&local_x) || !(0.0..64.0).contains(&local_y) {
            return None;
        }
        let (x0, y0) = (local_x as usize, local_y as usize);
        let (fx, fy) = (local_x - x0 as f32, local_y - y0 as f32);
        let h = |x: usize, y: usize| self.corner_height(plane, x, y) as f32;
        let south = h(x0, y0) * (1.0 - fx) + h(x0 + 1, y0) * fx;
        let north = h(x0, y0 + 1) * (1.0 - fx) + h(x0 + 1, y0 + 1) * fx;
        Some(south * (1.0 - fy) + north * fy)
    }

    /// Locs anchored on a world tile (all planes).
    pub fn locs_at(&self, tile: (i32, i32)) -> impl Iterator<Item = &LocInfo> {
        self.by_tile
            .get(&tile)
            .into_iter()
            .flatten()
            .filter_map(|index| self.locs.get(*index as usize))
    }
}

fn model_bounds(
    model: &ReferenceLitModel,
    rotation: u16,
    origin: [i32; 3],
) -> ([i32; 3], [i32; 3]) {
    let (sin, cos) = if rotation == 0 {
        (0, 0)
    } else {
        let tables = osrs_core::trig::trig_tables();
        let index = usize::from(rotation) & 2047;
        (tables.sine(index), tables.cosine(index))
    };
    let mut min = [i32::MAX; 3];
    let mut max = [i32::MIN; 3];
    for vertex in &model.vertices {
        let (mut x, y, mut z) = (vertex.x, vertex.y, vertex.z);
        if rotation != 0 {
            let original_x = x;
            x = (z
                .wrapping_mul(sin)
                .wrapping_add(original_x.wrapping_mul(cos)))
                >> 16;
            z = (z
                .wrapping_mul(cos)
                .wrapping_sub(original_x.wrapping_mul(sin)))
                >> 16;
        }
        let point = [x + origin[0], y + origin[1], z + origin[2]];
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    if min[0] > max[0] {
        return (origin, origin);
    }
    (min, max)
}

/// Collect inspection data for the tiles of `world` that lie in `owned` (scene tiles
/// `[min, max)` on both axes).
pub fn extract_region_info(
    world: &WorldScene,
    region: RegionCoord,
    owned: ((u32, u32), (u32, u32)),
    object: &mut dyn FnMut(osrs_core::ids::ObjectId) -> Option<Arc<ObjectDefinition>>,
) -> RegionInfo {
    let (base_x, base_y) = (world.window.base_x, world.window.base_y);
    let in_range =
        |x: u32, y: u32| x >= owned.0.0 && x < owned.1.0 && y >= owned.0.1 && y < owned.1.1;

    let mut tiles = vec![TileInfo::default(); PLANES * REGION_TILES * REGION_TILES];
    let mut heights = vec![0; PLANES * CORNERS * CORNERS];
    for plane in 0..PLANES {
        let storage = StoragePlane::new(plane as u8);
        for local_x in 0..CORNERS {
            for local_y in 0..CORNERS {
                heights[(plane * CORNERS + local_x) * CORNERS + local_y] = world.load.height(
                    plane,
                    owned.0.0 as usize + local_x,
                    owned.0.1 as usize + local_y,
                );
            }
        }
        for local_x in 0..REGION_TILES {
            for local_y in 0..REGION_TILES {
                let (sx, sy) = (owned.0.0 as usize + local_x, owned.0.1 as usize + local_y);
                let h = |dx: usize, dy: usize| world.load.height(plane, sx + dx, sy + dy);
                let mut info = TileInfo {
                    heights: [h(0, 0), h(1, 0), h(1, 1), h(0, 1)],
                    underlay: world.load.underlay(plane, sx, sy),
                    overlay_raw: world.load.overlay_raw(plane, sx, sy),
                    shape: world.load.shape(plane, sx, sy),
                    rotation: world.load.rotation(plane, sx, sy),
                    settings: world.load.settings(plane, sx, sy),
                    shadow: world.load.shadow(plane, sx, sy),
                    ..TileInfo::default()
                };
                if let Some(semantic) = storage.and_then(|plane| {
                    world
                        .scene
                        .tile(plane, SceneTile::new(sx as u32, sy as u32))
                }) {
                    info.min_plane = semantic.min_plane();
                    info.has_linked_below = semantic.linked_below().is_some();
                    match &semantic.terrain {
                        Some(TerrainSurface::Flat(flat)) => {
                            info.surface = SurfaceKind::Flat;
                            info.faces = 2;
                            info.skipped_faces = u8::from(flat.colors.northeast == SKIPPED) * 2;
                        }
                        Some(TerrainSurface::Shaped(shaped)) => {
                            info.surface = SurfaceKind::Shaped;
                            info.faces = shaped.faces.len() as u8;
                            info.skipped_faces = shaped
                                .faces
                                .iter()
                                .filter(|face| face.colors[0] == SKIPPED)
                                .count() as u8;
                        }
                        None => {}
                    }
                }
                tiles[(plane * REGION_TILES + local_x) * REGION_TILES + local_y] = info;
            }
        }
    }

    let mut locs = Vec::new();
    let mut by_tile: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
    for loc in &world.locs {
        if loc.renderables.is_empty() || !in_range(loc.tile.x, loc.tile.y) {
            continue;
        }
        let Some(definition) = object(loc.object_id) else {
            continue;
        };
        let (level, min_plane) = effective_level(world, loc.plane, loc.tile);
        let mut slots = Vec::new();
        for slot in loc_slots(loc, (0, 0)) {
            let model = match slot.renderable {
                LocRenderable::Animated(index) => world.animated.get(index).map(|m| &*m.base),
                other => lit_model(world, other),
            };
            let origin = [base_x * 128 + slot.x, slot.y, base_y * 128 + slot.z];
            let (min, max, vertices, faces) = match model {
                Some(model) => {
                    let (min, max) = model_bounds(model, slot.rotation, origin);
                    (min, max, model.vertices.len(), model.faces.len())
                }
                None => (origin, origin, 0, 0),
            };
            slots.push(SlotInfo {
                kind: match slot.renderable {
                    LocRenderable::Lit(_) => "lit",
                    LocRenderable::ModelData(_) => "merged-normals",
                    LocRenderable::Animated(_) => "animated",
                    LocRenderable::Omitted => "omitted",
                },
                min,
                max,
                origin,
                rotation: slot.rotation,
                vertices,
                faces,
            });
        }
        let tile = (base_x + loc.tile.x as i32, base_y + loc.tile.y as i32);
        by_tile.entry(tile).or_default().push(locs.len() as u32);
        locs.push(LocInfo {
            definition,
            loc_type: loc.loc_type,
            orientation: loc.orientation,
            plane: loc.plane.index().get(),
            level,
            min_plane,
            tile,
            plan: loc.plan,
            slots,
        });
    }

    RegionInfo {
        region,
        tiles,
        heights,
        locs,
        by_tile,
    }
}
