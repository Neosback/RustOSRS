//! GPU scene ownership for the editor.
//!
//! Phase 2.1 moves rendering state out of `EditorApp` incrementally. This first boundary owns the
//! sole `SceneRenderer`. Viewport targets and frame submission remain in the composition root until
//! the next focused RenderRuntime checkpoint.

use eframe::egui_wgpu;
use osrs_render::gpu::SceneRenderer;
use std::ops::{Deref, DerefMut};

/// Runtime owner of the editor's GPU scene renderer.
pub(super) struct RenderRuntime {
    scene: SceneRenderer,
}

impl RenderRuntime {
    pub(super) fn new(render_state: &egui_wgpu::RenderState, origin: (i32, i32)) -> Self {
        let mut scene = SceneRenderer::with_device(
            render_state.device.clone(),
            render_state.queue.clone(),
        );
        scene.set_render_origin(origin);
        Self { scene }
    }
}

// Temporary Phase 2.1 compatibility seam. Existing feature modules still call SceneRenderer APIs
// while ownership is extracted one boundary at a time. Remove these impls once those callers use
// narrow RenderRuntime methods in the following checkpoint.
impl Deref for RenderRuntime {
    type Target = SceneRenderer;

    fn deref(&self) -> &Self::Target {
        &self.scene
    }
}

impl DerefMut for RenderRuntime {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.scene
    }
}
