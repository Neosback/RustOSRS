//! Static GPU zone residency retains provenance from the same accepted build as inspection data.
use super::{RegionKey, RenderRuntime, ZoneKey, zone_belongs_to_region, zone_renderer_key};
use crate::app::{
    ANIMATED_KEY,
    world_runtime::{AnimationArtifact, AnimationArtifactIdentity, RegionArtifactIdentity},
};
use osrs_core::coords::MapTile;
use osrs_render::SceneGeometry;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

impl RenderRuntime {
    /// Replace one streamed region, storing each static 8x8 zone under its own opaque renderer key.
    pub(in crate::app) fn upload_region(
        &mut self,
        identity: Arc<RegionArtifactIdentity>,
        geometry: &SceneGeometry,
    ) {
        self.reuse.invalidate();
        let region = identity.region();
        let key = (region.x, region.y);
        let incoming: BTreeMap<ZoneKey, Arc<RegionArtifactIdentity>> = geometry
            .zones
            .iter()
            .map(|zone| ((zone.zone_x, zone.zone_z), Arc::clone(&identity)))
            .collect();
        let previous = self.region_zones.remove(&key).unwrap_or_default();

        for &stale in previous.keys().filter(|zone| !incoming.contains_key(zone)) {
            self.scene.remove_region(zone_renderer_key(key, stale));
        }
        for zone in &geometry.zones {
            let zone_key = (zone.zone_x, zone.zone_z);
            debug_assert!(zone_belongs_to_region(key, zone_key));
            self.scene
                .upload_zones(zone_renderer_key(key, zone_key), std::iter::once(zone));
        }
        self.region_zones.insert(key, incoming);
    }

    /// Refresh only the requested world zones of an already-resident streamed region.
    ///
    /// `geometry` may be a full worker result; only requested zones are installed. A requested zone
    /// absent from the result is removed, which handles edits that make its geometry empty.
    pub(in crate::app) fn upload_region_zones(
        &mut self,
        identity: Arc<RegionArtifactIdentity>,
        geometry: &SceneGeometry,
        requested: &BTreeSet<ZoneKey>,
    ) {
        self.reuse.invalidate();
        let region = identity.region();
        let key = (region.x, region.y);
        let resident = self.region_zones.entry(key).or_default();
        for &zone_key in requested {
            debug_assert!(zone_belongs_to_region(key, zone_key));
            let renderer_key = zone_renderer_key(key, zone_key);
            match geometry
                .zones
                .iter()
                .find(|zone| (zone.zone_x, zone.zone_z) == zone_key)
            {
                Some(zone) => {
                    self.scene.upload_zones(renderer_key, std::iter::once(zone));
                    resident.insert(zone_key, Arc::clone(&identity));
                }
                None => {
                    self.scene.remove_region(renderer_key);
                    resident.remove(&zone_key);
                }
            }
        }
    }

    /// Upload geometry that is not part of streamed static-region residency (animated locs).
    pub(in crate::app) fn upload_transient(&mut self, key: (i32, i32), geometry: &SceneGeometry) {
        self.reuse.invalidate();
        self.scene.upload_region(key, geometry);
        if key == ANIMATED_KEY {
            self.animated_identity = None;
        }
        if key == crate::app::npc_runtime::NPC_KEY {
            self.npc_identity = None;
        }
        if key == crate::app::player_runtime::PLAYER_KEY {
            self.player_identity = None;
        }
    }

    /// Publish one accepted aggregate and its captured inputs together. An empty output removes
    /// the previous image and metadata; a no-op worker completion never reaches this operation.
    pub(in crate::app) fn upload_animation(&mut self, artifact: &AnimationArtifact) {
        self.upload_transient(ANIMATED_KEY, &artifact.geometry);
        if artifact.geometry.vertex_count() != 0 {
            self.animated_identity = Some(Arc::clone(&artifact.identity));
        }
    }

    pub(in crate::app) fn animation_identity(&self) -> Option<&AnimationArtifactIdentity> {
        self.animated_identity.as_deref()
    }

    pub(in crate::app) fn remove_region(&mut self, key: RegionKey) {
        self.reuse.invalidate();
        if let Some(zones) = self.region_zones.remove(&key) {
            for zone in zones.into_keys() {
                self.scene.remove_region(zone_renderer_key(key, zone));
            }
        }
    }

    pub(in crate::app) fn static_identity(&self, tile: MapTile) -> Option<&RegionArtifactIdentity> {
        let region = tile.split_region().0;
        let zone = (tile.x.div_euclid(8), tile.y.div_euclid(8));
        self.region_zones
            .get(&(region.x, region.y))?
            .get(&zone)
            .map(AsRef::as_ref)
    }
}
