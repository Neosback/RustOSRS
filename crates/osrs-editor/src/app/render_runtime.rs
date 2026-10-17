//! GPU renderer, viewport targets, and frame submission owned by the editor runtime.
mod reuse;
mod theme;

use eframe::{egui, egui_wgpu, wgpu};
use osrs_hd::shadows::ShadowSettings;
use osrs_render::{
    SceneGeometry,
    gpu::{
        COLOR_FORMAT, FrameParams, HdLighting, HdParams, LineVertex, SceneRenderer, TextureLayer,
    },
};
use osrs_world::hd::gpu::HdGpuData;
use std::collections::{BTreeMap, BTreeSet};

const ZONES_PER_REGION_AXIS: i32 = 8;
const ZONES_PER_REGION: i32 = ZONES_PER_REGION_AXIS * ZONES_PER_REGION_AXIS;

type RegionKey = (i32, i32);
type ZoneKey = (i32, i32);

struct ViewportTargets {
    width: u32,
    height: u32,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    texture_id: egui::TextureId,
}

impl Drop for RenderRuntime {
    fn drop(&mut self) {
        if let Some(targets) = self.targets.take() {
            self.render_state
                .renderer
                .write()
                .free_texture(&targets.texture_id);
        }
    }
}

pub(super) struct EncodedViewport {
    pub(super) command_buffer: Option<wgpu::CommandBuffer>,
    params: FrameParams,
    pub(super) texture_id: egui::TextureId,
}

/// Owns the editor's disposable GPU projection state.
pub(super) struct RenderRuntime {
    render_state: egui_wgpu::RenderState,
    scene: SceneRenderer,
    /// World-zone keys currently installed for each streamed 64x64 map region.
    region_zones: BTreeMap<RegionKey, BTreeSet<ZoneKey>>,
    targets: Option<ViewportTargets>,
    reuse: reuse::ViewportReuse,
    theme_preparation: Option<osrs_render::gpu::theme::ThemeShaderPreparation>,
    theme_shaders: Vec<osrs_hd::theme::shader::ShaderExtension>,
    prepared_theme_shaders: Vec<osrs_hd::theme::shader::ShaderExtension>,
    rejected_theme_shaders: Option<Vec<osrs_hd::theme::shader::ShaderExtension>>,
    reuse_enabled: bool,
}

impl RenderRuntime {
    /// Share eframe's device with an isolated viewport; each runtime owns its own scene resources.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn render_state(&self) -> egui_wgpu::RenderState {
        self.render_state.clone()
    }
    pub(super) fn new(render_state: egui_wgpu::RenderState, origin: (i32, i32)) -> Self {
        let mut scene =
            SceneRenderer::with_device(render_state.device.clone(), render_state.queue.clone());
        scene.set_render_origin(origin);
        Self {
            render_state,
            scene,
            region_zones: BTreeMap::new(),
            targets: None,
            reuse: reuse::ViewportReuse::default(),
            theme_preparation: None,
            theme_shaders: Vec::new(),
            prepared_theme_shaders: Vec::new(),
            rejected_theme_shaders: None,
            reuse_enabled: std::env::var("RUSTOSRS_VIEWPORT_REUSE")
                .map_or(true, |value| value != "0"),
        }
    }

    pub(super) const fn render_origin(&self) -> (i32, i32) {
        self.scene.render_origin()
    }

    pub(super) fn set_render_origin(&mut self, origin: (i32, i32)) {
        self.reuse.invalidate();
        self.scene.set_render_origin(origin);
    }

    pub(super) fn set_textures(&mut self, layers: &[Option<TextureLayer>]) {
        self.reuse.invalidate();
        self.scene.set_textures(layers);
    }

    /// Replace one streamed region, storing each static 8x8 zone under its own opaque renderer key.
    pub(super) fn upload_region(&mut self, key: RegionKey, geometry: &SceneGeometry) {
        self.reuse.invalidate();
        let incoming: BTreeSet<ZoneKey> = geometry
            .zones
            .iter()
            .map(|zone| (zone.zone_x, zone.zone_z))
            .collect();
        let previous = self.region_zones.remove(&key).unwrap_or_default();

        for stale in previous.difference(&incoming) {
            self.scene.remove_region(zone_renderer_key(key, *stale));
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
    pub(super) fn upload_region_zones(
        &mut self,
        key: RegionKey,
        geometry: &SceneGeometry,
        requested: &BTreeSet<ZoneKey>,
    ) {
        self.reuse.invalidate();
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
                    resident.insert(zone_key);
                }
                None => {
                    self.scene.remove_region(renderer_key);
                    resident.remove(&zone_key);
                }
            }
        }
    }

    /// Upload geometry that is not part of streamed static-region residency (animated locs).
    pub(super) fn upload_transient(&mut self, key: (i32, i32), geometry: &SceneGeometry) {
        self.reuse.invalidate();
        self.scene.upload_region(key, geometry);
    }

    pub(super) fn remove_region(&mut self, key: RegionKey) {
        self.reuse.invalidate();
        if let Some(zones) = self.region_zones.remove(&key) {
            for zone in zones {
                self.scene.remove_region(zone_renderer_key(key, zone));
            }
        }
    }

    pub(super) fn set_overlay_lines(&mut self, lines: &[LineVertex]) {
        self.reuse.invalidate();
        self.scene.set_overlay_lines(lines);
    }

    pub(super) fn set_hd_lighting(&mut self, lighting: HdLighting) {
        self.scene.set_hd_lighting(lighting);
    }

    pub(super) fn set_hd_point_lights(
        &mut self,
        lights: &[osrs_render::gpu::hd::layout::GpuPointLight],
    ) {
        self.scene.set_hd_point_lights(lights);
    }

    pub(super) fn set_hd_shadows(&mut self, settings: ShadowSettings) {
        self.scene.set_hd_shadows(settings);
    }

    pub(super) fn set_hd_params(&mut self, params: HdParams) {
        self.scene.set_hd_params(params);
    }

    pub(super) fn install_hd_materials(&mut self, data: HdGpuData) -> Result<(), String> {
        self.reuse.invalidate();
        data.install(&mut self.scene)
            .map_err(|error| error.to_string())
    }

    pub(super) fn resident_zone_count(&self) -> usize {
        self.scene.resident_zone_count()
    }

    pub(super) fn resident_vertex_bytes(&self) -> u64 {
        self.scene.resident_vertex_bytes()
    }

    pub(super) fn hd_shadow_map_bytes(&self) -> u64 {
        self.scene.hd_shadow_map_bytes()
    }

    /// Structured adapter, backend and device-limit identity for reproducible reports.
    pub(super) fn adapter_report(&self) -> serde_json::Value {
        let info = self.render_state.adapter.get_info();
        let limits = self.render_state.device.limits();
        serde_json::json!({
            "name": info.name, "vendor": info.vendor, "device": info.device,
            "device_type": format!("{:?}", info.device_type), "backend": format!("{:?}", info.backend),
            "driver": info.driver, "driver_info": info.driver_info,
            "timestamp_queries": self.render_state.device.features()
                .contains(eframe::wgpu::Features::TIMESTAMP_QUERY),
            "limits": {
                "max_texture_dimension_2d": limits.max_texture_dimension_2d,
                "max_texture_array_layers": limits.max_texture_array_layers,
                "max_buffer_size": limits.max_buffer_size,
                "max_storage_buffer_binding_size": limits.max_storage_buffer_binding_size,
            },
        })
    }

    pub(super) fn gpu_pass_samples(&self) -> BTreeMap<&'static str, (u32, f32)> {
        self.scene.gpu_pass_samples()
    }

    pub(super) fn take_counters(&self) -> osrs_render::gpu::RenderCounters {
        self.scene.take_counters()
    }

    pub(super) fn resource_bytes(&self) -> BTreeMap<&'static str, u64> {
        let mut bytes = self.scene.resource_bytes();
        if let Some(targets) = &self.targets {
            bytes.insert(
                "viewport_color",
                u64::from(targets.width) * u64::from(targets.height) * 4,
            );
        }
        bytes
    }

    pub(super) fn ensure_targets(&mut self, width: u32, height: u32) {
        if let Some(targets) = &self.targets
            && targets.width == width
            && targets.height == height
        {
            return;
        }
        self.reuse.invalidate();
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
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());

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
            texture_id,
        });
    }

    pub(super) fn encode_viewport(&self, params: FrameParams) -> Option<EncodedViewport> {
        let targets = self.targets.as_ref()?;
        if self.reuse_enabled && self.reuse.can_reuse(params) {
            return Some(EncodedViewport {
                command_buffer: None,
                texture_id: targets.texture_id,
                params,
            });
        }
        let mut encoder =
            self.render_state
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("viewport"),
                });
        self.scene.encode_color_frame(
            &mut encoder,
            &targets.color_view,
            params,
            targets.width,
            targets.height,
        );
        let _ = &targets.color;
        Some(EncodedViewport {
            command_buffer: Some(encoder.finish()),
            params,
            texture_id: targets.texture_id,
        })
    }

    pub(super) fn submit_viewport(&self, frame: &mut EncodedViewport) -> Option<f32> {
        if let Some(command_buffer) = frame.command_buffer.take() {
            self.render_state.queue.submit(Some(command_buffer));
            self.reuse.submitted(frame.params);
            self.scene.finish_frame();
        }
        self.scene.gpu_scene_ms()
    }

    pub(super) fn on_submitted_work_done(&self, callback: impl FnOnce() + Send + 'static) {
        self.render_state.queue.on_submitted_work_done(callback);
    }
}

fn zone_belongs_to_region(region: RegionKey, zone: ZoneKey) -> bool {
    zone.0.div_euclid(ZONES_PER_REGION_AXIS) == region.0
        && zone.1.div_euclid(ZONES_PER_REGION_AXIS) == region.1
}

/// Preserve the old renderer ordering: region x, region y, then zone x/y inside the region.
fn zone_renderer_key(region: RegionKey, zone: ZoneKey) -> (i32, i32) {
    let local_x = zone.0 - region.0 * ZONES_PER_REGION_AXIS;
    let local_y = zone.1 - region.1 * ZONES_PER_REGION_AXIS;
    debug_assert!((0..ZONES_PER_REGION_AXIS).contains(&local_x));
    debug_assert!((0..ZONES_PER_REGION_AXIS).contains(&local_y));
    (
        region.0,
        region.1 * ZONES_PER_REGION + local_x * ZONES_PER_REGION_AXIS + local_y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_zone_keys_preserve_region_and_local_order() {
        let region = (50, 60);
        assert_eq!(zone_renderer_key(region, (400, 480)), (50, 60 * 64));
        assert_eq!(zone_renderer_key(region, (407, 487)), (50, 60 * 64 + 63));
        assert!(zone_belongs_to_region(region, (403, 486)));
        assert!(!zone_belongs_to_region(region, (408, 486)));
    }
}
