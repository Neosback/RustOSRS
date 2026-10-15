//! Projection-only Height dependency extraction for one streamed producer region.
//!
//! The semantic editor remains authoritative. This module records which source-map Height
//! vertices can affect each region-owned 8x8 output zone after the full contextual scene has been
//! assembled. Consumers may narrow projection invalidation only when `complete` is true.

use crate::{LocRenderable, OwnedTiles, WorldScene, extract::lit_model};
use osrs_core::{
    contour::model_xz_radius,
    coords::{MapTile, SourcePlane},
};
use std::collections::{BTreeMap, BTreeSet};

const ZONE_SIZE_TILES: i32 = 8;
const TILE_UNITS: i32 = 128;

pub type HeightOutputZone = (i32, i32);

/// One source-map terrain-height corner read by a disposable world projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeightSource {
    pub corner: MapTile,
    pub plane: SourcePlane,
}

impl HeightSource {
    const fn new(corner: MapTile, plane: SourcePlane) -> Self {
        Self { corner, plane }
    }
}

/// Region-complete inverse read-set produced from one fully assembled contextual scene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeightDependencySnapshot {
    pub complete: bool,
    pub by_source: BTreeMap<HeightSource, BTreeSet<HeightOutputZone>>,
}

impl Default for HeightDependencySnapshot {
    fn default() -> Self {
        Self {
            complete: true,
            by_source: BTreeMap::new(),
        }
    }
}

impl HeightDependencySnapshot {
    fn record(&mut self, source: HeightSource, output: HeightOutputZone) {
        self.by_source.entry(source).or_default().insert(output);
    }

    fn mark_incomplete(&mut self) {
        self.complete = false;
    }
}

/// Extract the Height read-set for all outputs whose anchor tiles are in `owned`.
///
/// Terrain dependencies are represented as the union of each output zone's direct corner and
/// one-grid-point slope-light reads. Static lit models conservatively use their actual final X/Z
/// cylinder radius as a superset of any contour samples. Placement-height midpoint samples are
/// recorded exactly. Scene-local `ModelData` normal reconciliation and posed animated contouring
/// are intentionally fail-closed for now: their presence marks the snapshot incomplete so the
/// editor retains region invalidation rather than guessing a dependency bound.
pub(crate) fn extract_height_dependencies(
    world: &WorldScene,
    owned: OwnedTiles,
) -> HeightDependencySnapshot {
    let mut snapshot = HeightDependencySnapshot::default();
    record_terrain_dependencies(world, owned, &mut snapshot);
    record_loc_dependencies(world, owned, &mut snapshot);
    snapshot
}

fn record_terrain_dependencies(
    world: &WorldScene,
    owned: OwnedTiles,
    snapshot: &mut HeightDependencySnapshot,
) {
    let output_zones = owned_output_zones(world, owned);
    let width = world.scene.width() as i32;
    let height = world.scene.height() as i32;

    for plane_index in 0..4_u8 {
        let Some(plane) = SourcePlane::new(plane_index) else {
            snapshot.mark_incomplete();
            continue;
        };
        for output in &output_zones {
            let world_min_x = output.0 * ZONE_SIZE_TILES;
            let world_min_y = output.1 * ZONE_SIZE_TILES;
            let world_max_x = world_min_x + ZONE_SIZE_TILES - 1;
            let world_max_y = world_min_y + ZONE_SIZE_TILES - 1;
            let owned_min_x = world.window.base_x + owned.min.0 as i32;
            let owned_min_y = world.window.base_y + owned.min.1 as i32;
            let owned_max_x = world.window.base_x + owned.max.0 as i32 - 1;
            let owned_max_y = world.window.base_y + owned.max.1 as i32 - 1;
            let min_x = world_min_x.max(owned_min_x) - world.window.base_x;
            let min_y = world_min_y.max(owned_min_y) - world.window.base_y;
            let max_x = world_max_x.min(owned_max_x) - world.window.base_x;
            let max_y = world_max_y.min(owned_max_y) - world.window.base_y;

            // Each terrain tile consumes its four corners. Each corner light additionally samples
            // one grid point in each cardinal axis, so the union for a tile span is [-1, +2].
            for source_x in (min_x - 1).max(0)..=(max_x + 2).min(width) {
                for source_y in (min_y - 1).max(0)..=(max_y + 2).min(height) {
                    record_local_source(world, snapshot, plane, source_x, source_y, *output);
                }
            }
        }
    }
}

fn record_loc_dependencies(
    world: &WorldScene,
    owned: OwnedTiles,
    snapshot: &mut HeightDependencySnapshot,
) {
    for loc in &world.locs {
        if loc.renderables.is_empty() || !owned.contains(loc.tile.x, loc.tile.y) {
            continue;
        }
        let Some(plane) = SourcePlane::new(loc.plane.index().get()) else {
            snapshot.mark_incomplete();
            continue;
        };
        let output = world_zone(
            world.window.base_x + loc.tile.x as i32,
            world.window.base_y + loc.tile.y as i32,
        );
        let mut has_output = false;

        for renderable in &loc.renderables {
            match *renderable {
                LocRenderable::Lit(_) => {
                    has_output = true;
                    let Some(model) = lit_model(world, *renderable) else {
                        snapshot.mark_incomplete();
                        continue;
                    };
                    record_static_model_samples(world, snapshot, plane, output, loc, model);
                }
                LocRenderable::ModelData(_) => {
                    has_output = true;
                    // ModelData can merge normals with neighbouring scene entities. A Height-driven
                    // geometry change can therefore change another model's final lighting. H4 can
                    // add an explicit reconciliation dependency closure; until then fail closed.
                    snapshot.mark_incomplete();
                    if let Some(model) = lit_model(world, *renderable) {
                        record_static_model_samples(world, snapshot, plane, output, loc, model);
                    }
                }
                LocRenderable::Animated(index) => {
                    has_output = true;
                    let Some(model) = world.animated.get(index) else {
                        snapshot.mark_incomplete();
                        continue;
                    };
                    if model.contour.is_some() {
                        // Posing can change X/Z bounds before contouring, so the static base radius
                        // is not a proof of the active frame's read-set.
                        snapshot.mark_incomplete();
                    }
                }
                LocRenderable::Omitted => {}
            }
        }

        if has_output {
            record_placement_samples(world, snapshot, plane, output, loc);
        }
    }
}

fn record_placement_samples(
    world: &WorldScene,
    snapshot: &mut HeightDependencySnapshot,
    plane: SourcePlane,
    output: HeightOutputZone,
    loc: &crate::WorldLoc,
) {
    let footprint = loc.plan.rotated_definition_footprint;
    let Some((x0, x1)) = midpoint_pair(loc.tile.x, u32::from(footprint.width), world.scene.width())
    else {
        snapshot.mark_incomplete();
        return;
    };
    let Some((y0, y1)) =
        midpoint_pair(loc.tile.y, u32::from(footprint.depth), world.scene.height())
    else {
        snapshot.mark_incomplete();
        return;
    };
    for (x, y) in [(x1, y0), (x0, y0), (x0, y1), (x1, y1)] {
        record_local_source(world, snapshot, plane, x as i32, y as i32, output);
    }
}

fn record_static_model_samples(
    world: &WorldScene,
    snapshot: &mut HeightDependencySnapshot,
    plane: SourcePlane,
    output: HeightOutputZone,
    loc: &crate::WorldLoc,
    model: &osrs_core::lighting::ReferenceLitModel,
) {
    let radius = model_xz_radius(model);
    let origin_x = loc.plan.model_center.x.units();
    let origin_y = loc.plan.model_center.z.units();
    let Some((min_x, max_x)) = contour_axis_samples(origin_x, radius, world.scene.width() as i32)
    else {
        snapshot.mark_incomplete();
        return;
    };
    let Some((min_y, max_y)) = contour_axis_samples(origin_y, radius, world.scene.height() as i32)
    else {
        snapshot.mark_incomplete();
        return;
    };
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            record_local_source(world, snapshot, plane, x, y, output);
        }
    }
}

fn record_local_source(
    world: &WorldScene,
    snapshot: &mut HeightDependencySnapshot,
    plane: SourcePlane,
    local_x: i32,
    local_y: i32,
    output: HeightOutputZone,
) {
    snapshot.record(
        HeightSource::new(
            MapTile::new(world.window.base_x + local_x, world.window.base_y + local_y),
            plane,
        ),
        output,
    );
}

fn owned_output_zones(world: &WorldScene, owned: OwnedTiles) -> BTreeSet<HeightOutputZone> {
    let min_world_x = world.window.base_x + owned.min.0 as i32;
    let min_world_y = world.window.base_y + owned.min.1 as i32;
    let max_world_x = world.window.base_x + owned.max.0 as i32 - 1;
    let max_world_y = world.window.base_y + owned.max.1 as i32 - 1;
    let mut zones = BTreeSet::new();
    for zone_x in min_world_x.div_euclid(ZONE_SIZE_TILES)..=max_world_x.div_euclid(ZONE_SIZE_TILES)
    {
        for zone_y in
            min_world_y.div_euclid(ZONE_SIZE_TILES)..=max_world_y.div_euclid(ZONE_SIZE_TILES)
        {
            zones.insert((zone_x, zone_y));
        }
    }
    zones
}

fn midpoint_pair(anchor: u32, extent: u32, scene_extent: u32) -> Option<(u32, u32)> {
    let fits = anchor.checked_add(extent)? <= scene_extent;
    if fits {
        Some((
            anchor.checked_add(extent >> 1)?,
            anchor.checked_add((extent + 1) >> 1)?,
        ))
    } else {
        Some((anchor, anchor.checked_add(1)?))
    }
}

fn contour_axis_samples(origin: i32, radius: i32, scene_extent: i32) -> Option<(i32, i32)> {
    if radius < 0 || scene_extent < 0 {
        return None;
    }
    let low = origin.checked_sub(radius)?.div_euclid(TILE_UNITS);
    let high = origin
        .checked_add(radius)?
        .div_euclid(TILE_UNITS)
        .checked_add(1)?;
    Some((low.max(0), high.min(scene_extent)))
}

fn world_zone(world_x: i32, world_y: i32) -> HeightOutputZone {
    (
        world_x.div_euclid(ZONE_SIZE_TILES),
        world_y.div_euclid(ZONE_SIZE_TILES),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_midpoint_pair_matches_reference_and_edge_fallback() {
        assert_eq!(midpoint_pair(0, 3, 4), Some((1, 2)));
        assert_eq!(midpoint_pair(3, 3, 4), Some((3, 4)));
    }

    #[test]
    fn contour_samples_include_bilinear_plus_one_and_clamp_to_scene() {
        assert_eq!(contour_axis_samples(128, 128, 104), Some((0, 3)));
        assert_eq!(contour_axis_samples(32, 256, 104), Some((0, 3)));
        assert_eq!(contour_axis_samples(13_000, 512, 104), Some((97, 104)));
    }

    #[test]
    fn world_zone_uses_euclidean_coordinates() {
        assert_eq!(world_zone(0, 0), (0, 0));
        assert_eq!(world_zone(-1, -1), (-1, -1));
        assert_eq!(world_zone(-9, -17), (-2, -3));
    }

    #[test]
    fn snapshot_deduplicates_source_to_output_edges() {
        let plane = SourcePlane::new(2).unwrap_or_else(|| unreachable!("test plane is valid"));
        let source = HeightSource::new(MapTile::new(-1, 64), plane);
        let mut snapshot = HeightDependencySnapshot::default();
        snapshot.record(source, (-1, 8));
        snapshot.record(source, (-1, 8));
        snapshot.record(source, (0, 8));

        assert_eq!(snapshot.by_source[&source].len(), 2);
        assert!(snapshot.complete);
    }
}
