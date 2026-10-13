//! GPU renderer, viewport targets, and frame submission owned by the editor runtime.

use eframe::{egui, egui_wgpu, wgpu};
use osrs_hd::shadows::ShadowSettings;
use osrs_render::{
    SceneGeometry,
    gpu::{
        COLOR_FORMAT, DEPTH_FORMAT, FrameParams, HdLighting, HdParams, LineVertex, SceneRenderer,
        TextureLayer,
    },
};
use osrs_world::hd::gpu::HdGpuData;

struct ViewportTargets {
    width: u32,
    height: u32,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    texture_id: egui::TextureId,
}

pub(super) struct EncodedViewport {
    pub(super) command_buffer: wgpu::CommandBuffer,
    pub(super) texture_id: egui::TextureId,
}

/// Owns the editor's disposable GPU projection state.
pub(super) struct RenderRuntime {
    render_state: egui_wgpu::RenderState,
    scene: SceneRenderer,
    targets: Option<ViewportTargets>,
}

impl RenderRuntime {
    pub(super) fn new(render_state: egui_wgpu::RenderState, origin: (i32, i32)) -> Self {
        let mut scene =
            SceneRenderer::with_device(render_state.device.clone(), render_state.queue.clone());
        scene.set_render_origin(origin);
        Self {
            render_state,
            scene,
            targets: None,
        }
    }

    pub(super) const fn render_origin(&self) -> (i32, i32) {
        self.scene.render_origin()
    }

    pub(super) fn set_render_origin(&mut self, origin: (i32, i32)) {
        self.scene.set_render_origin(origin);
    }

    pub(super) fn set_textures(&mut self, layers: &[Option<TextureLayer>]) {
        self.scene.set_textures(layers);
    }

    pub(super) fn upload_region(&mut self, key: (i32, i32), geometry: &SceneGeometry) {
        self.scene.upload_region(key, geometry);
    }

    pub(super) fn remove_region(&mut self, key: (i32, i32)) {
        self.scene.remove_region(key);
    }

    pub(super) fn set_overlay_lines(&mut self, lines: &[LineVertex]) {
        self.scene.set_overlay_lines(lines);
    }

    pub(super) fn set_hd_lighting(&mut self, lighting: HdLighting) {
        self.scene.set_hd_lighting(lighting);
    }

    pub(super) fn set_hd_shadows(&mut self, settings: ShadowSettings) {
        self.scene.set_hd_shadows(settings);
    }

    pub(super) fn set_hd_params(&mut self, params: HdParams) {
        self.scene.set_hd_params(params);
    }

    pub(super) fn install_hd_materials(&mut self, data: HdGpuData) -> Result<(), String> {
        data.install(&mut self.scene).map_err(|error| error.to_string())
    }

    pub(super) fn resident_zone_count(&self) -> usize {
        self.scene.resident_zone_count()
    }

    pub(super) fn resident_vertex_bytes(&self) -> u64 {
        self.scene.resident_vertex_bytes()
    }

    pub(super) fn ensure_targets(&mut self, width: u32, height: u32) {
        if let Some(targets) = &self.targets
            && targets.width == width
            && targets.height == height
        {
            return;
        }
        let device = &self.render_state.device;
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport-color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport-depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

        let mut egui_renderer = self.render_state.renderer.write();
        let texture_id = match self.targets.take() {
            Some(old) => {
                egui_renderer.update_egui_texture_from_wgpu_texture(
                    device,
                    &color_view,
                    wgpu::FilterMode::Nearest,
                    old.texture_id,
                );
                old.texture_id
            }
            None => egui_renderer.register_native_texture(
                device,
                &color_view,
                wgpu::FilterMode::Nearest,
            ),
        };
        self.targets = Some(ViewportTargets {
            width,
            height,
            color,
            color_view,
            depth_view,
            texture_id,
        });
    }

    pub(super) fn encode_viewport(&self, params: FrameParams) -> Option<EncodedViewport> {
        let targets = self.targets.as_ref()?;
        let mut encoder = self
            .render_state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("viewport"),
            });
        self.scene.encode_frame(
            &mut encoder,
            &targets.color_view,
            &targets.depth_view,
            params,
            targets.width,
            targets.height,
        );
        let _ = &targets.color;
        Some(EncodedViewport {
            command_buffer: encoder.finish(),
            texture_id: targets.texture_id,
        })
    }

    pub(super) fn submit_viewport(&self, command_buffer: wgpu::CommandBuffer) -> Option<f32> {
        self.render_state.queue.submit(Some(command_buffer));
        self.scene.finish_frame();
        self.scene.gpu_scene_ms()
    }

    pub(super) fn on_submitted_work_done(&self, callback: impl FnOnce() + Send + 'static) {
        self.render_state.queue.on_submitted_work_done(callback);
    }
}
