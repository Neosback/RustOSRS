//! Disposable scene shutdown, keeping thread joins outside the frame thread.
use super::WorldRuntime;
impl WorldRuntime {
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
