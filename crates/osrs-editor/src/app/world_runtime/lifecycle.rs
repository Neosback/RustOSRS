//! Disposable region removal and scene shutdown; authored state belongs to the edit session.
use super::WorldRuntime;
impl WorldRuntime {
    /// Discard an absent, failed or semantically rejected region after its FIFO freshness gate.
    ///
    /// Both full and scoped terminal outcomes discard the whole region's projections. The unavailable
    /// residency entry prevents a per-frame retry loop without touching authored/session state.
    pub(in crate::app) fn region_unavailable(&mut self, key: super::RegionKey) {
        self.loaded.insert(key, None);
        self.height_dependencies.remove(key);
        self.object_outputs.remove(key);
        self.dirty_regions.remove(&key);
        self.dirty_zones.remove(&key);
        self.remove_info(key);
        self.remove_animation_region(key);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn shutdown_in_background(&mut self) {
        self.streamer.shutdown_in_background();
        self.animations.shutdown_in_background();
    }
    /// Configure the isolated stage before any scene requests are made.
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn use_virtual_map(&self, underlay: u16) {
        self.streamer.use_virtual_map(underlay);
    }
}

#[cfg(test)]
mod tests;
