//! The eframe application: scene loading, the wgpu viewport, and fly-camera input.

use crate::camera::FlyCamera;
use eframe::{egui, egui_wgpu, wgpu};
use osrs_core::coords::RegionCoord;
use osrs_render::gpu::{COLOR_FORMAT, DEPTH_FORMAT, FrameParams, SceneRenderer};
use osrs_world::{RegionStreamer, StreamEvent, TerrainPresentation};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Instant,
};

/// Local units per map region (64 tiles of 128).
const REGION_UNITS: i32 = 64 * 128;
/// Rebase the render origin once the camera is this far (local units) from it.
const REBASE_DISTANCE: f32 = 90_000.0;
/// Regions being built at once.
const MAX_IN_FLIGHT: usize = 2;

struct ViewportTargets {
    width: u32,
    height: u32,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    texture_id: egui::TextureId,
}

#[derive(Clone, Copy)]
struct RegionStats {
    vertices: usize,
}

pub struct EditorApp {
    render_state: egui_wgpu::RenderState,
    renderer: SceneRenderer,
    camera: FlyCamera,
    targets: Option<ViewportTargets>,
    view_plane: u8,
    brightness: f32,
    streamer: RegionStreamer,
    ready: bool,
    status: Option<String>,
    /// Regions resident on the GPU (`None` = the cache has no map data there).
    loaded: HashMap<(i32, i32), Option<RegionStats>>,
    in_flight: HashSet<(i32, i32)>,
    stream_radius: i32,
    last_build_ms: f32,
    fps: f32,
    /// Smoke-test hook: exit after this many seconds (`RUSTOSRS_EXIT_AFTER_SECONDS`).
    exit_after_seconds: Option<f32>,
    /// Smoke-test hook: constant camera velocity in local units per second
    /// (`RUSTOSRS_AUTOFLY="vx,vz"`).
    autofly: Option<(f32, f32)>,
    frames: u64,
    peak_resident: usize,
    started: Instant,
}

impl EditorApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        cache_dir: PathBuf,
        base_x: i32,
        base_y: i32,
    ) -> Result<Self, String> {
        let render_state = cc
            .wgpu_render_state
            .clone()
            .ok_or("eframe is not running on the wgpu backend")?;
        let mut renderer =
            SceneRenderer::with_device(render_state.device.clone(), render_state.queue.clone());

        // Start above the middle of the requested window's region, looking north.
        let start_region = (base_x.div_euclid(64) + 1, base_y.div_euclid(64) + 1);
        let origin = (start_region.0 * REGION_UNITS, start_region.1 * REGION_UNITS);
        renderer.set_render_origin(origin);
        let camera = FlyCamera::new(
            0.45 * REGION_UNITS as f32,
            -2600.0,
            0.3 * REGION_UNITS as f32,
        );
        let streamer = RegionStreamer::spawn(cache_dir, TerrainPresentation::default());

        Ok(Self {
            render_state,
            renderer,
            camera,
            targets: None,
            view_plane: 0,
            brightness: 0.8,
            streamer,
            ready: false,
            status: None,
            loaded: HashMap::new(),
            in_flight: HashSet::new(),
            stream_radius: 1,
            last_build_ms: 0.0,
            fps: 0.0,
            exit_after_seconds: std::env::var("RUSTOSRS_EXIT_AFTER_SECONDS")
                .ok()
                .and_then(|value| value.parse().ok()),
            autofly: std::env::var("RUSTOSRS_AUTOFLY").ok().and_then(|value| {
                let (x, z) = value.split_once(',')?;
                Some((x.parse().ok()?, z.parse().ok()?))
            }),
            frames: 0,
            peak_resident: 0,
            started: Instant::now(),
        })
    }

    /// World position of the camera in local units.
    fn world_position(&self) -> (f64, f64) {
        let origin = self.renderer.render_origin();
        (
            f64::from(origin.0) + f64::from(self.camera.x),
            f64::from(origin.1) + f64::from(self.camera.z),
        )
    }

    fn camera_region(&self) -> (i32, i32) {
        let (x, z) = self.world_position();
        (
            (x / f64::from(REGION_UNITS)).floor() as i32,
            (z / f64::from(REGION_UNITS)).floor() as i32,
        )
    }

    /// Keep the render origin near the camera so f32 stays precise far from the map origin.
    fn rebase_if_needed(&mut self) {
        let far = self.camera.x.abs().max(self.camera.z.abs());
        if far < REBASE_DISTANCE {
            return;
        }
        let (world_x, world_z) = self.world_position();
        let new_origin = (
            (world_x / 1024.0).round() as i32 * 1024,
            (world_z / 1024.0).round() as i32 * 1024,
        );
        let old = self.renderer.render_origin();
        self.renderer.set_render_origin(new_origin);
        self.camera.x += (old.0 - new_origin.0) as f32;
        self.camera.z += (old.1 - new_origin.1) as f32;
    }

    /// Apply finished builds, then request and unload regions around the camera.
    fn stream(&mut self) {
        while let Some(event) = self.streamer.try_recv() {
            match event {
                StreamEvent::Ready { textures } => {
                    self.renderer.set_textures(&textures);
                    self.ready = true;
                }
                StreamEvent::Region(region) => {
                    let key = (region.region.x, region.region.y);
                    self.in_flight.remove(&key);
                    self.renderer.upload_region(key, &region.geometry);
                    self.last_build_ms = region.build_ms;
                    self.loaded.insert(
                        key,
                        Some(RegionStats {
                            vertices: region.geometry.vertex_count(),
                        }),
                    );
                }
                StreamEvent::Empty(region) => {
                    let key = (region.x, region.y);
                    self.in_flight.remove(&key);
                    self.loaded.insert(key, None);
                }
                StreamEvent::Failed(region, error) => {
                    self.in_flight.remove(&(region.x, region.y));
                    self.loaded.insert((region.x, region.y), None);
                    self.status = Some(format!("region {},{} failed: {error}", region.x, region.y));
                }
                StreamEvent::InitFailed(error) => {
                    self.status = Some(format!("cache failed to open: {error}"));
                }
            }
        }
        if !self.ready {
            return;
        }

        let (cx, cy) = self.camera_region();
        let radius = self.stream_radius;
        let mut wanted: Vec<(i32, i32)> = Vec::new();
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                wanted.push((cx + dx, cy + dy));
            }
        }
        wanted.sort_by_key(|&(x, y)| {
            (x - cx).abs().max((y - cy).abs()) * 100 + (x - cx).abs() + (y - cy).abs()
        });
        for key in wanted {
            if self.in_flight.len() >= MAX_IN_FLIGHT {
                break;
            }
            if !self.loaded.contains_key(&key) && !self.in_flight.contains(&key) {
                self.in_flight.insert(key);
                self.streamer.request(RegionCoord::new(key.0, key.1));
            }
        }

        let keep = radius + 1;
        let stale: Vec<(i32, i32)> = self
            .loaded
            .keys()
            .copied()
            .filter(|&(x, y)| (x - cx).abs().max((y - cy).abs()) > keep)
            .collect();
        for key in stale {
            self.loaded.remove(&key);
            self.renderer.remove_region(key);
        }
    }

    fn ensure_targets(&mut self, width: u32, height: u32) {
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

    fn handle_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
        if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Primary)
        {
            let delta = response.drag_delta();
            self.camera.look(delta.x, delta.y);
        }
        if response.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.speed =
                    (self.camera.speed * (1.0 + scroll * 0.002)).clamp(50.0, 20_000.0);
            }
        }
        ctx.input(|i| {
            let axis = |plus: egui::Key, minus: egui::Key| {
                f32::from(i.key_down(plus)) - f32::from(i.key_down(minus))
            };
            let boost = if i.modifiers.shift { 4.0 } else { 1.0 };
            let forward = axis(egui::Key::W, egui::Key::S) * boost;
            let right = axis(egui::Key::D, egui::Key::A) * boost;
            let up = axis(egui::Key::E, egui::Key::Q) * boost;
            if forward != 0.0 || right != 0.0 || up != 0.0 {
                self.camera.translate(forward, right, up, dt);
            }
        });
        ctx.input(|i| {
            for (key, plane) in [
                (egui::Key::Num1, 0),
                (egui::Key::Num2, 1),
                (egui::Key::Num3, 2),
                (egui::Key::Num4, 3),
            ] {
                if i.key_pressed(key) {
                    self.view_plane = plane;
                }
            }
        });
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = ctx.input(|i| i.unstable_dt).max(1e-4);
        self.fps = self.fps * 0.9 + (1.0 / dt) * 0.1;
        if let Some((vx, vz)) = self.autofly {
            let step = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
            self.camera.x += vx * step;
            self.camera.z += vz * step;
        }
        self.rebase_if_needed();
        self.stream();
        self.peak_resident = self
            .peak_resident
            .max(self.loaded.values().flatten().count());

        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{:.0} fps", self.fps));
                ui.separator();
                let resident: usize = self.loaded.values().flatten().map(|stats| stats.vertices).sum();
                let loaded = self.loaded.values().flatten().count();
                ui.label(format!(
                    "{loaded} regions ({} loading, last {:.0} ms), {:.1}M vertices, {:.0} MiB GPU, {} zones",
                    self.in_flight.len(),
                    self.last_build_ms,
                    resident as f32 / 1.0e6,
                    self.renderer.resident_vertex_bytes() as f32 / (1024.0 * 1024.0),
                    self.renderer.resident_zone_count()
                ));
                if let Some(status) = &self.status {
                    ui.colored_label(egui::Color32::LIGHT_RED, status);
                }
                ui.separator();
                let (region_x, region_y) = self.camera_region();
                let (world_x, world_z) = self.world_position();
                ui.label(format!(
                    "tile ({:.0}, {:.0}) region ({region_x}, {region_y}) height {:.0}  plane {}  speed {:.0}",
                    world_x / 128.0,
                    world_z / 128.0,
                    -self.camera.y,
                    self.view_plane + 1,
                    self.camera.speed
                ));
                ui.separator();
                ui.add(egui::Slider::new(&mut self.brightness, 0.5..=1.0).text("brightness"));
                ui.add(egui::Slider::new(&mut self.stream_radius, 0..=4).text("radius"));
                ui.add(egui::Slider::new(&mut self.camera.fov_degrees, 30.0..=100.0).text("fov"));
            });
            ui.label(
                "WASD fly, Q/E down/up, Shift fast, mouse-drag look, scroll = speed, 1-4 = plane",
            );
        });

        egui::CentralPanel::default().show(ui, |ui| {
            let available = ui.available_size();
            let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
            let pixels_per_point = ctx.pixels_per_point();
            let width = ((rect.width() * pixels_per_point) as u32).max(16);
            let height = ((rect.height() * pixels_per_point) as u32).max(16);

            self.handle_input(&ctx, &response);

            if self.ready {
                self.ensure_targets(width, height);
                if let Some(targets) = &self.targets {
                    let mut encoder = self.render_state.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor {
                            label: Some("viewport"),
                        },
                    );
                    self.renderer.encode_frame(
                        &mut encoder,
                        &targets.color_view,
                        &targets.depth_view,
                        FrameParams {
                            camera: self.camera.reference(targets.width as f32),
                            view_plane: self.view_plane,
                            // The client's `gameCycle & 127`, at 50 cycles per second.
                            tick: ((self.started.elapsed().as_secs_f32() * 50.0) as u32) & 127,
                            brightness: self.brightness,
                            clear_color: [0.55, 0.7, 0.9],
                        },
                        targets.width,
                        targets.height,
                    );
                    self.render_state.queue.submit(Some(encoder.finish()));
                    let _ = &targets.color;
                    ui.painter().image(
                        targets.texture_id,
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }
        });

        self.frames += 1;
        if let Some(limit) = self.exit_after_seconds
            && self.started.elapsed().as_secs_f32() >= limit
        {
            let seconds = self.started.elapsed().as_secs_f32();
            eprintln!(
                "smoke: {} frames in {:.2}s ({:.1} fps average); {} regions resident (peak {}), {:.0} MiB GPU, {} zones",
                self.frames,
                seconds,
                self.frames as f32 / seconds,
                self.loaded.values().flatten().count(),
                self.peak_resident,
                self.renderer.resident_vertex_bytes() as f32 / (1024.0 * 1024.0),
                self.renderer.resident_zone_count()
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Continuous repaint while the viewport is live.
        ctx.request_repaint();
    }
}
