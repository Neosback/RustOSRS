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
    let candidate =
        ObjectOutputSnapshot::scoped(RegionCoord::new(0, 0), [(0, 0)], &geometry(&[((0, 0), 1)]))
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
    let update =
        ObjectOutputSnapshot::scoped(RegionCoord::new(0, 0), [(0, 0)], &geometry(&[((0, 0), 3)]))
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

    assert_eq!(finish_scope(&mut tracker, (50, 50), 7), Some(scope));
    assert_eq!(tracker.len(), 0);
}

#[test]
fn edit_epoch_rejects_old_result_and_retires_request() {
    let mut tracker = BuildTracker::default();
    tracker.request((50, 50), RegionBuildScope::Full, 7);
    tracker.invalidate((50, 50));

    assert_eq!(finish_scope(&mut tracker, (50, 50), 7), None);
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

    assert_eq!(finish_scope(&mut tracker, (50, 50), 7), None);
    assert_eq!(tracker.len(), 1);
    assert_eq!(
        finish_scope(&mut tracker, (50, 50), 7),
        Some(RegionBuildScope::Zones(BTreeSet::from([(400, 400)])))
    );
    assert_eq!(tracker.len(), 0);
}

#[test]
fn presentation_change_rejects_old_result() {
    let mut tracker = BuildTracker::default();
    tracker.request((50, 50), RegionBuildScope::Full, 7);
    tracker.cancel_all();

    assert_eq!(finish_scope(&mut tracker, (50, 50), 8), None);
}

#[test]
fn source_reset_discards_unreachable_old_tickets() {
    let mut tracker = BuildTracker::default();
    tracker.request((50, 50), RegionBuildScope::Full, 7);
    tracker.invalidate((50, 50));
    tracker.reset_source();
    tracker.request((50, 50), RegionBuildScope::Full, 1);

    assert_eq!(
        finish_scope(&mut tracker, (50, 50), 1),
        Some(RegionBuildScope::Full)
    );
}

#[test]
fn diagnostic_build_events_preserve_fifo_freshness_and_reset_source_identity() {
    use crate::app::diagnostics::DiagnosticSink;
    use osrs_core::coords::MapTile;
    let sink = DiagnosticSink::enabled();
    let mut tracker = BuildTracker::default();
    tracker.set_diagnostics(sink.clone());
    tracker.observe_invalidation(12, vec![RegionCoord::new(0, 0)], "terrain zones");
    tracker.request((0, 0), RegionBuildScope::Full, 3);
    tracker.invalidate((0, 0));
    assert_eq!(finish_scope(&mut tracker, (0, 0), 3), None);
    let lines = sink.explain(MapTile::new(0, 0), 0);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("revision 12, epoch 0, presentation 3"))
    );
    assert!(lines.iter().any(|line| line.contains("stale: newer edit")));
    tracker.reset_source();
    assert!(sink.explain(MapTile::new(0, 0), 0).is_empty());
    tracker.request((0, 0), RegionBuildScope::Full, 3);
    assert_eq!(
        finish_scope(&mut tracker, (0, 0), 3),
        Some(RegionBuildScope::Full)
    );
    assert!(
        sink.explain(MapTile::new(0, 0), 0)
            .iter()
            .any(|line| line.contains("build 1/2: ticket accepted"))
    );
}

fn finish_scope(
    tracker: &mut BuildTracker,
    key: RegionKey,
    generation: u32,
) -> Option<RegionBuildScope> {
    tracker.finish(key, generation).map(RegionBuild::into_scope)
}
