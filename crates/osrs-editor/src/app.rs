//! The eframe application: scene loading, the wgpu viewport, and fly-camera input.

mod menu;
mod overlay;
mod properties;
mod status;

use crate::camera::FlyCamera;
use crate::grid::{self, GridSettings};
use crate::hud::{Destination, FrameStats, TeleportWindow};
use crate::pick::{self, Infos, ObjectPick, Pick};
use crate::settings::{self, Settings};
use eframe::{egui, egui_wgpu, wgpu};
use osrs_core::coords::RegionCoord;
use osrs_render::gpu::{
    COLOR_FORMAT, Colorblind, ColorblindMode, DEPTH_FORMAT, Fog, FrameParams, SceneRenderer,
};
use osrs_world::{AnimationWorker, RegionStreamer, StreamEvent, TerrainPresentation};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
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

/// Which editor panels are shown (the layout state of the window chrome).
struct Panels {
    /// Right dock with the Selection / View / Shading / Scene sections.
    properties: bool,
    /// Translucent fps and position readout over the viewport.
    stats_overlay: bool,
    help: bool,
}

/// What the grid overlay geometry depends on.
#[derive(Clone, Copy, PartialEq, Eq)]
struct GridKey {
    settings: GridSettings,
    center: (i32, i32),
    plane: u8,
    origin: (i32, i32),
    infos_version: u64,
}

/// `(region, loc index)` of a picked object.
type OutlineKey = ((i32, i32), usize);

/// Renderer key of the per-frame animated-loc geometry.
const ANIMATED_KEY: (i32, i32) = (i32::MIN, i32::MIN);
/// Animated locs farther than this many tiles from the camera are not drawn.
const ANIMATION_RADIUS_TILES: i32 = 80;
/// Animated geometry is rebuilt at most once per this many game cycles (20 ms each).
const ANIMATION_STRIDE_CYCLES: u64 = 2;

pub struct EditorApp {
    render_state: egui_wgpu::RenderState,
    renderer: SceneRenderer,
    camera: FlyCamera,
    targets: Option<ViewportTargets>,
    view_plane: u8,
    brightness: f32,
    remove_color_banding: bool,
    bright_textures: bool,
    fog: Fog,
    colorblind: Colorblind,
    panels: Panels,
    grid: GridSettings,
    /// Where settings persist (`None` disables saving).
    settings_path: Option<PathBuf>,
    saved_settings: Settings,
    settings_checked: Instant,
    /// Inputs the grid lines were last built for (see `update_grid`).
    grid_key: Option<GridKey>,
    /// Bumped whenever `infos` changes.
    infos_version: u64,
    smooth_terrain: bool,
    presentation_generation: u32,
    /// Background (and the colour holes in the terrain show). RuneLite's default sky is black.
    sky_color: [f32; 3],
    theme_index: usize,
    /// Inspection data of the resident regions (for picking).
    infos: Infos,
    hover: Pick,
    selected: Option<ObjectPick>,
    /// Model triangles of picked objects, keyed by (region, loc index).
    outlines: HashMap<OutlineKey, Arc<Vec<[[i32; 3]; 3]>>>,
    outline_requested: HashSet<OutlineKey>,
    context_pick: Pick,
    streamer: RegionStreamer,
    animations: AnimationWorker,
    animation_cycle: u64,
    animation_center: (i32, i32),
    animation_dirty: bool,
    ready: bool,
    status: Option<String>,
    /// Regions resident on the GPU (`None` = the cache has no map data there).
    loaded: HashMap<(i32, i32), Option<RegionStats>>,
    in_flight: HashSet<(i32, i32)>,
    stream_radius: i32,
    last_build_ms: f32,
    stats: FrameStats,
    teleport: TeleportWindow,
    /// Smoke-test hook: teleport on the first frame (`RUSTOSRS_TELEPORT="x,y,plane"`).
    pending_teleport: Option<Destination>,
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
        crate::theme::set_theme(&cc.egui_ctx, crate::theme::MOCHA);
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
        let streamer = RegionStreamer::spawn(cache_dir, Self::presentation(false));
        streamer.set_presentation(Self::presentation(false), 1);

        let mut app = Self {
            render_state,
            renderer,
            camera,
            targets: None,
            view_plane: 0,
            brightness: 0.8,
            remove_color_banding: true,
            bright_textures: false,
            fog: Fog::default(),
            colorblind: Colorblind {
                mode: ColorblindMode::None,
                intensity: 100.0,
            },
            settings_path: None,
            saved_settings: Settings::default(),
            settings_checked: Instant::now(),
            grid: std::env::var("RUSTOSRS_GRID").map_or_else(
                |_| GridSettings::default(),
                |value| GridSettings {
                    tiles: value.contains("tiles"),
                    zones: value.contains("zones"),
                    regions: value.contains("regions"),
                },
            ),
            grid_key: None,
            infos_version: 0,
            panels: Panels {
                properties: true,
                stats_overlay: true,
                help: false,
            },
            smooth_terrain: false,
            presentation_generation: 1,
            sky_color: [0.0, 0.0, 0.0],
            theme_index: 3,
            infos: Infos::new(),
            hover: Pick::default(),
            selected: None,
            outlines: HashMap::new(),
            outline_requested: HashSet::new(),
            context_pick: Pick::default(),
            streamer,
            animations: AnimationWorker::spawn(),
            animation_cycle: 0,
            animation_center: (0, 0),
            animation_dirty: true,
            ready: false,
            status: None,
            loaded: HashMap::new(),
            in_flight: HashSet::new(),
            stream_radius: 1,
            last_build_ms: 0.0,
            stats: FrameStats::new(),
            teleport: TeleportWindow::new(),
            pending_teleport: std::env::var("RUSTOSRS_TELEPORT").ok().and_then(|value| {
                let mut parts = value.split(',').map(|part| part.trim().parse::<i32>().ok());
                Some(Destination::Tile {
                    x: parts.next()??,
                    y: parts.next()??,
                    plane: u8::try_from(parts.next()??).ok()?,
                })
            }),
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
        };
        app.init_settings(&cc.egui_ctx);
        Ok(app)
    }

    /// Load persisted settings (skipped in smoke runs unless `RUSTOSRS_SETTINGS` is set).
    fn init_settings(&mut self, ctx: &egui::Context) {
        let smoke =
            self.exit_after_seconds.is_some() && std::env::var("RUSTOSRS_SETTINGS").is_err();
        if smoke {
            return;
        }
        self.settings_path = settings::path();
        let Some(path) = self.settings_path.clone() else {
            return;
        };
        let loaded = Settings::load(&path);
        let grid_override = std::env::var("RUSTOSRS_GRID").is_ok().then_some(self.grid);
        self.apply_settings(&loaded, ctx);
        if let Some(grid) = grid_override {
            self.grid = grid;
        }
        self.saved_settings = self.settings_snapshot();
    }

    fn settings_snapshot(&self) -> Settings {
        Settings {
            theme: self.theme_index,
            properties_panel: self.panels.properties,
            stats_overlay: self.panels.stats_overlay,
            brightness: self.brightness,
            remove_color_banding: self.remove_color_banding,
            bright_textures: self.bright_textures,
            fog_depth_tiles: self.fog.depth_tiles,
            draw_distance_tiles: self.fog.draw_distance_tiles,
            colorblind_mode: self.colorblind.mode as u8,
            colorblind_intensity: self.colorblind.intensity,
            sky_color: self.sky_color,
            smooth_terrain: self.smooth_terrain,
            grid_tiles: self.grid.tiles,
            grid_zones: self.grid.zones,
            grid_regions: self.grid.regions,
            load_radius: self.stream_radius,
            fov_degrees: self.camera.fov_degrees,
            fly_speed: self.camera.speed,
            plugins: self.saved_settings.plugins.clone(),
        }
    }

    fn apply_settings(&mut self, settings: &Settings, ctx: &egui::Context) {
        self.theme_index = settings.theme.min(crate::theme::ALL.len() - 1);
        crate::theme::set_theme(ctx, crate::theme::ALL[self.theme_index]);
        self.panels.properties = settings.properties_panel;
        self.panels.stats_overlay = settings.stats_overlay;
        self.brightness = settings.brightness.clamp(0.5, 1.0);
        self.remove_color_banding = settings.remove_color_banding;
        self.bright_textures = settings.bright_textures;
        self.fog.depth_tiles = settings.fog_depth_tiles.min(100);
        self.fog.draw_distance_tiles = settings.draw_distance_tiles.clamp(8, 184);
        self.colorblind.mode = match settings.colorblind_mode {
            1 => ColorblindMode::Protanope,
            2 => ColorblindMode::Deuteranope,
            3 => ColorblindMode::Tritanope,
            _ => ColorblindMode::None,
        };
        self.colorblind.intensity = settings.colorblind_intensity.clamp(0.0, 100.0);
        self.sky_color = settings.sky_color;
        self.grid = GridSettings {
            tiles: settings.grid_tiles,
            zones: settings.grid_zones,
            regions: settings.grid_regions,
        };
        self.stream_radius = settings.load_radius.clamp(0, 4);
        self.camera.fov_degrees = settings.fov_degrees.clamp(30.0, 100.0);
        self.camera.speed = settings.fly_speed.clamp(50.0, 20_000.0);
        if settings.smooth_terrain != self.smooth_terrain {
            self.smooth_terrain = settings.smooth_terrain;
            self.streamer.set_presentation(
                Self::presentation(self.smooth_terrain),
                self.presentation_generation,
            );
        }
        self.saved_settings.plugins = settings.plugins.clone();
    }

    /// Save the settings when they changed (checked once a second).
    fn autosave_settings(&mut self) {
        let Some(path) = self.settings_path.clone() else {
            return;
        };
        if self.settings_checked.elapsed().as_secs_f32() < 1.0 {
            return;
        }
        self.settings_checked = Instant::now();
        let snapshot = self.settings_snapshot();
        if snapshot != self.saved_settings && snapshot.save(&path).is_ok() {
            self.saved_settings = snapshot;
        }
    }

    fn presentation(smooth_terrain: bool) -> TerrainPresentation {
        TerrainPresentation {
            smooth_terrain,
            ..TerrainPresentation::default()
        }
    }

    /// Rebuild every resident region (after a presentation change).
    fn rebuild_all(&mut self) {
        self.presentation_generation += 1;
        self.streamer.set_presentation(
            Self::presentation(self.smooth_terrain),
            self.presentation_generation,
        );
        for key in self.loaded.keys().copied().collect::<Vec<_>>() {
            self.renderer.remove_region(key);
            self.animations.remove_region(key);
            self.infos.remove(&key);
            self.infos_version += 1;
        }
        self.loaded.clear();
        self.outlines.clear();
        self.outline_requested.clear();
        self.selected = None;
        self.in_flight.clear();
        self.animation_dirty = true;
    }

    /// World position of the camera in local units.
    fn world_position(&self) -> (f64, f64) {
        let origin = self.renderer.render_origin();
        (
            f64::from(origin.0) + f64::from(self.camera.x),
            f64::from(origin.1) + f64::from(self.camera.z),
        )
    }

    /// World tile under the camera.
    fn camera_tile(&self) -> (i32, i32) {
        let (x, z) = self.world_position();
        ((x / 128.0).floor() as i32, (z / 128.0).floor() as i32)
    }

    fn open_teleport(&mut self) {
        let tile = self.camera_tile();
        self.teleport.prefill(tile, self.view_plane);
        self.teleport.open = true;
    }

    /// Move the camera above `destination` and rebase the render origin onto it.
    fn teleport_to(&mut self, destination: Destination) {
        let (tile_x, tile_y, plane) = match destination {
            Destination::Tile { x, y, plane } => (x, y, plane),
            Destination::Region { x, y } => (x * 64 + 32, y * 64 + 32, self.view_plane),
        };
        let world_x = tile_x * 128 + 64;
        let world_z = tile_y * 128 + 64;
        let origin = (
            (world_x as f32 / 1024.0).round() as i32 * 1024,
            (world_z as f32 / 1024.0).round() as i32 * 1024,
        );
        self.renderer.set_render_origin(origin);
        self.camera.x = (world_x - origin.0) as f32;
        self.camera.z = (world_z - origin.1) as f32;
        self.camera.y = -(1800.0 + 240.0 * f32::from(plane));
        self.view_plane = plane;
        self.animation_dirty = true;
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
                    if region.generation != self.presentation_generation {
                        continue;
                    }
                    self.in_flight.remove(&key);
                    self.renderer.upload_region(key, &region.geometry);
                    self.animations.insert_region(key, region.animations);
                    self.infos.insert(key, region.info);
                    self.infos_version += 1;
                    self.animation_dirty = true;
                    self.last_build_ms = region.build_ms;
                    self.loaded.insert(
                        key,
                        Some(RegionStats {
                            vertices: region.geometry.vertex_count(),
                        }),
                    );
                }
                StreamEvent::Outline {
                    region,
                    index,
                    triangles,
                } => {
                    self.outlines.insert((region, index), Arc::new(triangles));
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
            self.animations.remove_region(key);
            self.infos.remove(&key);
            self.infos_version += 1;
            self.animation_dirty = true;
        }
    }

    /// Rebuild the grid overlay when what it depends on changed.
    fn update_grid(&mut self) {
        let key = GridKey {
            settings: self.grid,
            center: self.camera_tile(),
            plane: self.view_plane,
            origin: self.renderer.render_origin(),
            infos_version: self.infos_version,
        };
        if self.grid_key == Some(key) {
            return;
        }
        self.grid_key = Some(key);
        let lines = if self.grid.any() {
            grid::build(
                self.grid,
                &self.infos,
                key.center,
                usize::from(key.plane),
                key.origin,
            )
        } else {
            Vec::new()
        };
        self.renderer.set_overlay_lines(&lines);
    }

    /// Advance animated locs to the current game cycle on the animation worker and upload the
    /// geometry it finishes.
    fn update_animations(&mut self) {
        if let Some(update) = self.animations.poll()
            && let Some(geometry) = update.geometry
        {
            let phase = Instant::now();
            self.renderer.upload_region(ANIMATED_KEY, &geometry);
            self.stats.phase("anim-upload", phase);
        }
        if !self.animations.is_idle() {
            return;
        }
        let cycle = (self.started.elapsed().as_secs_f32() * 50.0) as u64;
        let (world_x, world_z) = self.world_position();
        let center = ((world_x / 128.0) as i32, (world_z / 128.0) as i32);
        let moved = (center.0 - self.animation_center.0)
            .abs()
            .max((center.1 - self.animation_center.1).abs())
            >= 8;
        if cycle < self.animation_cycle + ANIMATION_STRIDE_CYCLES && !self.animation_dirty && !moved
        {
            return;
        }
        let delta = (cycle - self.animation_cycle).min(500) as i32;
        self.animation_cycle = cycle;
        if self.animation_dirty || moved {
            self.animation_dirty = false;
            self.animation_center = center;
            self.animations.force_rebuild();
        }
        self.animations
            .request(delta, self.animation_center, ANIMATION_RADIUS_TILES);
    }

    /// Hover / click / right-click picking against the resident regions' inspection data.
    fn update_picking(
        &mut self,
        response: &egui::Response,
        rect: egui::Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) {
        if response.dragged() {
            return;
        }
        let Some(position) = response.hover_pos() else {
            self.hover = Pick::default();
            return;
        };
        let ray = pick::ray_through_pixel(
            &self.camera,
            self.renderer.render_origin(),
            width as f32,
            height as f32,
            (position.x - rect.min.x) * pixels_per_point,
            (position.y - rect.min.y) * pixels_per_point,
        );
        self.hover = pick::pick(&self.infos, &ray, self.view_plane);
        if response.clicked() {
            self.selected = self.hover.object.clone();
        }
        if response.secondary_clicked() {
            self.context_pick = self.hover.clone();
        }
        let wanted: Vec<ObjectPick> = [
            self.hover.object.clone(),
            self.selected.clone(),
            self.context_pick.object.clone(),
        ]
        .into_iter()
        .flatten()
        .collect();
        for object in wanted {
            let key = (object.region, object.index);
            if self.outlines.contains_key(&key) || !self.outline_requested.insert(key) {
                continue;
            }
            if let Some(loc) = self
                .infos
                .get(&object.region)
                .and_then(|info| info.locs.get(object.index))
            {
                self.streamer
                    .request_outline(object.region, object.index, loc.clone());
            }
        }
    }

    fn draw_selection(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) {
        let painter = ui.painter().with_clip_rect(rect);
        let outline = |object: &ObjectPick, fill: egui::Color32, edge: egui::Color32| {
            let Some(triangles) = self.outlines.get(&(object.region, object.index)) else {
                return;
            };
            let to_screen = |point: [i32; 3]| {
                pick::project(
                    &self.camera,
                    self.renderer.render_origin(),
                    width as f32,
                    height as f32,
                    [
                        f64::from(point[0]),
                        f64::from(point[1]),
                        f64::from(point[2]),
                    ],
                )
                .map(|p| {
                    egui::pos2(
                        rect.min.x + p[0] / pixels_per_point,
                        rect.min.y + p[1] / pixels_per_point,
                    )
                })
            };
            let mut mesh = egui::epaint::Mesh::default();
            for triangle in triangles.iter() {
                let (Some(a), Some(b), Some(c)) = (
                    to_screen(triangle[0]),
                    to_screen(triangle[1]),
                    to_screen(triangle[2]),
                ) else {
                    continue;
                };
                let base = mesh.vertices.len() as u32;
                for position in [a, b, c] {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: position,
                        uv: egui::epaint::WHITE_UV,
                        color: fill,
                    });
                }
                mesh.indices.extend([base, base + 1, base + 2]);
                painter.line_segment([a, b], egui::Stroke::new(0.75, edge));
                painter.line_segment([b, c], egui::Stroke::new(0.75, edge));
                painter.line_segment([c, a], egui::Stroke::new(0.75, edge));
            }
            painter.add(egui::Shape::mesh(mesh));
        };
        if let Some(object) = &self.hover.object {
            outline(
                object,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40),
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 90),
            );
        }
        if let Some(object) = &self.selected {
            outline(
                object,
                egui::Color32::from_rgba_unmultiplied(255, 190, 0, 60),
                egui::Color32::from_rgb(255, 200, 0),
            );
        }
        let text = pick::summary(&self.infos, &self.hover);
        if !text.is_empty() {
            painter.text(
                rect.left_bottom() + egui::vec2(8.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                text,
                egui::FontId::monospace(13.0),
                egui::Color32::WHITE,
            );
        }
    }

    /// Right-click menu: copy diagnostics of the clicked object / tile / surroundings.
    fn context_menu(&self, response: &egui::Response) {
        response.context_menu(|ui| self.copy_menu(ui, &self.context_pick));
    }

    /// "Copy ..." actions for an object and/or tile (context menu and the Selection section).
    fn copy_menu(&self, ui: &mut egui::Ui, pick: &Pick) {
        let loc = pick
            .object
            .as_ref()
            .and_then(|object| self.infos.get(&object.region)?.locs.get(object.index));
        let center = loc.map(|loc| loc.tile).or(pick.tile.map(|tile| tile.tile));
        let plane = loc.map_or(usize::from(self.view_plane), |loc| usize::from(loc.plane));
        let copy = |ui: &mut egui::Ui, label: &str, text: String| {
            if ui.button(label).clicked() {
                ui.ctx().copy_text(text);
                ui.close();
            }
        };
        if let Some(object) = &pick.object {
            copy(
                ui,
                "Copy object",
                pick::describe_object(&self.infos, object),
            );
        }
        if let Some(tile) = center {
            copy(ui, "Copy tile", pick::describe_tile(&self.infos, tile));
            copy(
                ui,
                "Copy neighbourhood (3x3)",
                pick::describe_neighbourhood(&self.infos, tile, plane, 1),
            );
            copy(
                ui,
                "Copy neighbourhood (5x5)",
                pick::describe_neighbourhood(&self.infos, tile, plane, 2),
            );
            if let Some(object) = &pick.object {
                let mut text = pick::describe_object(&self.infos, object);
                text.push_str(&pick::describe_neighbourhood(&self.infos, tile, plane, 1));
                copy(ui, "Copy object + neighbourhood", text);
            }
        } else {
            ui.label("nothing under the cursor");
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
        if ctx.egui_wants_keyboard_input() {
            return;
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
    fn on_exit(&mut self) {
        if let Some(path) = &self.settings_path {
            let _ = self.settings_snapshot().save(path);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let frame_started = self.stats.begin_frame();
        if let Some(destination) = self.pending_teleport.take() {
            self.teleport_to(destination);
        }
        if !ctx.egui_wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::T)) {
                self.open_teleport();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::N)) {
                self.panels.properties = !self.panels.properties;
            }
        }
        if let Some((vx, vz)) = self.autofly {
            let step = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
            self.camera.x += vx * step;
            self.camera.z += vz * step;
        }
        self.rebase_if_needed();
        let phase = Instant::now();
        self.stream();
        self.stats.phase("stream", phase);
        let phase = Instant::now();
        self.update_animations();
        self.stats.phase("anim", phase);
        let phase = Instant::now();
        self.update_grid();
        self.stats.phase("grid", phase);
        self.peak_resident = self
            .peak_resident
            .max(self.loaded.values().flatten().count());

        self.menu_bar(ui);
        self.status_bar(ui);
        self.properties_panel(ui);

        egui::CentralPanel::default().show(ui, |ui| {
            let available = ui.available_size();
            let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
            let pixels_per_point = ctx.pixels_per_point();
            let width = ((rect.width() * pixels_per_point) as u32).max(16);
            let height = ((rect.height() * pixels_per_point) as u32).max(16);

            self.handle_input(&ctx, &response);
            let phase = Instant::now();
            self.update_picking(&response, rect, pixels_per_point, width, height);
            self.stats.phase("pick", phase);

            if self.ready {
                self.ensure_targets(width, height);
                if let Some(targets) = &self.targets {
                    let mut encoder = self.render_state.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor {
                            label: Some("viewport"),
                        },
                    );
                    let phase = Instant::now();
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
                            remove_color_banding: self.remove_color_banding,
                            bright_textures: self.bright_textures,
                            fog: self.fog,
                            colorblind: self.colorblind,
                            clear_color: self.sky_color.map(f64::from),
                        },
                        targets.width,
                        targets.height,
                    );
                    self.stats.phase("encode", phase);
                    self.render_state.queue.submit(Some(encoder.finish()));
                    self.renderer.finish_frame();
                    self.stats.set_gpu_scene_ms(self.renderer.gpu_scene_ms());
                    self.render_state
                        .queue
                        .on_submitted_work_done(self.stats.gpu_probe());
                    let _ = &targets.color;
                    ui.painter().image(
                        targets.texture_id,
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }
            self.draw_selection(ui, rect, pixels_per_point, width, height);
            self.viewport_overlays(ui, rect);
            self.context_menu(&response);
        });

        self.help_window(&ctx);
        if let Some(destination) = self.teleport.show(&ctx) {
            self.teleport_to(destination);
        }
        self.autosave_settings();
        self.stats.end_frame(frame_started);
        self.frames += 1;
        if let Some(limit) = self.exit_after_seconds
            && self.started.elapsed().as_secs_f32() >= limit
        {
            let seconds = self.started.elapsed().as_secs_f32();
            let timings = format!(
                "{} | {}",
                self.stats.performance_text(),
                self.stats.phases_text()
            );
            eprintln!(
                "smoke: {} frames in {:.2}s ({:.1} fps average); {} regions resident (peak {}), {:.0} MiB GPU, {} zones\nsmoke: tile {:?} region {:?}\nsmoke: {} | {}",
                self.frames,
                seconds,
                self.frames as f32 / seconds,
                self.loaded.values().flatten().count(),
                self.peak_resident,
                self.renderer.resident_vertex_bytes() as f32 / (1024.0 * 1024.0),
                self.renderer.resident_zone_count(),
                self.camera_tile(),
                self.camera_region(),
                timings,
                self.stats
                    .memory_text(self.renderer.resident_vertex_bytes())
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Continuous repaint while the viewport is live.
        ctx.request_repaint();
    }
}
