//! Editor-side freshness wrapper for exact Object producer-output comparison.
//!
//! O3 defines the proof contract only. O4 will decide when accepted worker output is installed and
//! when a post-edit candidate build is compared. Until then Object invalidation remains on the O1
//! conservative 3x3 producer-region fallback.

#![cfg_attr(not(test), allow(dead_code))]

use super::{RegionKey, ZoneKey};
use osrs_core::coords::RegionCoord;
use osrs_world::{ObjectOutputComparison, ObjectOutputProofError, ObjectOutputSnapshot};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ObjectOutputStamp {
    pub projection_epoch: u64,
    pub presentation_generation: u32,
}

impl ObjectOutputStamp {
    pub(super) const fn new(projection_epoch: u64, presentation_generation: u32) -> Self {
        Self {
            projection_epoch,
            presentation_generation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ObjectOutputLookup {
    Stale,
    Incomplete,
    Current(BTreeSet<ZoneKey>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ObjectOutputStateError {
    ProducerKeyMismatch {
        expected: RegionCoord,
        actual: RegionCoord,
    },
    Proof(ObjectOutputProofError),
}

impl From<ObjectOutputProofError> for ObjectOutputStateError {
    fn from(error: ObjectOutputProofError) -> Self {
        Self::Proof(error)
    }
}

/// Accepted exact static output for one producer, stamped to editor projection identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ObjectOutputState {
    key: RegionKey,
    stamp: ObjectOutputStamp,
    snapshot: ObjectOutputSnapshot,
}

impl ObjectOutputState {
    pub(super) fn new(
        key: RegionKey,
        stamp: ObjectOutputStamp,
        snapshot: ObjectOutputSnapshot,
    ) -> Result<Self, ObjectOutputStateError> {
        validate_producer(key, &snapshot)?;
        Ok(Self {
            key,
            stamp,
            snapshot,
        })
    }

    pub(super) const fn stamp(&self) -> ObjectOutputStamp {
        self.stamp
    }

    pub(super) fn snapshot(&self) -> &ObjectOutputSnapshot {
        &self.snapshot
    }

    /// Compare current accepted output with a full candidate only when the accepted state stamp is
    /// still current. Missing coverage on either side fails closed as `Incomplete`.
    pub(super) fn compare(
        &self,
        current: ObjectOutputStamp,
        candidate: &ObjectOutputSnapshot,
    ) -> Result<ObjectOutputLookup, ObjectOutputStateError> {
        if current != self.stamp {
            return Ok(ObjectOutputLookup::Stale);
        }
        validate_producer(self.key, candidate)?;
        Ok(match self.snapshot.compare(candidate)? {
            ObjectOutputComparison::Incomplete => ObjectOutputLookup::Incomplete,
            ObjectOutputComparison::Exact(zones) => ObjectOutputLookup::Current(zones),
        })
    }

    /// Apply one accepted static refresh and advance the proof identity to that accepted build.
    /// Scoped updates preserve untouched baseline zones; an incomplete baseline stays incomplete.
    pub(super) fn refresh(
        &mut self,
        stamp: ObjectOutputStamp,
        update: ObjectOutputSnapshot,
    ) -> Result<(), ObjectOutputStateError> {
        validate_producer(self.key, &update)?;
        self.snapshot.merge(update)?;
        self.stamp = stamp;
        Ok(())
    }

    /// Replace retained output after an accepted full producer build.
    pub(super) fn replace(
        &mut self,
        stamp: ObjectOutputStamp,
        snapshot: ObjectOutputSnapshot,
    ) -> Result<(), ObjectOutputStateError> {
        validate_producer(self.key, &snapshot)?;
        self.snapshot = snapshot;
        self.stamp = stamp;
        Ok(())
    }
}

fn validate_producer(
    key: RegionKey,
    snapshot: &ObjectOutputSnapshot,
) -> Result<(), ObjectOutputStateError> {
    let expected = RegionCoord::new(key.0, key.1);
    let actual = snapshot.producer_region();
    if actual != expected {
        return Err(ObjectOutputStateError::ProducerKeyMismatch { expected, actual });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_render::{SceneGeometry, ZoneGeometry, ZoneGroup};

    fn geometry(entries: &[((i32, i32), u8)]) -> SceneGeometry {
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

    fn full(key: RegionKey, entries: &[((i32, i32), u8)]) -> ObjectOutputSnapshot {
        ObjectOutputSnapshot::full(RegionCoord::new(key.0, key.1), &geometry(entries))
            .unwrap_or_else(|error| unreachable!("valid full snapshot: {error:?}"))
    }

    #[test]
    fn stale_projection_or_presentation_identity_never_returns_exact_zones() {
        let key = (50, 50);
        let state = ObjectOutputState::new(
            key,
            ObjectOutputStamp::new(7, 3),
            full(key, &[((400, 400), 0)]),
        )
        .unwrap_or_else(|error| unreachable!("valid state: {error:?}"));
        let candidate = full(key, &[((400, 400), 1)]);

        assert_eq!(
            state.compare(ObjectOutputStamp::new(8, 3), &candidate),
            Ok(ObjectOutputLookup::Stale)
        );
        assert_eq!(
            state.compare(ObjectOutputStamp::new(7, 4), &candidate),
            Ok(ObjectOutputLookup::Stale)
        );
    }

    #[test]
    fn current_complete_output_returns_exact_changed_zones() {
        let key = (0, 0);
        let stamp = ObjectOutputStamp::new(2, 9);
        let state = ObjectOutputState::new(
            key,
            stamp,
            full(key, &[((0, 0), 0), ((1, 0), 0)]),
        )
        .unwrap_or_else(|error| unreachable!("valid state: {error:?}"));
        let candidate = full(key, &[((1, 0), 2), ((2, 0), 0)]);

        assert_eq!(
            state.compare(stamp, &candidate),
            Ok(ObjectOutputLookup::Current(BTreeSet::from([
                (0, 0),
                (1, 0),
                (2, 0)
            ])))
        );
    }

    #[test]
    fn incomplete_candidate_fails_closed() {
        let key = (0, 0);
        let stamp = ObjectOutputStamp::new(1, 1);
        let state = ObjectOutputState::new(key, stamp, full(key, &[((0, 0), 0)]))
            .unwrap_or_else(|error| unreachable!("valid state: {error:?}"));
        let candidate = ObjectOutputSnapshot::scoped(
            RegionCoord::new(0, 0),
            [(0, 0)],
            &geometry(&[((0, 0), 1)]),
        )
        .unwrap_or_else(|error| unreachable!("valid scoped candidate: {error:?}"));

        assert_eq!(
            state.compare(stamp, &candidate),
            Ok(ObjectOutputLookup::Incomplete)
        );
    }

    #[test]
    fn accepted_scoped_refresh_preserves_complete_baseline_and_advances_stamp() {
        let key = (0, 0);
        let mut state = ObjectOutputState::new(
            key,
            ObjectOutputStamp::new(4, 2),
            full(key, &[((0, 0), 0), ((1, 0), 0)]),
        )
        .unwrap_or_else(|error| unreachable!("valid state: {error:?}"));
        let update = ObjectOutputSnapshot::scoped(
            RegionCoord::new(0, 0),
            [(0, 0)],
            &geometry(&[((0, 0), 3)]),
        )
        .unwrap_or_else(|error| unreachable!("valid scoped update: {error:?}"));
        let next = ObjectOutputStamp::new(5, 2);

        state
            .refresh(next, update)
            .unwrap_or_else(|error| unreachable!("accepted refresh: {error:?}"));
        assert_eq!(state.stamp(), next);
        assert_eq!(
            state.compare(next, &full(key, &[((0, 0), 3), ((1, 0), 0)])),
            Ok(ObjectOutputLookup::Current(BTreeSet::new()))
        );
    }

    #[test]
    fn producer_mismatch_is_explicit() {
        let snapshot = full((1, 0), &[]);
        assert!(matches!(
            ObjectOutputState::new((0, 0), ObjectOutputStamp::new(0, 1), snapshot),
            Err(ObjectOutputStateError::ProducerKeyMismatch { .. })
        ));
    }
}
