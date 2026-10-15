use super::*;
use osrs_core::coords::SourcePlane;
use osrs_edit::HeightVertexCoord;

fn height(x: i32, y: i32, plane: u8) -> HeightVertexCoord {
    let plane = SourcePlane::new(plane).unwrap_or_else(|| unreachable!("test plane is valid"));
    HeightVertexCoord::new(MapTile::new(x, y), plane)
}

fn touched_regions(footprint: &EditFootprint) -> Vec<RegionCoord> {
    footprint
        .terrain_tiles
        .iter()
        .map(|tile| tile.split_region().0)
        .chain(
            footprint
                .height_vertices
                .iter()
                .map(|coord| coord.corner.split_region().0),
        )
        .chain(
            footprint
                .object_tiles
                .iter()
                .map(|tile| tile.split_region().0),
        )
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
        None,
    );
    let ProjectionInvalidation::Zones(zones) = result else {
        unreachable!("pure terrain edit is zone bounded")
    };
    assert_eq!(zones.len(), 9);
    assert_eq!(zones.first(), Some(&(399, 399)));
    assert_eq!(zones.last(), Some(&(401, 401)));
}

#[test]
fn height_receipt_without_metadata_keeps_previous_region_ring_fallback() {
    let footprint = EditFootprint::new([], [height(3204, 3204, 0)], []);
    let regions = touched_regions(&footprint);
    let result = projection_invalidation(
        ChangeImpact::TERRAIN_GEOMETRY
            | ChangeImpact::NORMALS
            | ChangeImpact::LIGHTING
            | ChangeImpact::LOCATIONS
            | ChangeImpact::BOUNDS,
        &regions,
        &footprint,
        None,
    );
    let ProjectionInvalidation::Regions(regions) = result else {
        unreachable!("height without proof remains region conservative")
    };
    assert_eq!(regions.len(), 9);
    assert_eq!(regions.first(), Some(&RegionCoord::new(49, 49)));
    assert_eq!(regions.last(), Some(&RegionCoord::new(51, 51)));
}

#[test]
fn height_receipt_with_proven_dependencies_uses_exact_zones() {
    let footprint = EditFootprint::new([], [height(3204, 3204, 2)], []);
    let regions = touched_regions(&footprint);
    let proven = BTreeSet::from([(400, 400), (401, 400)]);
    assert_eq!(
        projection_invalidation(
            ChangeImpact::TERRAIN_GEOMETRY
                | ChangeImpact::NORMALS
                | ChangeImpact::LIGHTING
                | ChangeImpact::LOCATIONS
                | ChangeImpact::BOUNDS,
            &regions,
            &footprint,
            Some(&proven),
        ),
        ProjectionInvalidation::Zones(proven)
    );
}

#[test]
fn height_proof_can_span_an_eight_tile_zone_edge() {
    let footprint = EditFootprint::new([], [height(7, 7, 0)], []);
    let regions = touched_regions(&footprint);
    let proven = BTreeSet::from([(0, 0), (1, 0), (0, 1), (1, 1)]);
    assert_eq!(
        projection_invalidation(
            ChangeImpact::TERRAIN_GEOMETRY | ChangeImpact::NORMALS | ChangeImpact::LIGHTING,
            &regions,
            &footprint,
            Some(&proven),
        ),
        ProjectionInvalidation::Zones(proven)
    );
}

#[test]
fn height_proof_can_span_a_sixty_four_tile_region_edge() {
    let footprint = EditFootprint::new([], [height(63, 31, 1)], []);
    let regions = touched_regions(&footprint);
    let proven = BTreeSet::from([(7, 3), (8, 3)]);
    assert_eq!(
        projection_invalidation(
            ChangeImpact::TERRAIN_GEOMETRY
                | ChangeImpact::NORMALS
                | ChangeImpact::LIGHTING
                | ChangeImpact::LOCATIONS,
            &regions,
            &footprint,
            Some(&proven),
        ),
        ProjectionInvalidation::Zones(proven)
    );
    assert_eq!(zone_region((7, 3)), (0, 0));
    assert_eq!(zone_region((8, 3)), (1, 0));
}

#[test]
fn mixed_height_and_terrain_unions_both_zone_sets() {
    let footprint = EditFootprint::new([MapTile::new(3204, 3204)], [height(3204, 3204, 0)], []);
    let regions = touched_regions(&footprint);
    let proven = BTreeSet::from([(410, 410)]);
    let ProjectionInvalidation::Zones(zones) = projection_invalidation(
        ChangeImpact::TERRAIN_GEOMETRY
            | ChangeImpact::TERRAIN_MATERIAL
            | ChangeImpact::NORMALS
            | ChangeImpact::LIGHTING,
        &regions,
        &footprint,
        Some(&proven),
    ) else {
        unreachable!("complete Height proof plus terrain dependency remains zone bounded")
    };
    assert!(zones.contains(&(410, 410)));
    assert!(zones.contains(&(399, 399)));
    assert!(zones.contains(&(401, 401)));
}

#[test]
fn object_receipt_without_runtime_proof_rebuilds_the_contextual_producer_ring() {
    let footprint = EditFootprint::new([], [], [MapTile::new(3204, 3204)]);
    let regions = touched_regions(&footprint);
    let ProjectionInvalidation::Regions(regions) = projection_invalidation(
        ChangeImpact::LOCATIONS | ChangeImpact::COLLISION | ChangeImpact::BOUNDS,
        &regions,
        &footprint,
        None,
    ) else {
        unreachable!("unproven object dependencies remain region conservative")
    };
    assert_eq!(regions.len(), 9);
    assert_eq!(regions.first(), Some(&RegionCoord::new(49, 49)));
    assert_eq!(regions.last(), Some(&RegionCoord::new(51, 51)));
}

#[test]
fn unproven_object_receipt_overrides_any_height_zone_proof() {
    let footprint = EditFootprint::new([], [height(3204, 3204, 0)], [MapTile::new(3204, 3204)]);
    let regions = touched_regions(&footprint);
    let proven = BTreeSet::from([(400, 400)]);
    let ProjectionInvalidation::Regions(regions) = projection_invalidation(
        ChangeImpact::TERRAIN_GEOMETRY | ChangeImpact::LOCATIONS | ChangeImpact::BOUNDS,
        &regions,
        &footprint,
        Some(&proven),
    ) else {
        unreachable!("unproven object dependencies remain region conservative")
    };
    assert_eq!(regions.len(), 9);
}

#[test]
fn empty_impact_never_invalidates_projection() {
    let footprint = EditFootprint::new([MapTile::new(3204, 3204)], [], []);
    let regions = touched_regions(&footprint);
    assert_eq!(
        projection_invalidation(ChangeImpact::NONE, &regions, &footprint, None),
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
