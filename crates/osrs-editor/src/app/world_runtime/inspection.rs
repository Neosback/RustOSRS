//! CPU picking and outline projections for resident world regions. Selection belongs to
//! `InteractionRuntime`; these caches are disposable and never answer semantic edit queries.
use super::{RegionKey, WorldRuntime};
use crate::pick::Infos;
use osrs_world::RegionInfo;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
type OutlineKey = (RegionKey, usize);
type Triangles = Arc<Vec<[[i32; 3]; 3]>>;

#[derive(Default)]
pub(super) struct InspectionProjection {
    infos: Infos,
    version: u64,
    outlines: HashMap<OutlineKey, Triangles>,
    requested: HashSet<OutlineKey>,
}
impl WorldRuntime {
    pub(in crate::app) fn infos(&self) -> &Infos {
        &self.inspection.infos
    }
    pub(in crate::app) fn infos_version(&self) -> u64 {
        self.inspection.version
    }
    pub(in crate::app) fn install_info(&mut self, key: RegionKey, info: Arc<RegionInfo>) {
        self.inspection
            .outlines
            .retain(|(region, _), _| *region != key);
        self.inspection
            .requested
            .retain(|(region, _)| *region != key);
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
    ) {
        self.inspection.outlines.insert(key, Arc::new(triangles));
    }
    pub(in crate::app) fn outline(&self, key: &OutlineKey) -> Option<&Triangles> {
        self.inspection.outlines.get(key)
    }
    pub(in crate::app) fn queue_outline(&mut self, key: OutlineKey) {
        if self.inspection.outlines.contains_key(&key) || !self.inspection.requested.insert(key) {
            return;
        }
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
