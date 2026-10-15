use super::{RegionBuildScope, RegionKey, ZoneKey};
use osrs_core::coords::RegionCoord;
use osrs_world::{ObjectOutputComparison, ObjectOutputProofError, ObjectOutputSnapshot};
use std::collections::{BTreeSet, HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuildStamp {
    request_id: u64,
    projection_epoch: u64,
    presentation_generation: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingBuild {
    key: RegionKey,
    scope: RegionBuildScope,
    stamp: BuildStamp,
}

/// Freshness identity for one accepted Object producer-output snapshot.
///
/// This deliberately matches the editor-side identity used to reject stale worker results. O4 can
/// trust exact Object zone comparison only while both the producer projection epoch and active
/// presentation generation still match this stamp.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ObjectOutputStamp {
    pub projection_epoch: u64,
    pub presentation_generation: u32,
}

#[cfg_attr(not(test), allow(dead_code))]
impl ObjectOutputStamp {
    pub(super) const fn new(projection_epoch: u64, presentation_generation: u32) -> Self {
        Self {
            projection_epoch,
            presentation_generation,
        }
    }
}

/// Result of attempting one exact producer-output comparison.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ObjectOutputLookup {
    Stale,
    Incomplete,
    Current(BTreeSet<ZoneKey>),
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ObjectOutputStateError {
    ProducerKeyMismatch {
        expected: RegionCoord,
        actual: RegionCoord,
    },
    Proof(ObjectOutputProofError),
}

#[cfg_attr(not(test), allow(dead_code))]
impl From<ObjectOutputProofError> for ObjectOutputStateError {
    fn from(error: ObjectOutputProofError) -> Self {
        Self::Proof(error)
    }
}

/// Accepted exact static output for one producer, stamped to editor projection identity.
///
/// O3 defines this state contract only. O4 owns installation from accepted worker builds and the
/// runtime decision to use exact changed zones versus the O1 conservative producer-ring fallback.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ObjectOutputState {
    key: RegionKey,
    stamp: ObjectOutputStamp,
    snapshot: ObjectOutputSnapshot,
}

#[cfg_attr(not(test), allow(dead_code))]
impl ObjectOutputState {
    pub(super) fn new(
        key: RegionKey,
        stamp: ObjectOutputStamp,
        snapshot: ObjectOutputSnapshot,
    ) -> Result<Self, ObjectOutputStateError> {
        validate_object_output_producer(key, &snapshot)?;
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

    /// Compare current accepted output with a candidate only while the accepted state is fresh.
    /// Missing coverage on either side remains explicitly incomplete and cannot prove exact zones.
    pub(super) fn compare(
        &self,
        current: ObjectOutputStamp,
        candidate: &ObjectOutputSnapshot,
    ) -> Result<ObjectOutputLookup, ObjectOutputStateError> {
        if current != self.stamp {
            return Ok(ObjectOutputLookup::Stale);
        }
        validate_object_output_producer(self.key, candidate)?;
        Ok(match self.snapshot.compare(candidate)? {
            ObjectOutputComparison::Incomplete => ObjectOutputLookup::Incomplete,
            ObjectOutputComparison::Exact(zones) => ObjectOutputLookup::Current(zones),
        })
    }

    /// Apply one accepted scoped static refresh and advance proof identity to that accepted build.
    /// Covered empty zones remove retained geometry; uncovered baseline zones remain untouched.
    pub(super) fn refresh(
        &mut self,
        stamp: ObjectOutputStamp,
        update: ObjectOutputSnapshot,
    ) -> Result<(), ObjectOutputStateError> {
        validate_object_output_producer(self.key, &update)?;
        self.snapshot.merge(update)?;
        self.stamp = stamp;
        Ok(())
    }

    /// Replace retained output after an accepted complete producer build.
    pub(super) fn replace(
        &mut self,
        stamp: ObjectOutputStamp,
        snapshot: ObjectOutputSnapshot,
    ) -> Result<(), ObjectOutputStateError> {
        validate_object_output_producer(self.key, &snapshot)?;
        self.snapshot = snapshot;
        self.stamp = stamp;
        Ok(())
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn validate_object_output_producer(
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

/// Tracks the editor-side identity of FIFO region-build requests.
///
/// `RegionStreamer` processes one request at a time, so build results arrive in build-request order
/// even when edit, presentation, outline, or HD events are interleaved. Keeping this ledger outside
/// the worker lets the editor reject stale results without making semantic revisions part of the
/// world crate's public protocol.
#[derive(Debug, Default)]
pub(super) struct BuildTracker {
    next_request_id: u64,
    projection_epochs: HashMap<RegionKey, u64>,
    pending_results: VecDeque<PendingBuild>,
    current: HashMap<RegionKey, u64>,
}

impl BuildTracker {
    pub(super) fn len(&self) -> usize {
        self.current.len()
    }

    pub(super) fn contains(&self, key: &RegionKey) -> bool {
        self.current.contains_key(key)
    }

    /// Record one build immediately before it is sent to the single FIFO worker.
    pub(super) fn request(
        &mut self,
        key: RegionKey,
        scope: RegionBuildScope,
        presentation_generation: u32,
    ) {
        debug_assert!(!self.current.contains_key(&key));
        self.next_request_id = self.next_request_id.wrapping_add(1);
        let request_id = self.next_request_id;
        let stamp = BuildStamp {
            request_id,
            projection_epoch: self.projection_epoch(key),
            presentation_generation,
        };
        self.pending_results
            .push_back(PendingBuild { key, scope, stamp });
        self.current.insert(key, request_id);
    }

    /// Mark derived output for `key` obsolete after a newer edit.
    pub(super) fn invalidate(&mut self, key: RegionKey) {
        let epoch = self.projection_epochs.entry(key).or_default();
        *epoch = epoch.wrapping_add(1);
    }

    /// Match one Region/Empty/Failed worker event to the build that produced it.
    ///
    /// A matching old request is retired even when stale so queued dirty work can be scheduled on
    /// the next frame. If the region was cancelled and requested again, the old result leaves the
    /// newer in-flight request intact.
    pub(super) fn finish(
        &mut self,
        key: RegionKey,
        presentation_generation: u32,
    ) -> Option<RegionBuildScope> {
        let pending = self.pending_results.pop_front()?;
        if pending.key != key {
            debug_assert_eq!(
                pending.key, key,
                "region build results must preserve FIFO order"
            );
            self.pending_results.push_front(pending);
            return None;
        }

        let is_current_request = self.current.get(&key).copied() == Some(pending.stamp.request_id);
        if is_current_request {
            self.current.remove(&key);
        }

        let current_projection_epoch = self.projection_epoch(key);
        (is_current_request
            && pending.stamp.projection_epoch == current_projection_epoch
            && pending.stamp.presentation_generation == presentation_generation)
            .then_some(pending.scope)
    }

    /// Stop treating a request as current while retaining its FIFO result ticket.
    pub(super) fn cancel(&mut self, key: RegionKey) {
        self.current.remove(&key);
    }

    /// Presentation changes invalidate every current request, but those old results still arrive on
    /// the same worker and must stay represented in the FIFO ledger.
    pub(super) fn cancel_all(&mut self) {
        self.current.clear();
    }

    /// A source swap replaces the worker/receiver entirely, so old result tickets will never arrive.
    pub(super) fn reset_source(&mut self) {
        self.current.clear();
        self.pending_results.clear();
        self.projection_epochs.clear();
    }

    pub(super) fn projection_epoch(&self, key: RegionKey) -> u64 {
        self.projection_epochs.get(&key).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_render::{SceneGeometry, ZoneGeometry, ZoneGroup};
    use std::collections::BTreeSet;

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
    fn object_output_stale_projection_or_presentation_never_returns_exact_zones() {
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
    fn object_output_current_complete_state_returns_exact_changed_zones() {
        let key = (0, 0);
        let stamp = ObjectOutputStamp::new(2, 9);
        let state = ObjectOutputState::new(key, stamp, full(key, &[((0, 0), 0), ((1, 0), 0)]))
            .unwrap_or_else(|error| unreachable!("valid state: {error:?}"));
        let candidate = full(key, &[((1, 0), 2), ((2, 0), 0)]);

        assert_eq!(state.snapshot().producer_region(), RegionCoord::new(0, 0));
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
    fn object_output_incomplete_candidate_fails_closed() {
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
    fn object_output_accepted_scoped_refresh_preserves_baseline_and_advances_stamp() {
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

        let replacement = full(key, &[((2, 0), 1)]);
        let replacement_stamp = ObjectOutputStamp::new(6, 2);
        state
            .replace(replacement_stamp, replacement.clone())
            .unwrap_or_else(|error| unreachable!("accepted replacement: {error:?}"));
        assert_eq!(state.stamp(), replacement_stamp);
        assert_eq!(state.snapshot(), &replacement);
    }

    #[test]
    fn object_output_producer_mismatch_is_explicit() {
        let snapshot = full((1, 0), &[]);
        assert!(matches!(
            ObjectOutputState::new((0, 0), ObjectOutputStamp::new(0, 1), snapshot),
            Err(ObjectOutputStateError::ProducerKeyMismatch { .. })
        ));
    }

    #[test]
    fn current_result_finishes_with_original_scope() {
        let mut tracker = BuildTracker::default();
        let scope = RegionBuildScope::Zones(BTreeSet::from([(400, 400), (401, 400)]));
        tracker.request((50, 50), scope.clone(), 7);

        assert_eq!(tracker.finish((50, 50), 7), Some(scope));
        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn edit_epoch_rejects_old_result_and_retires_request() {
        let mut tracker = BuildTracker::default();
        tracker.request((50, 50), RegionBuildScope::Full, 7);
        tracker.invalidate((50, 50));

        assert_eq!(tracker.finish((50, 50), 7), None);
        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn cancelled_old_result_cannot_consume_newer_request() {
        let mut tracker = BuildTracker::default();
        tracker.request((50, 50), RegionBuildScope::Full, 7);
        tracker.cancel((50, 50));
        tracker.request(
            (50, 50),
            RegionBuildScope::Zones(BTreeSet::from([(400, 400)])),
            7,
        );

        assert_eq!(tracker.finish((50, 50), 7), None);
        assert_eq!(tracker.len(), 1);
        assert_eq!(
            tracker.finish((50, 50), 7),
            Some(RegionBuildScope::Zones(BTreeSet::from([(400, 400)])))
        );
        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn presentation_change_rejects_old_result() {
        let mut tracker = BuildTracker::default();
        tracker.request((50, 50), RegionBuildScope::Full, 7);
        tracker.cancel_all();

        assert_eq!(tracker.finish((50, 50), 8), None);
    }

    #[test]
    fn source_reset_discards_unreachable_old_tickets() {
        let mut tracker = BuildTracker::default();
        tracker.request((50, 50), RegionBuildScope::Full, 7);
        tracker.invalidate((50, 50));
        tracker.reset_source();
        tracker.request((50, 50), RegionBuildScope::Full, 1);

        assert_eq!(tracker.finish((50, 50), 1), Some(RegionBuildScope::Full));
    }
}
