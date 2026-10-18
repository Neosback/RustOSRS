use super::{RegionBuildScope, RegionKey, WorldRuntime, ZoneKey};
use crate::app::diagnostics::{DiagnosticSink, TraceEvent};
use osrs_core::coords::RegionCoord;
use osrs_world::{ObjectOutputComparison, ObjectOutputProofError, ObjectOutputSnapshot};
use std::collections::{BTreeSet, HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuildStamp {
    request_id: u64,
    revision: u64,
    source: u64,
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
    revision: u64,
    source: u64,
    diagnostics: DiagnosticSink,
    projection_epochs: HashMap<RegionKey, u64>,
    pending_results: VecDeque<PendingBuild>,
    current: HashMap<RegionKey, u64>,
}

impl WorldRuntime {
    /// Stamp newly scheduled work with the current semantic snapshot revision, including hydration.
    pub(in crate::app) fn set_semantic_revision(&mut self, revision: u64) {
        self.builds.revision = revision;
    }
}

impl BuildTracker {
    pub(super) fn set_diagnostics(&mut self, sink: DiagnosticSink) {
        self.diagnostics = sink;
    }
    pub(super) fn observe_invalidation(
        &mut self,
        revision: u64,
        regions: Vec<RegionCoord>,
        reason: &'static str,
    ) {
        self.revision = revision;
        self.diagnostics.record(|| TraceEvent::Invalidation {
            revision,
            regions,
            reason,
        });
    }
    fn observe_build(&self, region: RegionKey, stamp: BuildStamp, outcome: &'static str) {
        self.diagnostics.record(|| TraceEvent::Build {
            source: stamp.source,
            request: stamp.request_id,
            region: RegionCoord::new(region.0, region.1),
            revision: stamp.revision,
            epoch: stamp.projection_epoch,
            generation: stamp.presentation_generation,
            outcome,
        });
    }

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
            revision: self.revision,
            source: self.source,
            projection_epoch: self.projection_epoch(key),
            presentation_generation,
        };
        self.observe_build(key, stamp, "requested");
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
        let accepted = is_current_request
            && pending.stamp.projection_epoch == current_projection_epoch
            && pending.stamp.presentation_generation == presentation_generation;
        let outcome = if !is_current_request {
            "stale: cancelled or replaced request"
        } else if pending.stamp.projection_epoch != current_projection_epoch {
            "stale: newer edit"
        } else if pending.stamp.presentation_generation != presentation_generation {
            "stale: presentation changed"
        } else {
            "ticket accepted"
        };
        self.observe_build(key, pending.stamp, outcome);
        accepted.then_some(pending.scope)
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
        self.source = self.source.wrapping_add(1);
        self.revision = 0;
        self.diagnostics.clear();
        self.current.clear();
        self.pending_results.clear();
        self.projection_epochs.clear();
    }

    pub(super) fn source_generation(&self) -> u64 {
        self.source
    }

    pub(super) fn projection_epoch(&self, key: RegionKey) -> u64 {
        self.projection_epochs.get(&key).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests;
