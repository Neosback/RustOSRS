//! Reuse only an exactly matching submitted Reference image. HD has continuous environment/
//! water time and is deliberately always encoded. GPU projection mutations invalidate the image.
use osrs_render::gpu::{FrameParams, RenderProfile};
use std::cell::Cell;

#[derive(Default)]
pub(super) struct ViewportReuse {
    submitted: Cell<Option<FrameParams>>,
}

impl ViewportReuse {
    pub(super) fn invalidate(&self) {
        self.submitted.set(None);
    }
    pub(super) fn can_reuse(&self, params: FrameParams) -> bool {
        params.profile == RenderProfile::Reference && self.submitted.get() == Some(params)
    }
    pub(super) fn submitted(&self, params: FrameParams) {
        self.submitted.set(Some(params));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_render::gpu::{Colorblind, Fog, ReferenceCamera};
    fn params() -> FrameParams {
        FrameParams {
            camera: ReferenceCamera {
                x: 0.0,
                y: -2000.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.75,
                scale: 512.0,
            },
            view_plane: 0,
            tick: 0,
            brightness: 0.8,
            remove_color_banding: true,
            clear_color: [0.0; 3],
            bright_textures: false,
            fog: Fog::default(),
            colorblind: Colorblind::default(),
            profile: RenderProfile::Reference,
        }
    }
    #[test]
    fn reuse_requires_a_submitted_image_and_is_invalidated_by_projection_changes() {
        let reuse = ViewportReuse::default();
        let params = params();
        assert!(!reuse.can_reuse(params));
        reuse.submitted(params);
        assert!(reuse.can_reuse(params));
        reuse.invalidate();
        assert!(!reuse.can_reuse(params));
    }
    #[test]
    fn animation_camera_visibility_and_settings_changes_require_a_new_image() {
        let reuse = ViewportReuse::default();
        let original = params();
        reuse.submitted(original);
        let mut changed = original;
        changed.tick += 1;
        assert!(!reuse.can_reuse(changed));
        changed = original;
        changed.camera.x += 1.0;
        assert!(!reuse.can_reuse(changed));
        changed = original;
        changed.view_plane = 1;
        assert!(!reuse.can_reuse(changed));
        changed = original;
        changed.brightness = 0.9;
        assert!(!reuse.can_reuse(changed));
        changed = original;
        changed.fog.draw_distance_tiles = 200;
        assert!(!reuse.can_reuse(changed));
    }
    #[test]
    fn hd_is_never_reused_even_when_frame_parameters_match() {
        let reuse = ViewportReuse::default();
        let mut params = params();
        params.profile = RenderProfile::Hd;
        reuse.submitted(params);
        assert!(!reuse.can_reuse(params));
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn gpu_viewport_reuses_only_after_submit_and_invalidates_on_upload_and_resize()
    -> Result<(), Box<dyn std::error::Error>> {
        use eframe::{egui_wgpu, wgpu};
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let state = match pollster::block_on(egui_wgpu::RenderState::create(
            &egui_wgpu::WgpuConfiguration::default(),
            &instance,
            None,
            egui_wgpu::RendererOptions::default(),
        )) {
            Ok(state) => state,
            Err(error) => {
                eprintln!("GPU viewport test unavailable: {error}");
                return Ok(());
            }
        };
        let mut runtime = super::super::RenderRuntime::new(state, (0, 0));
        runtime.reuse_enabled = true;
        runtime.ensure_targets(64, 48);
        let mut first = runtime
            .encode_viewport(params())
            .ok_or("missing viewport")?;
        assert!(first.command_buffer.is_some());
        // Encoding without submission cannot certify an output image.
        assert!(
            runtime
                .encode_viewport(params())
                .ok_or("missing viewport")?
                .command_buffer
                .is_some()
        );
        runtime.submit_viewport(&mut first);
        assert!(
            runtime
                .encode_viewport(params())
                .ok_or("missing viewport")?
                .command_buffer
                .is_none()
        );
        runtime.upload_transient((0, 0), &osrs_render::SceneGeometry::default());
        let mut changed = runtime
            .encode_viewport(params())
            .ok_or("missing viewport")?;
        assert!(changed.command_buffer.is_some());
        runtime.submit_viewport(&mut changed);
        runtime.ensure_targets(64, 49);
        assert!(
            runtime
                .encode_viewport(params())
                .ok_or("missing viewport")?
                .command_buffer
                .is_some()
        );
        assert_eq!(runtime.resource_bytes()["viewport_color"], 64 * 49 * 4);
        Ok(())
    }
}
