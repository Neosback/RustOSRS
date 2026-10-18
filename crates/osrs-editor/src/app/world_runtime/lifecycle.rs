//! Disposable scene shutdown, keeping thread joins outside the frame thread.
use super::WorldRuntime;
impl WorldRuntime {
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn shutdown_in_background(&mut self) {
        self.streamer.shutdown_in_background();
        self.animations.shutdown_in_background();
    }
}
