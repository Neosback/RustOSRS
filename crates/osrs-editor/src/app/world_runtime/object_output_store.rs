//! Accepted Object producer-output state and in-flight comparison planning.
//!
//! The semantic document remains authoritative. This store retains only disposable accepted static
//! output so a later Object edit can compare one full post-edit producer build with the exact
//! pre-edit output that is currently installed. Any stale, missing, incomplete, or invalid state
//! fails closed to a full producer install.

use super::{
    RegionBuildScope, RegionKey,
    builds::{ObjectOutputLookup, ObjectOutputStamp, ObjectOutputState},
};
use osrs_core::coords::RegionCoord;
use osrs_render::SceneGeometry;
use osrs_world::ObjectOutputSnapshot;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct ObjectOutputStore {
    states: HashMap<RegionKey, ObjectOutputState>,
    /// Pre-edit accepted baseline stamps for full builds requested specifically to prove Object
    /// output changes. The build tracker independently gates the post-edit candidate result.
    pending: HashMap<RegionKey, ObjectOutputStamp>,
}

impl ObjectOutputStore {
    pub(super) fn clear(&mut self) {
        self.states.clear();
        self.pending.clear();
    }

    pub(super) fn remove(&mut self, key: RegionKey) {
        self.states.remove(&key);
        self.pending.remove(&key);
    }

    pub(super) fn cancel_pending(&mut self, key: RegionKey) {
        self.pending.remove(&key);
    }

    /// Return the exact accepted baseline stamp only when the retained producer output is both
    /// current and complete. Self-comparison is exact and empty only for a complete snapshot.
    pub(super) fn current_complete(
        &self,
        key: RegionKey,
        current: ObjectOutputStamp,
    ) -> Option<ObjectOutputStamp> {
        let state = self.states.get(&key)?;
        match state.compare(current, state.snapshot()).ok()? {
            ObjectOutputLookup::Current(zones) if zones.is_empty() => Some(state.stamp()),
            ObjectOutputLookup::Stale
            | ObjectOutputLookup::Incomplete
            | ObjectOutputLookup::Current(_) => None,
        }
    }

    pub(super) fn begin_proof(&mut self, key: RegionKey, baseline: ObjectOutputStamp) {
        self.pending.insert(key, baseline);
    }

    /// Resolve one accepted build into its static install scope. A normal build keeps its requested
    /// scope. A pending Object proof consumes a full candidate, compares it with the retained
    /// pre-edit baseline, and narrows only when both snapshots are exact and complete.
    pub(super) fn resolve_scope(
        &mut self,
        key: RegionKey,
        requested: RegionBuildScope,
        geometry: &SceneGeometry,
    ) -> RegionBuildScope {
        if !matches!(&requested, RegionBuildScope::Full) {
            self.pending.remove(&key);
            return requested;
        }
        let Some(baseline) = self.pending.remove(&key) else {
            return requested;
        };
        let Some(state) = self.states.get(&key) else {
            return RegionBuildScope::Full;
        };
        let candidate = match ObjectOutputSnapshot::full(RegionCoord::new(key.0, key.1), geometry) {
            Ok(candidate) => candidate,
            Err(_) => return RegionBuildScope::Full,
        };
        match state.compare(baseline, &candidate) {
            Ok(ObjectOutputLookup::Current(zones)) => RegionBuildScope::Zones(zones),
            Ok(ObjectOutputLookup::Stale | ObjectOutputLookup::Incomplete) | Err(_) => {
                RegionBuildScope::Full
            }
        }
    }

    /// Install proof state only after the worker result passed request/generation freshness gates and
    /// the renderer accepted the corresponding static scope.
    pub(super) fn install(
        &mut self,
        key: RegionKey,
        stamp: ObjectOutputStamp,
        scope: &RegionBuildScope,
        geometry: &SceneGeometry,
    ) {
        let snapshot = match snapshot_for_scope(key, scope, geometry) {
            Ok(snapshot) => snapshot,
            Err(()) => {
                self.states.remove(&key);
                return;
            }
        };

        let result = match self.states.get_mut(&key) {
            Some(state) => match scope {
                RegionBuildScope::Full => state.replace(stamp, snapshot),
                RegionBuildScope::Zones(_) => state.refresh(stamp, snapshot),
            },
            None => ObjectOutputState::new(key, stamp, snapshot).map(|state| {
                self.states.insert(key, state);
            }),
        };
        if result.is_err() {
            self.states.remove(&key);
        }
    }
}

fn snapshot_for_scope(
    key: RegionKey,
    scope: &RegionBuildScope,
    geometry: &SceneGeometry,
) -> Result<ObjectOutputSnapshot, ()> {
    let region = RegionCoord::new(key.0, key.1);
    match scope {
        RegionBuildScope::Full => ObjectOutputSnapshot::full(region, geometry).map_err(|_| ()),
        RegionBuildScope::Zones(zones) => {
            let filtered = SceneGeometry {
                zones: geometry
                    .zones
                    .iter()
                    .filter(|zone| zones.contains(&(zone.zone_x, zone.zone_z)))
                    .cloned()
                    .collect(),
            };
            ObjectOutputSnapshot::scoped(region, zones.iter().copied(), &filtered).map_err(|_| ())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_render::{ZoneGeometry, ZoneGroup};
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

    #[test]
    fn only_current_complete_state_can_begin_object_proof() {
        let key = (0, 0);
        let stamp = ObjectOutputStamp::new(3, 7);
        let mut store = ObjectOutputStore::default();
        store.install(
            key,
            stamp,
            &RegionBuildScope::Full,
            &geometry(&[((0, 0), 0)]),
        );

        assert_eq!(store.current_complete(key, stamp), Some(stamp));
        assert_eq!(
            store.current_complete(key, ObjectOutputStamp::new(4, 7)),
            None
        );

        let scoped = RegionBuildScope::Zones(BTreeSet::from([(0, 0)]));
        let mut incomplete = ObjectOutputStore::default();
        incomplete.install(key, stamp, &scoped, &geometry(&[((0, 0), 0)]));
        assert_eq!(incomplete.current_complete(key, stamp), None);
    }

    #[test]
    fn accepted_full_candidate_resolves_to_exact_changed_zones() {
        let key = (0, 0);
        let baseline = ObjectOutputStamp::new(2, 5);
        let mut store = ObjectOutputStore::default();
        store.install(
            key,
            baseline,
            &RegionBuildScope::Full,
            &geometry(&[((0, 0), 0), ((1, 0), 0)]),
        );
        store.begin_proof(key, baseline);

        assert_eq!(
            store.resolve_scope(
                key,
                RegionBuildScope::Full,
                &geometry(&[((1, 0), 2), ((2, 0), 0)]),
            ),
            RegionBuildScope::Zones(BTreeSet::from([(0, 0), (1, 0), (2, 0)]))
        );
    }

    #[test]
    fn stale_or_missing_baseline_falls_back_to_full_install() {
        let key = (0, 0);
        let baseline = ObjectOutputStamp::new(2, 5);
        let mut store = ObjectOutputStore::default();
        store.begin_proof(key, baseline);
        assert_eq!(
            store.resolve_scope(key, RegionBuildScope::Full, &SceneGeometry::default()),
            RegionBuildScope::Full
        );

        store.install(
            key,
            baseline,
            &RegionBuildScope::Full,
            &SceneGeometry::default(),
        );
        store.begin_proof(key, ObjectOutputStamp::new(1, 5));
        assert_eq!(
            store.resolve_scope(key, RegionBuildScope::Full, &SceneGeometry::default()),
            RegionBuildScope::Full
        );
    }

    #[test]
    fn exact_empty_diff_advances_complete_state_without_static_reupload() {
        let key = (0, 0);
        let before = ObjectOutputStamp::new(1, 4);
        let after = ObjectOutputStamp::new(2, 4);
        let full = geometry(&[((0, 0), 0), ((1, 0), 0)]);
        let mut store = ObjectOutputStore::default();
        store.install(key, before, &RegionBuildScope::Full, &full);
        store.begin_proof(key, before);
        let scope = store.resolve_scope(key, RegionBuildScope::Full, &full);
        assert_eq!(scope, RegionBuildScope::Zones(BTreeSet::new()));

        store.install(key, after, &scope, &full);
        assert_eq!(store.current_complete(key, after), Some(after));
    }
}
