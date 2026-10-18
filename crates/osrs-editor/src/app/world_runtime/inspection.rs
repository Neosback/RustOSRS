//! CPU picking and outline projections for resident world regions. Selection belongs to
//! `InteractionRuntime`; these caches are disposable and never answer semantic edit queries.
use super::{RegionKey, WorldRuntime};
use crate::pick::Infos;
use osrs_core::coords::MapTile;
use osrs_scene::collision::CollisionSnapshot;
use osrs_world::{FloorTable, RegionInfo};
use std::{collections::HashMap, sync::Arc};
use web_time::Instant;
type OutlineKey = (RegionKey, usize);
type Triangles = Arc<Vec<[[i32; 3]; 3]>>;

#[derive(Default)]
pub(super) struct InspectionProjection {
    infos: Infos,
    info_epochs: HashMap<RegionKey, u64>,
    version: u64,
    outlines: HashMap<OutlineKey, Triangles>,
    /// The floor definitions the terrain builder reads, for appearance previews.
    floors: Option<Arc<FloorTable>>,
    /// Outlines asked of the worker, with the request time (latency observation only).
    requested: HashMap<OutlineKey, (osrs_world::OutlineIdentity, Instant)>,
    next_outline: u64,
    roofs: crate::app::roof_view::RoofCache,
}

pub(in crate::app) enum CollisionInspection<'a> {
    Ready(&'a CollisionSnapshot),
    Updating,
    Unavailable,
}

fn collision_view(
    snapshot: Option<&CollisionSnapshot>,
    installed_epoch: Option<u64>,
    current_epoch: u64,
    rebuilding: bool,
) -> CollisionInspection<'_> {
    match snapshot {
        None => CollisionInspection::Unavailable,
        Some(_) if installed_epoch != Some(current_epoch) || rebuilding => {
            CollisionInspection::Updating
        }
        Some(snapshot) => CollisionInspection::Ready(snapshot),
    }
}

impl WorldRuntime {
    pub(in crate::app) fn request_outline(
        &self,
        identity: osrs_world::OutlineIdentity,
        region: RegionKey,
        index: usize,
        loc: osrs_world::LocInfo,
    ) {
        self.streamer.request_outline(identity, region, index, loc);
    }

    pub(in crate::app) fn roofs_over(
        &mut self,
        plane: u8,
        focus: &[(i32, i32)],
    ) -> std::collections::HashSet<u32> {
        let inspection = &mut self.inspection;
        inspection
            .roofs
            .query(inspection.version, plane, focus, &|p, x, y| {
                crate::app::roof_view::tile_flags(&inspection.infos, p, (x, y))
            })
    }

    pub(in crate::app) fn collision_inspection(&self, tile: MapTile) -> CollisionInspection<'_> {
        let region = tile.split_region().0;
        let key = (region.x, region.y);
        collision_view(
            self.inspection
                .infos
                .get(&key)
                .map(|info| info.collision.as_ref()),
            self.inspection.info_epochs.get(&key).copied(),
            self.builds.projection_epoch(key),
            self.dirty_regions.contains(&key)
                || self.dirty_zones.contains_key(&key)
                || self.builds.contains(&key),
        )
    }

    pub(in crate::app) fn infos(&self) -> &Infos {
        &self.inspection.infos
    }
    pub(in crate::app) fn infos_version(&self) -> u64 {
        self.inspection.version
    }
    pub(in crate::app) fn set_floors(&mut self, floors: Arc<FloorTable>) {
        self.inspection.floors = Some(floors);
    }
    pub(in crate::app) fn floors(&self) -> Option<&Arc<FloorTable>> {
        self.inspection.floors.as_ref()
    }
    pub(in crate::app) fn install_info(&mut self, key: RegionKey, info: Arc<RegionInfo>) {
        self.inspection
            .info_epochs
            .insert(key, self.builds.projection_epoch(key));
        self.inspection
            .outlines
            .retain(|(region, _), _| *region != key);
        self.inspection
            .requested
            .retain(|(region, _), _| *region != key);
        self.inspection.infos.insert(key, info);
        self.inspection.version += 1;
    }
    pub(in crate::app) fn remove_info(&mut self, key: RegionKey) {
        self.inspection
            .outlines
            .retain(|(region, _), _| *region != key);
        self.inspection
            .requested
            .retain(|(region, _), _| *region != key);
        self.inspection.infos.remove(&key);
        self.inspection.info_epochs.remove(&key);
        self.inspection.version += 1;
    }
    pub(in crate::app) fn clear_inspection(&mut self) {
        self.inspection.infos.clear();
        self.inspection.info_epochs.clear();
        self.inspection.version += 1;
        self.clear_outlines();
    }
    pub(in crate::app) fn clear_outlines(&mut self) {
        self.inspection.outlines.clear();
        self.inspection.requested.clear();
    }
    pub(in crate::app) fn install_outline(
        &mut self,
        key: OutlineKey,
        identity: osrs_world::OutlineIdentity,
        triangles: Vec<[[i32; 3]; 3]>,
    ) -> Option<f32> {
        self.inspection.accept_outline(
            key,
            identity,
            self.builds.source_generation(),
            self.builds.projection_epoch(key.0),
            triangles,
        )
    }
    pub(in crate::app) fn outline(&self, key: &OutlineKey) -> Option<&Triangles> {
        self.inspection.outlines.get(key)
    }
    pub(in crate::app) fn queue_outline(&mut self, key: OutlineKey) {
        if self.inspection.outlines.contains_key(&key)
            || self.inspection.requested.contains_key(&key)
        {
            return;
        }
        if let Some(loc) = self
            .inspection
            .infos
            .get(&key.0)
            .and_then(|info| info.locs.get(key.1))
            .cloned()
        {
            self.inspection.next_outline += 1;
            let identity = osrs_world::OutlineIdentity {
                source_generation: self.builds.source_generation(),
                projection_epoch: self.builds.projection_epoch(key.0),
                request_id: self.inspection.next_outline,
            };
            self.inspection
                .requested
                .insert(key, (identity, Instant::now()));
            self.request_outline(identity, key.0, key.1, loc);
        }
    }
}

impl InspectionProjection {
    fn accept_outline(
        &mut self,
        key: OutlineKey,
        identity: osrs_world::OutlineIdentity,
        source: u64,
        epoch: u64,
        triangles: Vec<[[i32; 3]; 3]>,
    ) -> Option<f32> {
        let (wanted, started) = self.requested.get(&key)?;
        if *wanted != identity
            || identity.source_generation != source
            || identity.projection_epoch != epoch
        {
            return None;
        }
        let elapsed = started.elapsed().as_secs_f32() * 1000.0;
        self.outlines.insert(key, Arc::new(triangles));
        self.requested.remove(&key);
        Some(elapsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_scene::collision::CollisionSnapshotBuilder;

    #[test]
    fn outlines_reject_old_requests_even_when_the_same_index_was_requested_again() {
        let mut projection = InspectionProjection::default();
        let key = ((50, 50), 7);
        let old = osrs_world::OutlineIdentity {
            source_generation: 1,
            projection_epoch: 2,
            request_id: 3,
        };
        let current = osrs_world::OutlineIdentity {
            request_id: 4,
            ..old
        };
        projection.requested.insert(key, (current, Instant::now()));
        assert!(projection.accept_outline(key, old, 1, 2, vec![]).is_none());
        assert!(
            projection
                .accept_outline(key, current, 2, 2, vec![])
                .is_none()
        );
        assert!(
            projection
                .accept_outline(key, current, 1, 3, vec![])
                .is_none()
        );
        assert!(
            projection
                .accept_outline(key, current, 1, 2, vec![])
                .is_some()
        );
        assert!(
            projection
                .accept_outline(key, current, 1, 2, vec![])
                .is_none()
        );
    }

    #[test]
    fn collision_view_hides_stale_pending_and_replaced_source_results() {
        let snapshot = CollisionSnapshotBuilder::reference(MapTile::new(0, 0)).build();
        assert!(matches!(
            collision_view(Some(&snapshot), Some(1), 1, false),
            CollisionInspection::Ready(_)
        ));
        // An edit advances the producer epoch; even a failed/cancelled build must not restore old evidence.
        assert!(matches!(
            collision_view(Some(&snapshot), Some(1), 2, false),
            CollisionInspection::Updating
        ));
        assert!(matches!(
            collision_view(Some(&snapshot), Some(2), 2, true),
            CollisionInspection::Updating
        ));
        assert!(matches!(
            collision_view(Some(&snapshot), None, 2, false),
            CollisionInspection::Updating
        ));
        // Only a newly accepted snapshot at the current epoch becomes available again.
        assert!(matches!(
            collision_view(Some(&snapshot), Some(2), 2, false),
            CollisionInspection::Ready(_)
        ));
        // Source replacement/unload clears the installed snapshot.
        assert!(matches!(
            collision_view(None, None, 2, false),
            CollisionInspection::Unavailable
        ));
    }
}
