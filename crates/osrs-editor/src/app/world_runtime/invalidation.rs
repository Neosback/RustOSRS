use super::{
    WorldRuntime, ZoneKey, builds::ObjectOutputStamp,
    height_dependencies::HeightDependencyStamp,
};
use crate::app::edit_runtime::{EditFootprint, EditTouches};
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

impl WorldRuntime {
    /// Translate one finalized semantic touch set into projection invalidation.
    ///
    /// Object edits first attempt the accepted-output comparison path. That path is entered only
    /// when every resident non-empty producer in the O1 3x3 conservative universe has current,
    /// complete accepted output. Each producer then receives a full contextual build; exact static
    /// zones are selected only after that accepted post-edit result is compared with the retained
    /// pre-edit baseline. If any baseline is absent, stale, or incomplete, Object edits retain the
    /// O1 full-region fallback. Height-only edits keep their existing dependency proof path.
    pub(in crate::app) fn invalidate_semantic_edit(
        &mut self,
        impact: ChangeImpact,
        touched_regions: &EditTouches,
    ) {
        if !touched_regions.footprint.object_tiles.is_empty()
            && self.try_queue_object_proofs(touched_regions)
        {
            return;
        }

        let proven_height_zones = if touched_regions.footprint.height_vertices.is_empty() {
            None
        } else {
            let producers = self
                .loaded
                .iter()
                .filter_map(|(&key, stats)| {
                    stats.as_ref().map(|_| {
                        (
                            key,
                            HeightDependencyStamp::new(
                                self.builds.projection_epoch(key),
                                self.presentation_generation,
                            ),
                        )
                    })
                })
                .collect::<Vec<_>>();
            self.height_dependencies
                .proven_zones(&producers, &touched_regions.footprint.height_vertices)
        };

        match projection_invalidation(
            impact,
            touched_regions,
            &touched_regions.footprint,
            proven_height_zones.as_ref(),
        ) {
            ProjectionInvalidation::None => {}
            ProjectionInvalidation::Regions(regions) => {
                for region in regions {
                    self.mark_dirty((region.x, region.y));
                }
            }
            ProjectionInvalidation::Zones(zones) => {
                for zone in zones {
                    self.mark_zone_dirty(zone);
                }
            }
        }
    }

    /// Queue full candidate builds for exact Object output comparison. Planning is all-or-nothing:
    /// one untrusted resident producer prevents every producer from using the proof path.
    fn try_queue_object_proofs(&mut self, touched_regions: &[RegionCoord]) -> bool {
        let producer_ring = expand_regions(touched_regions, 1);
        let producers = producer_ring
            .iter()
            .map(|region| (region.x, region.y))
            .filter(|key| matches!(self.loaded.get(key), Some(Some(_))))
            .collect::<Vec<_>>();

        let mut baselines = Vec::with_capacity(producers.len());
        for key in producers {
            let current = ObjectOutputStamp::new(
                self.builds.projection_epoch(key),
                self.presentation_generation,
            );
            let Some(baseline) = self.object_outputs.current_complete(key, current) else {
                return false;
            };
            baselines.push((key, baseline));
        }

        // Increment projection epochs before retaining the pre-edit stamp. A later edit will cancel
        // the pending proof and/or advance the epoch again, causing any already-requested candidate
        // result to be rejected by BuildTracker before comparison.
        for (key, baseline) in baselines {
            self.mark_dirty(key);
            self.object_outputs.begin_proof(key, baseline);
        }
        true
    }

    /// Queue a full `region` rebuild if it is resident. Full invalidation subsumes queued zones.
    pub(super) fn mark_dirty(&mut self, region: (i32, i32)) {
        self.object_outputs.cancel_pending(region);
        if self.loaded.contains_key(&region) {
            self.builds.invalidate(region);
            self.dirty_zones.remove(&region);
            self.dirty_regions.insert(region);
        }
    }

    /// Queue one world 8x8 zone when its owning region is resident and no full rebuild supersedes it.
    fn mark_zone_dirty(&mut self, zone: ZoneKey) {
        let region = zone_region(zone);
        // Any later semantic invalidation cancels an Object proof that was waiting on this producer.
        // The conservative/normal dirty state remains and will rebuild safely.
        self.object_outputs.cancel_pending(region);
        if self.dirty_regions.contains(&region)
            || !matches!(self.loaded.get(&region), Some(Some(_)))
        {
            return;
        }
        self.builds.invalidate(region);
        self.dirty_zones.entry(region).or_default().insert(zone);
    }
}

pub(super) fn projection_invalidation(
    impact: ChangeImpact,
    touched_regions: &[RegionCoord],
    footprint: &EditFootprint,
    proven_height_zones: Option<&BTreeSet<ZoneKey>>,
) -> ProjectionInvalidation {
    if impact.is_empty() || touched_regions.is_empty() {
        return ProjectionInvalidation::None;
    }

    let EditFootprint {
        terrain_tiles,
        height_vertices,
        object_tiles,
        ..
    } = footprint;

    // Runtime Object comparison is attempted before this pure fallback function. Reaching this path
    // with an Object footprint means at least one required producer baseline was absent, stale, or
    // incomplete, so fail closed to the complete 3x3 producer-region ring.
    if !object_tiles.is_empty() {
        return ProjectionInvalidation::Regions(expand_regions(touched_regions, 1));
    }

    if terrain_tiles.is_empty() && height_vertices.is_empty() {
        return ProjectionInvalidation::Regions(invalidated_regions(impact, touched_regions));
    }

    if !height_vertices.is_empty() {
        let Some(height_zones) = proven_height_zones else {
            return ProjectionInvalidation::Regions(invalidated_regions(impact, touched_regions));
        };
        let mut zones = height_zones.clone();
        if !terrain_tiles.is_empty() {
            zones.extend(invalidated_terrain_zones(
                terrain_tiles,
                TERRAIN_BLEND_RADIUS_TILES,
            ));
        }
        return ProjectionInvalidation::Zones(zones);
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
    expand_regions(touched_regions, reach)
}

fn expand_regions(touched_regions: &[RegionCoord], reach: i32) -> Vec<RegionCoord> {
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
}
