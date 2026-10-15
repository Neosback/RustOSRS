use super::{RegionBuildScope, RegionKey};
use std::collections::{HashMap, VecDeque};

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
        self.pending_results.push_back(PendingBuild {
            key,
            scope,
            stamp,
        });
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
            debug_assert_eq!(pending.key, key, "region build results must preserve FIFO order");
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

    fn projection_epoch(&self, key: RegionKey) -> u64 {
        self.projection_epochs.get(&key).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

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
