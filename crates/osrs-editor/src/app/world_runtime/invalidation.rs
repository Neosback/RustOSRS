use super::ZoneKey;
use crate::app::edit_runtime::EditFootprint;
use osrs_core::coords::{MapTile, RegionCoord};
use osrs_edit::ChangeImpact;
use std::collections::BTreeSet;

/// Renderer/world extraction zones are 8x8 map tiles.
const ZONE_SIZE_TILES: i32 = 8;
/// Reference underlay blending reaches five tiles from the edited terrain tile.
const TERRAIN_BLEND_RADIUS_TILES: i32 = 5;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ProjectionInvalidation {
    None,
    Regions(Vec<RegionCoord>),
    Zones(BTreeSet<ZoneKey>),
}

pub(super) fn projection_invalidation(
    impact: ChangeImpact,
    touched_regions: &[RegionCoord],
    footprint: &EditFootprint,
) -> ProjectionInvalidation {
    if impact.is_empty() || touched_regions.is_empty() {
        return ProjectionInvalidation::None;
    }

    let EditFootprint {
        terrain_tiles,
        height_vertices,
        object_tiles,
    } = footprint;

    // Height can re-contour models over their model-space XZ bounds; object edits can write clipped
    // shadows over definition-sized footprints. Until those dependency footprints are explicit,
    // retain the previously certified region policy rather than guessing a zone radius.
    if !height_vertices.is_empty() || !object_tiles.is_empty() || terrain_tiles.is_empty() {
        return ProjectionInvalidation::Regions(invalidated_regions(impact, touched_regions));
    }

    ProjectionInvalidation::Zones(invalidated_terrain_zones(
        terrain_tiles,
        TERRAIN_BLEND_RADIUS_TILES,
    ))
}

/// Previous conservative region policy, retained as the safe fallback for unbounded dependencies.
fn invalidated_regions(impact: ChangeImpact, touched_regions: &[RegionCoord]) -> Vec<RegionCoord> {
    if impact.is_empty() || touched_regions.is_empty() {
        return Vec::new();
    }

    let crosses_region_boundary = impact.intersects(ChangeImpact::TERRAIN_GEOMETRY)
        || impact.intersects(ChangeImpact::TERRAIN_MATERIAL)
        || impact.intersects(ChangeImpact::NORMALS)
        || impact.intersects(ChangeImpact::LIGHTING);
    let reach = if crosses_region_boundary { 1 } else { 0 };
    let mut regions = BTreeSet::new();
    for region in touched_regions {
        for dx in -reach..=reach {
            for dy in -reach..=reach {
                regions.insert(RegionCoord::new(region.x + dx, region.y + dy));
            }
        }
    }
    regions.into_iter().collect()
}

fn invalidated_terrain_zones(tiles: &[MapTile], radius: i32) -> BTreeSet<ZoneKey> {
    let mut zones = BTreeSet::new();
    for tile in tiles {
        for x in tile.x - radius..=tile.x + radius {
            for y in tile.y - radius..=tile.y + radius {
                zones.insert((x.div_euclid(ZONE_SIZE_TILES), y.div_euclid(ZONE_SIZE_TILES)));
            }
        }
    }
    zones
}

pub(super) fn zone_region(zone: ZoneKey) -> (i32, i32) {
    (
        zone.0.div_euclid(64 / ZONE_SIZE_TILES),
        zone.1.div_euclid(64 / ZONE_SIZE_TILES),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touched_regions(footprint: &EditFootprint) -> Vec<RegionCoord> {
        footprint
            .terrain_tiles
            .iter()
            .chain(&footprint.height_vertices)
            .chain(&footprint.object_tiles)
            .map(|tile| tile.split_region().0)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    #[test]
    fn terrain_only_receipt_maps_radius_five_to_world_zones() {
        let footprint = EditFootprint::new([MapTile::new(3204, 3204)], [], []);
        let regions = touched_regions(&footprint);
        let result = projection_invalidation(
            ChangeImpact::TERRAIN_GEOMETRY
                | ChangeImpact::TERRAIN_MATERIAL
                | ChangeImpact::NORMALS
                | ChangeImpact::LIGHTING,
            &regions,
            &footprint,
        );
        let ProjectionInvalidation::Zones(zones) = result else {
            unreachable!("pure terrain edit is zone bounded")
        };
        assert_eq!(zones.len(), 9);
        assert_eq!(zones.first(), Some(&(399, 399)));
        assert_eq!(zones.last(), Some(&(401, 401)));
    }

    #[test]
    fn height_receipt_keeps_previous_region_ring_fallback() {
        let footprint = EditFootprint::new([], [MapTile::new(3204, 3204)], []);
        let regions = touched_regions(&footprint);
        let result = projection_invalidation(
            ChangeImpact::TERRAIN_GEOMETRY
                | ChangeImpact::NORMALS
                | ChangeImpact::LIGHTING
                | ChangeImpact::LOCATIONS
                | ChangeImpact::BOUNDS,
            &regions,
            &footprint,
        );
        let ProjectionInvalidation::Regions(regions) = result else {
            unreachable!("height remains region conservative")
        };
        assert_eq!(regions.len(), 9);
        assert_eq!(regions.first(), Some(&RegionCoord::new(49, 49)));
        assert_eq!(regions.last(), Some(&RegionCoord::new(51, 51)));
    }

    #[test]
    fn object_receipt_keeps_touched_region_fallback() {
        let footprint = EditFootprint::new([], [], [MapTile::new(3204, 3204)]);
        let regions = touched_regions(&footprint);
        assert_eq!(
            projection_invalidation(
                ChangeImpact::LOCATIONS | ChangeImpact::COLLISION | ChangeImpact::BOUNDS,
                &regions,
                &footprint,
            ),
            ProjectionInvalidation::Regions(vec![RegionCoord::new(50, 50)])
        );
    }

    #[test]
    fn empty_impact_never_invalidates_projection() {
        let footprint = EditFootprint::new([MapTile::new(3204, 3204)], [], []);
        let regions = touched_regions(&footprint);
        assert_eq!(
            projection_invalidation(ChangeImpact::NONE, &regions, &footprint),
            ProjectionInvalidation::None
        );
    }

    #[test]
    fn zone_to_region_uses_euclidean_coordinates() {
        assert_eq!(zone_region((400, 400)), (50, 50));
        assert_eq!(zone_region((-1, -1)), (-1, -1));
        assert_eq!(zone_region((-8, -8)), (-1, -1));
        assert_eq!(zone_region((-9, -9)), (-2, -2));
    }
}
