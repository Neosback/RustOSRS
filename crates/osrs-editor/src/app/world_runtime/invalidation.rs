use super::{
    WorldRuntime, ZoneKey, builds::ObjectOutputStamp, height_dependencies::HeightDependencyStamp,
};
use crate::app::edit_runtime::{EditFootprint, EditReceipt};
use osrs_core::coords::{MapTile, RegionCoord};
use osrs_edit::ChangeImpact;
use std::collections::BTreeSet;

/// Renderer/world extraction zones are 8x8 map tiles.
const ZONE_SIZE_TILES: i32 = 8;
/// Reference underlay blending reaches this many tiles from the edited terrain tile.
const TERRAIN_BLEND_RADIUS_TILES: i32 = osrs_scene::terrain_appearance::BLEND_RADIUS;

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
    pub(in crate::app) fn invalidate_semantic_edit(&mut self, receipt: &EditReceipt) {
        let impact = receipt.impact;
        let touched_regions = &receipt.touched_regions;
        if !touched_regions.footprint.object_tiles.is_empty()
            && self.try_queue_object_proofs(touched_regions)
        {
            self.builds.observe_invalidation(
                receipt.revision,
                expand_regions(touched_regions, 1),
                "object output proof: contextual candidate build",
            );
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
            ProjectionInvalidation::None => {
                self.builds.observe_invalidation(
                    receipt.revision,
                    Vec::new(),
                    "no derived output affected",
                );
            }
            ProjectionInvalidation::Regions(regions) => {
                self.builds.observe_invalidation(
                    receipt.revision,
                    regions.clone(),
                    "conservative region rebuild: dependency proof unavailable",
                );
                for region in regions {
                    self.mark_dirty((region.x, region.y));
                }
            }
            ProjectionInvalidation::Zones(zones) => {
                let regions = zones
                    .iter()
                    .copied()
                    .map(zone_region)
                    .map(|(x, y)| RegionCoord::new(x, y))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                let reason = if touched_regions.footprint.height_vertices.is_empty() {
                    "terrain zones: radius-5 material dependency"
                } else {
                    "height zones: dependency proof"
                };
                self.builds
                    .observe_invalidation(receipt.revision, regions, reason);
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
mod tests;
