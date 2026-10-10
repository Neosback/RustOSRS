//! The eframe application: scene loading, the wgpu viewport, and fly-camera input.

use crate::camera::FlyCamera;
use eframe::{egui, egui_wgpu, wgpu};
use osrs_render::{
    SceneGeometry,
    gpu::{COLOR_FORMAT, DEPTH_FORMAT, FrameParams, SceneRenderer},
};
use osrs_world::{
    SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene, extract_scene_geometry,
};
use std::{path::PathBuf, time::Instant};

struct ViewportTargets {
    width: u32,
    height: u32,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    texture_id: egui::TextureId,
}

struct LoadedScene {
    vertex_count: usize,
    zone_count: usize,
    load_time_ms: f32,
}

pub struct EditorApp {
    render_state: egui_wgpu::RenderState,
    renderer: SceneRenderer,
    camera: FlyCamera,
    targets: Option<ViewportTargets>,
    view_plane: u8,
    brightness: f32,
    scene: Result<LoadedScene, String>,
    fps: f32,
    /// Smoke-test hook: exit after this many frames (`RUSTOSRS_EXIT_AFTER_FRAMES`).
    exit_after_frames: Option<u64>,
    frames: u64,
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

        let scene = load_scene(&cache_dir, base_x, base_y, &mut renderer);
        // Start above the middle of the window, looking north.
        let camera = FlyCamera::new(
            (base_x.rem_euclid(8) as f32 + 52.0) * 128.0,
            -2600.0,
            24.0 * 128.0,
        );
        Ok(Self {
            render_state,
            renderer,
            camera,
            targets: None,
            view_plane: 0,
            brightness: 0.8,
            scene,
            fps: 0.0,
            exit_after_frames: std::env::var("RUSTOSRS_EXIT_AFTER_FRAMES")
                .ok()
                .and_then(|value| value.parse().ok()),
            frames: 0,
            started: Instant::now(),
        })
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

fn load_scene(
    cache_dir: &PathBuf,
    base_x: i32,
    base_y: i32,
    renderer: &mut SceneRenderer,
) -> Result<LoadedScene, String> {
    let started = Instant::now();
    let mut definitions = WorldDefinitions::open(cache_dir).map_err(|error| error.to_string())?;
    let window = SceneWindow::new(base_x, base_y)
        .ok_or_else(|| "scene base must be a multiple of 8".to_owned())?;
    let world = build_world_scene(&mut definitions, window, TerrainPresentation::default())
        .map_err(|error| error.to_string())?;
    let geometry: SceneGeometry = extract_scene_geometry(&world, definitions.floors());
    renderer.upload_scene(&geometry);
    Ok(LoadedScene {
        vertex_count: geometry.vertex_count(),
        zone_count: geometry.zones.len(),
        load_time_ms: started.elapsed().as_secs_f32() * 1000.0,
    })
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = ctx.input(|i| i.unstable_dt).max(1e-4);
        self.fps = self.fps * 0.9 + (1.0 / dt) * 0.1;

        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{:.0} fps", self.fps));
                ui.separator();
                match &self.scene {
                    Ok(scene) => {
                        ui.label(format!(
                            "{} zones, {:.1}M vertices ({:.1} MiB GPU), loaded in {:.0} ms",
                            scene.zone_count,
                            scene.vertex_count as f32 / 1.0e6,
                            self.renderer.resident_vertex_bytes() as f32 / (1024.0 * 1024.0),
                            scene.load_time_ms
                        ));
                    }
                    Err(error) => {
                        ui.colored_label(egui::Color32::LIGHT_RED, format!("load failed: {error}"));
                    }
                }
                ui.separator();
                ui.label(format!(
                    "pos ({:.0}, {:.0}, {:.0})  plane {}  speed {:.0}",
                    self.camera.x,
                    self.camera.y,
                    self.camera.z,
                    self.view_plane + 1,
                    self.camera.speed
                ));
                ui.separator();
                ui.add(egui::Slider::new(&mut self.brightness, 0.5..=1.0).text("brightness"));
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

            if self.scene.is_ok() {
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
                            camera: self.camera.reference(),
                            view_plane: self.view_plane,
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
        if let Some(limit) = self.exit_after_frames
            && self.frames >= limit
        {
            let seconds = self.started.elapsed().as_secs_f32();
            eprintln!(
                "smoke: {} frames in {:.2}s ({:.1} fps average)",
                self.frames,
                seconds,
                self.frames as f32 / seconds
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Continuous repaint while the viewport is live.
        ctx.request_repaint();
    }
}
