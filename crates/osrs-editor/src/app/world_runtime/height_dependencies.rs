//! Projection-only Height dependency metadata.
//!
//! Semantic map state remains in `EditorSession`. This module owns disposable inverse read-sets
//! from accepted world builds and fails closed whenever freshness or completeness cannot be proven.

use super::{RegionKey, ZoneKey, invalidation::zone_region};
use osrs_core::coords::RegionCoord;
use osrs_edit::HeightVertexCoord;
use osrs_world::HeightDependencySnapshot;
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HeightDependencyStamp {
    pub projection_epoch: u64,
    pub presentation_generation: u32,
}

impl HeightDependencyStamp {
    pub(super) const fn new(projection_epoch: u64, presentation_generation: u32) -> Self {
        Self {
            projection_epoch,
            presentation_generation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HeightDependencyCompleteness {
    Incomplete,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HeightDependencyError {
    ProducerRegionMismatch {
        expected: RegionCoord,
        actual: RegionCoord,
    },
    StampMismatch {
        expected: HeightDependencyStamp,
        actual: HeightDependencyStamp,
    },
    OutputZoneOutsideProducerRegion {
        producer: RegionCoord,
        zone: ZoneKey,
        actual: RegionCoord,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum HeightDependencyLookup<'a> {
    Stale,
    Incomplete,
    Current(Option<&'a BTreeSet<ZoneKey>>),
}

/// Deterministic projection read-set for one producer region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HeightDependencyCollection {
    producer_region: RegionCoord,
    stamp: HeightDependencyStamp,
    completeness: HeightDependencyCompleteness,
    by_source: BTreeMap<HeightVertexCoord, BTreeSet<ZoneKey>>,
}

impl HeightDependencyCollection {
    pub(super) fn new(producer_region: RegionCoord, stamp: HeightDependencyStamp) -> Self {
        Self {
            producer_region,
            stamp,
            completeness: HeightDependencyCompleteness::Incomplete,
            by_source: BTreeMap::new(),
        }
    }

    pub(super) const fn producer_region(&self) -> RegionCoord {
        self.producer_region
    }

    pub(super) const fn stamp(&self) -> HeightDependencyStamp {
        self.stamp
    }

    pub(super) const fn completeness(&self) -> HeightDependencyCompleteness {
        self.completeness
    }

    pub(super) fn record(
        &mut self,
        source: HeightVertexCoord,
        output_zone: ZoneKey,
    ) -> Result<(), HeightDependencyError> {
        let actual = zone_region(output_zone);
        if actual != (self.producer_region.x, self.producer_region.y) {
            return Err(HeightDependencyError::OutputZoneOutsideProducerRegion {
                producer: self.producer_region,
                zone: output_zone,
                actual: RegionCoord::new(actual.0, actual.1),
            });
        }
        self.by_source.entry(source).or_default().insert(output_zone);
        Ok(())
    }

    pub(super) fn mark_complete(&mut self) {
        self.completeness = HeightDependencyCompleteness::Complete;
    }

    pub(super) fn merge(&mut self, other: Self) -> Result<(), HeightDependencyError> {
        if other.producer_region != self.producer_region {
            return Err(HeightDependencyError::ProducerRegionMismatch {
                expected: self.producer_region,
                actual: other.producer_region,
            });
        }
        if other.stamp != self.stamp {
            return Err(HeightDependencyError::StampMismatch {
                expected: self.stamp,
                actual: other.stamp,
            });
        }
        let both_complete = self.completeness == HeightDependencyCompleteness::Complete
            && other.completeness == HeightDependencyCompleteness::Complete;
        for (source, zones) in other.by_source {
            self.by_source.entry(source).or_default().extend(zones);
        }
        self.completeness = if both_complete {
            HeightDependencyCompleteness::Complete
        } else {
            HeightDependencyCompleteness::Incomplete
        };
        Ok(())
    }

    pub(super) fn lookup(
        &self,
        source: HeightVertexCoord,
        current: HeightDependencyStamp,
    ) -> HeightDependencyLookup<'_> {
        if current != self.stamp {
            return HeightDependencyLookup::Stale;
        }
        if self.completeness != HeightDependencyCompleteness::Complete {
            return HeightDependencyLookup::Incomplete;
        }
        HeightDependencyLookup::Current(self.by_source.get(&source))
    }
}

/// Accepted dependency snapshots keyed by the region whose output zones they describe.
#[derive(Debug, Default)]
pub(super) struct HeightDependencyStore {
    by_region: HashMap<RegionKey, HeightDependencyCollection>,
}

impl HeightDependencyStore {
    pub(super) fn clear(&mut self) {
        self.by_region.clear();
    }

    pub(super) fn remove(&mut self, key: RegionKey) {
        self.by_region.remove(&key);
    }

    pub(super) fn install(
        &mut self,
        key: RegionKey,
        stamp: HeightDependencyStamp,
        snapshot: &HeightDependencySnapshot,
    ) -> Result<(), HeightDependencyError> {
        let mut collection =
            HeightDependencyCollection::new(RegionCoord::new(key.0, key.1), stamp);
        for (source, zones) in &snapshot.by_source {
            let source = HeightVertexCoord::new(source.corner, source.plane);
            for &zone in zones {
                if let Err(error) = collection.record(source, zone) {
                    self.by_region.remove(&key);
                    return Err(error);
                }
            }
        }
        if snapshot.complete {
            collection.mark_complete();
        }
        self.by_region.insert(key, collection);
        Ok(())
    }

    /// Resolve through every resident producer. One unusable producer invalidates the proof.
    pub(super) fn proven_zones(
        &self,
        producers: &[(RegionKey, HeightDependencyStamp)],
        sources: &[HeightVertexCoord],
    ) -> Option<BTreeSet<ZoneKey>> {
        let mut zones = BTreeSet::new();
        for &(key, stamp) in producers {
            let dependencies = self.by_region.get(&key)?;
            for &source in sources {
                match dependencies.lookup(source, stamp) {
                    HeightDependencyLookup::Current(Some(found)) => {
                        zones.extend(found.iter().copied());
                    }
                    HeightDependencyLookup::Current(None) => {}
                    HeightDependencyLookup::Stale | HeightDependencyLookup::Incomplete => {
                        return None;
                    }
                }
            }
        }
        Some(zones)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_core::coords::{MapTile, SourcePlane};

    fn plane(value: u8) -> SourcePlane {
        SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
    }

    fn source(x: i32, y: i32, source_plane: u8) -> HeightVertexCoord {
        HeightVertexCoord::new(MapTile::new(x, y), plane(source_plane))
    }

    #[test]
    fn collections_begin_incomplete_and_require_explicit_certification() {
        let stamp = HeightDependencyStamp::new(4, 9);
        let mut dependencies = HeightDependencyCollection::new(RegionCoord::new(50, 50), stamp);
        let vertex = source(3200, 3200, 0);
        assert_eq!(dependencies.record(vertex, (400, 400)), Ok(()));
        assert_eq!(dependencies.completeness(), HeightDependencyCompleteness::Incomplete);
        assert_eq!(dependencies.lookup(vertex, stamp), HeightDependencyLookup::Incomplete);
        dependencies.mark_complete();
        assert_eq!(dependencies.completeness(), HeightDependencyCompleteness::Complete);
        assert_eq!(
            dependencies.lookup(vertex, stamp),
            HeightDependencyLookup::Current(dependencies.by_source.get(&vertex))
        );
    }

    #[test]
    fn negative_coordinates_and_multi_zone_dependencies_are_deterministic() {
        let stamp = HeightDependencyStamp::new(0, 1);
        let mut dependencies = HeightDependencyCollection::new(RegionCoord::new(-1, -1), stamp);
        let vertex = source(-1, -9, 2);
        assert_eq!(dependencies.record(vertex, (-1, -1)), Ok(()));
        assert_eq!(dependencies.record(vertex, (-8, -8)), Ok(()));
        assert_eq!(dependencies.record(vertex, (-1, -1)), Ok(()));
        dependencies.mark_complete();
        let HeightDependencyLookup::Current(Some(zones)) = dependencies.lookup(vertex, stamp)
        else {
            unreachable!("complete current source has recorded zones")
        };
        assert_eq!(zones.iter().copied().collect::<Vec<_>>(), vec![(-8, -8), (-1, -1)]);
    }

    #[test]
    fn output_zone_cannot_escape_its_producer_region() {
        let stamp = HeightDependencyStamp::new(2, 3);
        let mut dependencies = HeightDependencyCollection::new(RegionCoord::new(0, 0), stamp);
        let vertex = source(63, 63, 0);
        assert_eq!(dependencies.record(vertex, (7, 7)), Ok(()));
        assert_eq!(
            dependencies.record(vertex, (8, 0)),
            Err(HeightDependencyError::OutputZoneOutsideProducerRegion {
                producer: RegionCoord::new(0, 0),
                zone: (8, 0),
                actual: RegionCoord::new(1, 0),
            })
        );
    }

    #[test]
    fn merge_unions_and_deduplicates_only_matching_projection_identity() {
        let stamp = HeightDependencyStamp::new(6, 12);
        let region = RegionCoord::new(50, 50);
        let vertex = source(3208, 3208, 1);
        let mut first = HeightDependencyCollection::new(region, stamp);
        assert_eq!(first.record(vertex, (401, 401)), Ok(()));
        assert_eq!(first.record(vertex, (400, 401)), Ok(()));
        first.mark_complete();
        let mut second = HeightDependencyCollection::new(region, stamp);
        assert_eq!(second.record(vertex, (401, 401)), Ok(()));
        assert_eq!(second.record(vertex, (401, 400)), Ok(()));
        second.mark_complete();
        assert_eq!(first.merge(second), Ok(()));
        assert_eq!(first.completeness(), HeightDependencyCompleteness::Complete);
        let HeightDependencyLookup::Current(Some(zones)) = first.lookup(vertex, stamp) else {
            unreachable!("merged complete dependency exists")
        };
        assert_eq!(
            zones.iter().copied().collect::<Vec<_>>(),
            vec![(400, 401), (401, 400), (401, 401)]
        );
        let wrong_stamp =
            HeightDependencyCollection::new(region, HeightDependencyStamp::new(7, 12));
        assert_eq!(
            first.merge(wrong_stamp),
            Err(HeightDependencyError::StampMismatch {
                expected: stamp,
                actual: HeightDependencyStamp::new(7, 12),
            })
        );
    }

    #[test]
    fn incomplete_merge_cannot_upgrade_to_complete() {
        let stamp = HeightDependencyStamp::new(1, 1);
        let region = RegionCoord::new(0, 0);
        let mut complete = HeightDependencyCollection::new(region, stamp);
        complete.mark_complete();
        let incomplete = HeightDependencyCollection::new(region, stamp);
        assert_eq!(complete.merge(incomplete), Ok(()));
        assert_eq!(complete.completeness(), HeightDependencyCompleteness::Incomplete);
    }

    #[test]
    fn stale_epoch_or_presentation_never_returns_dependencies() {
        let stamp = HeightDependencyStamp::new(10, 20);
        let mut dependencies = HeightDependencyCollection::new(RegionCoord::new(0, 0), stamp);
        let vertex = source(0, 0, 3);
        assert_eq!(dependencies.record(vertex, (0, 0)), Ok(()));
        dependencies.mark_complete();
        assert_eq!(
            dependencies.lookup(vertex, HeightDependencyStamp::new(11, 20)),
            HeightDependencyLookup::Stale
        );
        assert_eq!(
            dependencies.lookup(vertex, HeightDependencyStamp::new(10, 21)),
            HeightDependencyLookup::Stale
        );
    }

    #[test]
    fn complete_collection_distinguishes_known_empty_from_unusable() {
        let stamp = HeightDependencyStamp::new(0, 1);
        let mut dependencies = HeightDependencyCollection::new(RegionCoord::new(0, 0), stamp);
        dependencies.mark_complete();
        assert_eq!(
            dependencies.lookup(source(5, 5, 0), stamp),
            HeightDependencyLookup::Current(None)
        );
    }

    #[test]
    fn accessors_preserve_producer_identity() {
        let stamp = HeightDependencyStamp::new(17, 3);
        let region = RegionCoord::new(-2, 7);
        let dependencies = HeightDependencyCollection::new(region, stamp);
        assert_eq!(dependencies.producer_region(), region);
        assert_eq!(dependencies.stamp(), stamp);
    }
}
