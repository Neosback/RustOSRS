//! CPU picking and outline projections for resident world regions. Selection belongs to
//! `InteractionRuntime`; these caches are disposable and never answer semantic edit queries.
use super::{RegionKey, WorldRuntime};
use crate::pick::Infos;
use osrs_world::{FloorTable, RegionInfo};
use std::{collections::HashMap, sync::Arc};
use web_time::Instant;
type OutlineKey = (RegionKey, usize);
type Triangles = Arc<Vec<[[i32; 3]; 3]>>;

#[derive(Default)]
pub(super) struct InspectionProjection {
    infos: Infos,
    version: u64,
    outlines: HashMap<OutlineKey, Triangles>,
    /// The floor definitions the terrain builder reads, for appearance previews.
    floors: Option<Arc<FloorTable>>,
    /// Outlines asked of the worker, with the request time (latency observation only).
    requested: HashMap<OutlineKey, Instant>,
}
impl WorldRuntime {
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
            .outlines
            .retain(|(region, _), _| *region != key);
        self.inspection
            .requested
            .retain(|(region, _), _| *region != key);
        self.inspection.infos.insert(key, info);
        self.inspection.version += 1;
    }
    pub(in crate::app) fn remove_info(&mut self, key: RegionKey) {
        self.inspection.infos.remove(&key);
        self.inspection.version += 1;
    }
    pub(in crate::app) fn clear_inspection(&mut self) {
        self.inspection.infos.clear();
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
        triangles: Vec<[[i32; 3]; 3]>,
    ) -> Option<f32> {
        self.inspection.outlines.insert(key, Arc::new(triangles));
        self.inspection
            .requested
            .get(&key)
            .map(|requested| requested.elapsed().as_secs_f32() * 1000.0)
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
        self.inspection.requested.insert(key, Instant::now());
        if let Some(loc) = self
            .inspection
            .infos
            .get(&key.0)
            .and_then(|info| info.locs.get(key.1))
            .cloned()
        {
            self.request_outline(key.0, key.1, loc);
        }
    }
}
