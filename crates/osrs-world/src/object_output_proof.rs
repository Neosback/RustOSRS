//! Exact producer-output comparison contract for Object invalidation.
//!
//! Object placement effects are not safely bounded by anchor distance: placement order, scene
//! capacity, fixed-slot replacement, wall-decoration hosts, terrain shadows, and ModelData normal
//! reconciliation can all change output that belongs to another placement. Rather than guess a
//! causal radius, this module compares exact static output produced from the fully assembled scene.
//!
//! A full snapshot covers all 64 8x8 output zones owned by one 64x64 producer region. Scoped
//! snapshots carry explicit coverage and are incomplete until merged into complete accepted state.
//! Missing coverage is never interpreted as unchanged output.

use osrs_core::coords::RegionCoord;
use osrs_render::{SceneGeometry, ZoneGeometry};
use std::collections::{BTreeMap, BTreeSet};

const ZONES_PER_REGION: i32 = 8;

pub type ObjectOutputZone = (i32, i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectOutputCompleteness {
    Incomplete,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectOutputProofError {
    ProducerRegionMismatch {
        expected: RegionCoord,
        actual: RegionCoord,
    },
    CoverageZoneOutsideProducerRegion {
        producer: RegionCoord,
        zone: ObjectOutputZone,
        actual: RegionCoord,
    },
    GeometryZoneOutsideProducerRegion {
        producer: RegionCoord,
        zone: ObjectOutputZone,
        actual: RegionCoord,
    },
    GeometryZoneOutsideCoverage {
        zone: ObjectOutputZone,
    },
    DuplicateGeometryZone {
        zone: ObjectOutputZone,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectOutputComparison {
    Incomplete,
    Exact(BTreeSet<ObjectOutputZone>),
}

/// Exact static output owned by one producer region.
///
/// `zones` stores the actual emitted geometry for covered zones. A covered zone absent from `zones`
/// is a proven empty output zone. The separate `covered` set is therefore essential: absence from
/// an incomplete scoped build cannot be treated as empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectOutputSnapshot {
    producer_region: RegionCoord,
    covered: BTreeSet<ObjectOutputZone>,
    zones: BTreeMap<ObjectOutputZone, ZoneGeometry>,
}

impl ObjectOutputSnapshot {
    /// Capture a complete producer snapshot from full static geometry extraction.
    pub fn full(
        producer_region: RegionCoord,
        geometry: &SceneGeometry,
    ) -> Result<Self, ObjectOutputProofError> {
        Self::scoped(producer_region, producer_zones(producer_region), geometry)
    }

    /// Capture explicitly covered output zones from a scoped static extraction.
    pub fn scoped(
        producer_region: RegionCoord,
        covered: impl IntoIterator<Item = ObjectOutputZone>,
        geometry: &SceneGeometry,
    ) -> Result<Self, ObjectOutputProofError> {
        let covered = covered.into_iter().collect::<BTreeSet<_>>();
        for &zone in &covered {
            let actual = output_zone_region(zone);
            if actual != producer_region {
                return Err(ObjectOutputProofError::CoverageZoneOutsideProducerRegion {
                    producer: producer_region,
                    zone,
                    actual,
                });
            }
        }

        let mut zones = BTreeMap::new();
        for zone_geometry in &geometry.zones {
            let zone = (zone_geometry.zone_x, zone_geometry.zone_z);
            let actual = output_zone_region(zone);
            if actual != producer_region {
                return Err(ObjectOutputProofError::GeometryZoneOutsideProducerRegion {
                    producer: producer_region,
                    zone,
                    actual,
                });
            }
            if !covered.contains(&zone) {
                return Err(ObjectOutputProofError::GeometryZoneOutsideCoverage { zone });
            }
            if zones.insert(zone, zone_geometry.clone()).is_some() {
                return Err(ObjectOutputProofError::DuplicateGeometryZone { zone });
            }
        }

        Ok(Self {
            producer_region,
            covered,
            zones,
        })
    }

    pub const fn producer_region(&self) -> RegionCoord {
        self.producer_region
    }

    pub fn completeness(&self) -> ObjectOutputCompleteness {
        if self.covered == producer_zones(self.producer_region) {
            ObjectOutputCompleteness::Complete
        } else {
            ObjectOutputCompleteness::Incomplete
        }
    }

    pub fn covered_zones(&self) -> &BTreeSet<ObjectOutputZone> {
        &self.covered
    }

    /// Merge an accepted scoped refresh into retained producer output.
    ///
    /// Covered empty zones remove previously retained geometry. Uncovered zones remain untouched.
    pub fn merge(&mut self, update: Self) -> Result<(), ObjectOutputProofError> {
        if update.producer_region != self.producer_region {
            return Err(ObjectOutputProofError::ProducerRegionMismatch {
                expected: self.producer_region,
                actual: update.producer_region,
            });
        }

        for zone in update.covered {
            match update.zones.get(&zone) {
                Some(geometry) => {
                    self.zones.insert(zone, geometry.clone());
                }
                None => {
                    self.zones.remove(&zone);
                }
            }
            self.covered.insert(zone);
        }
        Ok(())
    }

    /// Compare two complete snapshots of the same producer and return exactly the changed zones.
    ///
    /// This is intentionally fail-closed: if either side lacks full producer coverage, no exact
    /// Object zone proof exists yet.
    pub fn compare(
        &self,
        candidate: &Self,
    ) -> Result<ObjectOutputComparison, ObjectOutputProofError> {
        if candidate.producer_region != self.producer_region {
            return Err(ObjectOutputProofError::ProducerRegionMismatch {
                expected: self.producer_region,
                actual: candidate.producer_region,
            });
        }
        if self.completeness() != ObjectOutputCompleteness::Complete
            || candidate.completeness() != ObjectOutputCompleteness::Complete
        {
            return Ok(ObjectOutputComparison::Incomplete);
        }

        let mut changed = BTreeSet::new();
        for zone in producer_zones(self.producer_region) {
            if self.zones.get(&zone) != candidate.zones.get(&zone) {
                changed.insert(zone);
            }
        }
        Ok(ObjectOutputComparison::Exact(changed))
    }
}

fn producer_zones(region: RegionCoord) -> BTreeSet<ObjectOutputZone> {
    let min_x = region.x * ZONES_PER_REGION;
    let min_y = region.y * ZONES_PER_REGION;
    let mut zones = BTreeSet::new();
    for dx in 0..ZONES_PER_REGION {
        for dy in 0..ZONES_PER_REGION {
            zones.insert((min_x + dx, min_y + dy));
        }
    }
    zones
}

fn output_zone_region(zone: ObjectOutputZone) -> RegionCoord {
    RegionCoord::new(
        zone.0.div_euclid(ZONES_PER_REGION),
        zone.1.div_euclid(ZONES_PER_REGION),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_render::ZoneGroup;

    fn geometry(entries: &[(ObjectOutputZone, u8)]) -> SceneGeometry {
        SceneGeometry {
            zones: entries
                .iter()
                .map(|&(zone, level)| ZoneGeometry {
                    zone_x: zone.0,
                    zone_z: zone.1,
                    groups: vec![ZoneGroup {
                        level,
                        ..ZoneGroup::default()
                    }],
                })
                .collect(),
        }
    }

    #[test]
    fn full_snapshot_explicitly_covers_all_sixty_four_owned_zones() {
        let region = RegionCoord::new(50, 50);
        let snapshot = ObjectOutputSnapshot::full(region, &geometry(&[((400, 400), 0)]))
            .unwrap_or_else(|error| unreachable!("valid producer geometry: {error:?}"));

        assert_eq!(snapshot.covered_zones().len(), 64);
        assert_eq!(
            snapshot.completeness(),
            ObjectOutputCompleteness::Complete
        );
    }

    #[test]
    fn negative_zone_ownership_uses_euclidean_coordinates() {
        let region = RegionCoord::new(-1, -1);
        let snapshot = ObjectOutputSnapshot::full(region, &geometry(&[((-1, -1), 0)]))
            .unwrap_or_else(|error| unreachable!("negative producer is valid: {error:?}"));

        assert!(snapshot.covered_zones().contains(&(-8, -8)));
        assert!(snapshot.covered_zones().contains(&(-1, -1)));
        assert_eq!(snapshot.covered_zones().len(), 64);
    }

    #[test]
    fn scoped_snapshot_is_incomplete_until_accepted_coverage_is_complete() {
        let region = RegionCoord::new(0, 0);
        let scoped = ObjectOutputSnapshot::scoped(region, [(0, 0)], &geometry(&[((0, 0), 0)]))
            .unwrap_or_else(|error| unreachable!("valid scoped geometry: {error:?}"));
        assert_eq!(
            scoped.completeness(),
            ObjectOutputCompleteness::Incomplete
        );

        let full = ObjectOutputSnapshot::full(region, &geometry(&[((0, 0), 0)]))
            .unwrap_or_else(|error| unreachable!("valid full geometry: {error:?}"));
        assert_eq!(
            scoped
                .compare(&full)
                .unwrap_or_else(|error| unreachable!("same producer: {error:?}")),
            ObjectOutputComparison::Incomplete
        );
    }

    #[test]
    fn accepted_scoped_merge_replaces_and_removes_only_covered_zones() {
        let region = RegionCoord::new(0, 0);
        let mut baseline = ObjectOutputSnapshot::full(
            region,
            &geometry(&[((0, 0), 0), ((1, 0), 0), ((2, 0), 0)]),
        )
        .unwrap_or_else(|error| unreachable!("valid baseline: {error:?}"));
        let update = ObjectOutputSnapshot::scoped(
            region,
            [(0, 0), (1, 0)],
            &geometry(&[((0, 0), 3)]),
        )
        .unwrap_or_else(|error| unreachable!("valid scoped update: {error:?}"));
        baseline
            .merge(update)
            .unwrap_or_else(|error| unreachable!("same producer merge: {error:?}"));

        let expected = ObjectOutputSnapshot::full(
            region,
            &geometry(&[((0, 0), 3), ((2, 0), 0)]),
        )
        .unwrap_or_else(|error| unreachable!("valid expected state: {error:?}"));
        assert_eq!(baseline, expected);
        assert_eq!(
            baseline.completeness(),
            ObjectOutputCompleteness::Complete
        );
    }

    #[test]
    fn exact_comparison_detects_add_remove_and_in_place_output_change() {
        let region = RegionCoord::new(0, 0);
        let before = ObjectOutputSnapshot::full(
            region,
            &geometry(&[((0, 0), 0), ((1, 0), 0), ((2, 0), 0)]),
        )
        .unwrap_or_else(|error| unreachable!("valid baseline: {error:?}"));
        let after = ObjectOutputSnapshot::full(
            region,
            &geometry(&[((1, 0), 2), ((2, 0), 0), ((3, 0), 0)]),
        )
        .unwrap_or_else(|error| unreachable!("valid candidate: {error:?}"));

        assert_eq!(
            before
                .compare(&after)
                .unwrap_or_else(|error| unreachable!("same producer: {error:?}")),
            ObjectOutputComparison::Exact(BTreeSet::from([(0, 0), (1, 0), (3, 0)]))
        );
    }

    #[test]
    fn geometry_outside_declared_scope_is_rejected() {
        let region = RegionCoord::new(0, 0);
        assert_eq!(
            ObjectOutputSnapshot::scoped(region, [(0, 0)], &geometry(&[((1, 0), 0)])),
            Err(ObjectOutputProofError::GeometryZoneOutsideCoverage { zone: (1, 0) })
        );
    }

    #[test]
    fn producer_mismatch_cannot_be_compared_or_merged() {
        let first = ObjectOutputSnapshot::full(RegionCoord::new(0, 0), &SceneGeometry::default())
            .unwrap_or_else(|error| unreachable!("valid first producer: {error:?}"));
        let second = ObjectOutputSnapshot::full(RegionCoord::new(1, 0), &SceneGeometry::default())
            .unwrap_or_else(|error| unreachable!("valid second producer: {error:?}"));

        assert!(matches!(
            first.compare(&second),
            Err(ObjectOutputProofError::ProducerRegionMismatch { .. })
        ));
        let mut first = first;
        assert!(matches!(
            first.merge(second),
            Err(ObjectOutputProofError::ProducerRegionMismatch { .. })
        ));
    }
}
