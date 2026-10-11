//! The eframe application: scene loading, the wgpu viewport, and fly-camera input.

mod cache_picker;
mod context_menu;
mod drawer;
mod editing;
mod floating;
mod hd;
mod menu;
mod minimap;
mod overlay;
mod plugins;
mod properties;
mod selection;
mod settings_window;
mod shelf;
mod spline_tool;
mod status;
mod tool_strip;
mod toolbar;
mod widgets;
mod workspace;
mod worldmap;

use crate::animation_smoothing::AnimationSmoothingPlugin;
use crate::assets::Assets;
use crate::camera::{CameraMode, FlyCamera};
use crate::grid::GridPlugin;
use crate::hud::{Destination, FrameStats, TeleportWindow};
use crate::interact;
use crate::keymap::Keymap;
use crate::pick::{self, Infos, ObjectPick, Pick};
use crate::plugin::EditorPlugin;
use crate::settings::ProfileChoice;
use crate::settings::{self, Settings};
use eframe::{egui, egui_wgpu, wgpu};
use osrs_core::coords::RegionCoord;
use osrs_render::gpu::{COLOR_FORMAT, DEPTH_FORMAT, FrameParams, SceneRenderer};
use osrs_world::{AnimationWorker, RegionStreamer, StreamEvent, TerrainPresentation};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use web_time::Instant;

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
    /// Optional tools and overlays (see `plugin.rs`).
    plugins: Vec<Box<dyn EditorPlugin>>,
    /// Guide lines per plugin, merged into the renderer's line overlay.
    plugin_lines: std::collections::BTreeMap<&'static str, Vec<osrs_render::gpu::LineVertex>>,
    /// Where settings persist (`None` disables saving).
    settings_path: Option<PathBuf>,
    saved_settings: Settings,
    settings_checked: Instant,
    /// Bumped whenever `infos` changes.
    infos_version: u64,
    assets: Assets,
    world_map: worldmap::WorldMapView,
    minimap_hud: minimap::MinimapHud,
    /// Every persisted value (see `settings.rs`); the UI edits it in place.
    prefs: Settings,
    presentation_generation: u32,
    /// Whether regions are being built with the HD vertex stream (the HD render profile).
    presentation_hd: bool,
    /// The HD profile's area environments and their transition state.
    hd: hd::HdRuntime,
    /// Background (and the colour holes in the terrain show). RuneLite's default sky is black.
    /// Inspection data of the resident regions (for picking).
    infos: Infos,
    hover: Pick,
    selected: Option<ObjectPick>,
    /// Every selected object (the last added is `selected`) and every selected tile.
    selection_objects: Vec<ObjectPick>,
    selection_tiles: HashSet<(i32, i32)>,
    /// The box being dragged with the Select tool.
    marquee: Option<selection::Marquee>,
    /// The spline tool's control points and preview.
    spline: spline_tool::SplineState,
    /// A held tool key: the tool to return to on release, the key and when it went down.
    peek: Option<(crate::paint::Tool, egui::Key, Instant)>,
    /// Floor textures from the world worker, and their egui copies for the palettes.
    floor_textures: Option<Vec<Option<osrs_render::gpu::TextureLayer>>>,
    texture_handles: HashMap<u32, (egui::TextureHandle, osrs_render::TextureAnimationVector)>,
    palette_search: String,
    /// Tiles whose height the editor has edited (their stored height byte is sent with edits).
    height_edited: HashSet<editing::TileKey>,
    stroke_counter: u64,
    /// Model triangles of picked objects, keyed by (region, loc index).
    outlines: HashMap<OutlineKey, Arc<Vec<[[i32; 3]; 3]>>>,
    outline_requested: HashSet<OutlineKey>,
    context_pick: Pick,
    /// Where the viewport context menu (Shift + right click) is open.
    context_menu: Option<egui::Pos2>,
    keymap: Keymap,
    /// Keyboard Shortcuts window state: open, and the action waiting for a key press.
    /// The settings window: open, its search text, and whether to focus the search box.
    settings_open: bool,
    settings_search: String,
    settings_focus_search: bool,
    /// The keyboard shortcut waiting for a key press (settings window, Controls page).
    capturing_binding: Option<&'static str>,
    streamer: RegionStreamer,
    animations: AnimationWorker,
    /// Whether animation frames are blended (changes the update stride).
    animation_smoothing: bool,
    /// A short message shown in the status bar (menu choices that need a client).
    notice: Option<(String, Instant)>,
    /// Whether the worker currently holds animated objects at their rest pose.
    animations_frozen: bool,
    animation_cycle: u64,
    animation_center: (i32, i32),
    animation_dirty: bool,
    ready: bool,
    status: Option<String>,
    /// Regions resident on the GPU (`None` = the cache has no map data there).
    loaded: HashMap<(i32, i32), Option<RegionStats>>,
    in_flight: HashSet<(i32, i32)>,
    last_build_ms: f32,
    stats: FrameStats,
    teleport: TeleportWindow,
    /// Floor colours for the drawer's palettes, loaded on a worker thread.
    palette: Option<std::sync::Arc<osrs_world::FloorPalette>>,
    palette_rx:
        Option<std::sync::mpsc::Receiver<Result<osrs_world::FloorPalette, osrs_world::WorldError>>>,
    /// Browser: the palette is loaded in the frame loop once the cache is open.
    #[cfg(target_arch = "wasm32")]
    palette_job: Option<(
        std::sync::mpsc::Sender<Result<osrs_world::FloorPalette, osrs_world::WorldError>>,
        osrs_cache::source::CacheSource,
    )>,
    /// Browser: the cache folder being read.
    #[cfg(target_arch = "wasm32")]
    web_pick: Option<crate::platform::web::SharedPick>,
    /// Tiles the editor has painted, by `(plane, x, y)`, and the undo / redo stacks of strokes.
    edited: HashMap<editing::TileKey, crate::paint::TileValues>,
    history: Vec<editing::EditStep>,
    redo_stack: Vec<editing::EditStep>,
    stroke: Option<editing::Stroke>,
    /// Regions waiting for a rebuild after an edit.
    dirty_regions: HashSet<(i32, i32)>,
    /// The cache folder in use, the build and profile it matched, and why it could not be opened.
    cache_dir: PathBuf,
    cache_build: Option<(u32, String)>,
    cache_error: Option<String>,
    /// Where the quick tools HUD (Space) is open.
    quick_open: Option<egui::Pos2>,
    /// The last tile the cursor was over (the Tile tab keeps showing it after the cursor leaves).
    last_hover_tile: Option<(i32, i32)>,
    /// Smoke-test hook: teleport on the first frame (`RUSTOSRS_TELEPORT="x,y,plane"`).
    pending_teleport: Option<Destination>,
    /// Smoke-test hook: undo the last stroke once this frame is reached (`RUSTOSRS_UNDO_AT`).
    smoke_undo_at: Option<u64>,
    /// Smoke-test hooks: control points of a spline (`RUSTOSRS_SPLINE="x,y;x,y;..."`) and a box of
    /// selected tiles (`RUSTOSRS_SELECT_TILES="x0,y0,x1,y1"`).
    smoke_spline: Option<Vec<(f32, f32)>>,
    /// Smoke-test hook: lay the spline once this frame is reached (`RUSTOSRS_LAY_AT`).
    smoke_lay_at: Option<u64>,
    smoke_tiles: Option<(i32, i32, i32, i32)>,
    /// Smoke-test hook: paint the brush here once the region is loaded (`RUSTOSRS_PAINT="x,y"`).
    smoke_paint: Option<(i32, i32)>,
    /// Smoke-test hook: select the object anchored on this tile (`RUSTOSRS_SELECT="x,y"`).
    smoke_select: Option<(i32, i32)>,
    /// Smoke-test hook: exit after this many seconds (`RUSTOSRS_EXIT_AFTER_SECONDS`).
    exit_after_seconds: Option<f32>,
    screenshot: Option<crate::screenshot::ScreenshotHook>,
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
        crate::fonts::install(&cc.egui_ctx);
        crate::theme::set_theme(&cc.egui_ctx, crate::theme::ALL[0]);
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
        let initial_dir = cache_dir.clone();
        let initial_source = osrs_cache::source::CacheSource::Dir(cache_dir);
        let streamer =
            RegionStreamer::spawn(initial_source.clone(), Self::presentation(false, false));
        streamer.set_presentation(Self::presentation(false, false), 1);

        let mut app = Self {
            render_state,
            renderer,
            camera,
            targets: None,
            view_plane: 0,
            settings_path: None,
            saved_settings: Settings::default(),
            settings_checked: Instant::now(),
            plugins: vec![
                Box::new(GridPlugin::new()),
                Box::new(AnimationSmoothingPlugin::new()),
                Box::new(crate::wiki::WikiPlugin::new()),
            ],
            plugin_lines: std::collections::BTreeMap::new(),
            infos_version: 0,
            assets: Assets::new(initial_source.clone()),
            world_map: worldmap::WorldMapView::new(),
            minimap_hud: minimap::MinimapHud::default(),
            prefs: Settings::default(),
            presentation_generation: 1,
            presentation_hd: false,
            hd: hd::HdRuntime::new(),
            infos: Infos::new(),
            hover: Pick::default(),
            selected: None,
            selection_objects: Vec::new(),
            selection_tiles: HashSet::new(),
            marquee: None,
            spline: spline_tool::SplineState::default(),
            peek: None,
            floor_textures: None,
            texture_handles: HashMap::new(),
            palette_search: String::new(),
            height_edited: HashSet::new(),
            stroke_counter: 0,
            outlines: HashMap::new(),
            outline_requested: HashSet::new(),
            context_pick: Pick::default(),
            context_menu: None,
            keymap: Keymap::new(),
            // Smoke-test hook: open the settings window at start (`RUSTOSRS_OPEN_SETTINGS=1`).
            settings_open: std::env::var_os("RUSTOSRS_OPEN_SETTINGS").is_some(),
            settings_search: String::new(),
            settings_focus_search: false,
            capturing_binding: None,
            streamer,
            animations: AnimationWorker::spawn(),
            animation_smoothing: false,
            notice: None,
            animations_frozen: false,
            animation_cycle: 0,
            animation_center: (0, 0),
            animation_dirty: true,
            ready: false,
            status: None,
            loaded: HashMap::new(),
            in_flight: HashSet::new(),
            last_build_ms: 0.0,
            stats: FrameStats::new(),
            teleport: TeleportWindow::new(),
            palette: None,
            palette_rx: None,
            #[cfg(target_arch = "wasm32")]
            palette_job: None,
            #[cfg(target_arch = "wasm32")]
            web_pick: None,
            // Smoke-test hook: open the quick tools HUD at start (`RUSTOSRS_QUICK=1`).
            edited: HashMap::new(),
            history: Vec::new(),
            redo_stack: Vec::new(),
            stroke: None,
            dirty_regions: HashSet::new(),
            cache_error: (!cache_picker::looks_like_cache(&initial_dir)).then(|| {
                if initial_dir.as_os_str().is_empty() {
                    "No cache is open yet.".to_owned()
                } else {
                    format!("No cache found at {}.", initial_dir.display())
                }
            }),
            cache_dir: initial_dir,
            cache_build: None,
            quick_open: std::env::var_os("RUSTOSRS_QUICK").map(|_| egui::pos2(520.0, 300.0)),
            last_hover_tile: None,
            smoke_lay_at: std::env::var("RUSTOSRS_LAY_AT")
                .ok()
                .and_then(|value| value.parse().ok()),
            smoke_spline: std::env::var("RUSTOSRS_SPLINE").ok().map(|value| {
                value
                    .split(';')
                    .filter_map(|pair| {
                        let (x, y) = pair.split_once(',')?;
                        Some((
                            x.trim().parse::<f32>().ok()? + 0.5,
                            y.trim().parse::<f32>().ok()? + 0.5,
                        ))
                    })
                    .collect()
            }),
            smoke_tiles: std::env::var("RUSTOSRS_SELECT_TILES")
                .ok()
                .and_then(|value| {
                    let mut parts = value.split(',').map(|p| p.trim().parse::<i32>().ok());
                    Some((
                        parts.next()??,
                        parts.next()??,
                        parts.next()??,
                        parts.next()??,
                    ))
                }),
            smoke_undo_at: std::env::var("RUSTOSRS_UNDO_AT")
                .ok()
                .and_then(|value| value.parse().ok()),
            smoke_paint: std::env::var("RUSTOSRS_PAINT").ok().and_then(|value| {
                let (x, y) = value.split_once(',')?;
                Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
            }),
            smoke_select: std::env::var("RUSTOSRS_SELECT").ok().and_then(|value| {
                let (x, y) = value.split_once(',')?;
                Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
            }),
            pending_teleport: std::env::var("RUSTOSRS_TELEPORT").ok().and_then(|value| {
                let mut parts = value.split(',').map(|part| part.trim().parse::<i32>().ok());
                Some(Destination::Tile {
                    x: parts.next()??,
                    y: parts.next()??,
                    plane: u8::try_from(parts.next()??).ok()?,
                })
            }),
            screenshot: crate::screenshot::ScreenshotHook::from_env(),
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
        for plugin in &app.plugins {
            app.keymap.register(plugin.actions());
        }
        app.init_settings(&cc.egui_ctx);
        #[cfg(not(target_arch = "wasm32"))]
        app.start_palette_loader(initial_source);
        #[cfg(target_arch = "wasm32")]
        if let Some(base) = crate::platform::web::cache_url_param() {
            app.pick_cache_url(&cc.egui_ctx, base);
        }
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
        self.apply_settings(loaded, ctx);
        self.saved_settings = self.settings_snapshot();
    }

    /// The settings as they are right now: the prefs plus the runtime state that persists.
    fn settings_snapshot(&self) -> Settings {
        Settings {
            fov_degrees: self.camera.fov_degrees,
            fly_speed: self.camera.speed,
            orbit_camera: self.camera.mode == CameraMode::Orbit,
            keymap: self.keymap.to_saved(),
            plugins: self
                .plugins
                .iter()
                .filter_map(|plugin| Some((plugin.name().to_owned(), plugin.save_settings()?)))
                .collect(),
            ..self.prefs.clone()
        }
    }

    /// Adopt loaded settings: store them and push them into the runtime state that mirrors them.
    fn apply_settings(&mut self, settings: Settings, ctx: &egui::Context) {
        let settings = settings.sanitized();
        crate::theme::set_theme(ctx, crate::theme::ALL[settings.appearance]);
        self.camera.fov_degrees = settings.fov_degrees;
        self.camera.speed = settings.fly_speed;
        self.camera.mode = if settings.orbit_camera {
            CameraMode::Orbit
        } else {
            CameraMode::Fly
        };
        self.keymap.load(&settings.keymap);
        for plugin in &mut self.plugins {
            plugin.load_settings(
                settings
                    .plugins
                    .get(plugin.name())
                    .unwrap_or(&serde_json::Value::Null),
            );
        }
        if settings.smooth_terrain != self.prefs.smooth_terrain {
            self.streamer.set_presentation(
                Self::presentation(settings.smooth_terrain, self.presentation_hd),
                self.presentation_generation,
            );
        }
        self.prefs = settings;
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

    fn presentation(smooth_terrain: bool, hd_geometry: bool) -> TerrainPresentation {
        TerrainPresentation {
            smooth_terrain,
            hd_geometry,
            // Always on, no switch: lifts diagonal wall-decoration plates off their wall so the
            // depth buffer does not hide their recessed faces (the software client painted
            // decorations over their wall regardless of depth).
            flush_diagonal_decorations: true,
            ..TerrainPresentation::default()
        }
    }

    /// Rebuild every resident region (after a presentation change).
    fn rebuild_all(&mut self) {
        self.presentation_generation += 1;
        self.presentation_hd = self.prefs.render_profile == ProfileChoice::Hd;
        self.streamer.set_presentation(
            Self::presentation(self.prefs.smooth_terrain, self.presentation_hd),
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
        self.clear_selection();
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

    /// Point the camera at the selected object (orbit pivot in orbit mode).
    fn focus_selection(&mut self) {
        let Some(object) = &self.selected else {
            return;
        };
        let Some(info) = self.infos.get(&object.region) else {
            return;
        };
        let Some(loc) = info.locs.get(object.index) else {
            return;
        };
        let origin = self.renderer.render_origin();
        let world = (
            (loc.tile.0 * 128 + 64) as f32,
            (loc.tile.1 * 128 + 64) as f32,
        );
        let ground = info
            .height_at(usize::from(loc.plane), world.0, world.1)
            .unwrap_or(0.0);
        let extent = loc
            .slots
            .iter()
            .flat_map(|slot| (0..3).map(move |a| (slot.max[a] - slot.min[a]) as f32))
            .fold(0.0_f32, f32::max);
        self.camera.focus(
            [
                world.0 - origin.0 as f32,
                ground - 64.0,
                world.1 - origin.1 as f32,
            ],
            (extent * 1.8).clamp(500.0, 4000.0),
        );
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
    fn stream(&mut self, ctx: &egui::Context) {
        // The HD profile needs geometry with the HD vertex stream: rebuild when it is switched.
        if (self.prefs.render_profile == ProfileChoice::Hd) != self.presentation_hd {
            self.rebuild_all();
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.poll_web_pick(ctx);
            self.run_palette_job();
        }
        // Browser: no worker threads, so run the streamer's queue for a slice of this frame.
        self.streamer.pump(std::time::Duration::from_millis(8));
        while let Some(event) = self.streamer.try_recv() {
            match event {
                StreamEvent::HdMaterials(data) => {
                    self.hd.set_environments(Arc::clone(&data.environments));
                    if let Err(error) = data.install(&mut self.renderer) {
                        self.status = Some(format!("HD materials failed: {error}"));
                    }
                }
                StreamEvent::HdFailed(error) => {
                    self.status = Some(format!("117 HD data failed to load: {error}"));
                }
                StreamEvent::Ready {
                    textures,
                    profile_id,
                    build,
                } => {
                    self.cache_opened(ctx, profile_id, build);
                    self.renderer.set_textures(&textures);
                    self.floor_textures = Some(textures);
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
                StreamEvent::InitFailed(_) if self.cache_dir.as_os_str().is_empty() => {}
                StreamEvent::InitFailed(error) => {
                    // The card explains it; the status bar stays quiet.
                    self.cache_error = Some(if error.contains("No such file") {
                        "The cache files could not be read. Is this the folder that holds main_file_cache.dat2?".to_owned()
                    } else {
                        error
                    });
                }
            }
        }
        if !self.ready {
            return;
        }
        self.flush_dirty_regions();

        let (cx, cy) = self.camera_region();
        let radius = self.prefs.load_radius;
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
        let frozen = !self.prefs.animate_objects;
        if frozen != self.animations_frozen {
            self.animations_frozen = frozen;
            self.animations.set_frozen(frozen);
            self.animation_dirty = true;
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
        // Smoothing blends inside a frame, so it needs every game cycle.
        let stride = if self.animation_smoothing {
            1
        } else {
            ANIMATION_STRIDE_CYCLES
        };
        if (frozen || cycle < self.animation_cycle + stride) && !self.animation_dirty && !moved {
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
        // Camera drags (right and middle button) freeze picking; a left drag paints or selects.
        if response.dragged() && !response.dragged_by(egui::PointerButton::Primary) {
            return;
        }
        let Some(position) = response.hover_pos() else {
            self.hover = Pick::default();
            // The pointer left the viewport: a stroke or box in progress ends here.
            if !response.ctx.input(|i| i.pointer.primary_down()) {
                self.end_stroke();
                self.marquee = None;
            }
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
        if let Some(pick) = self.hover.tile {
            self.last_hover_tile = Some(pick.tile);
        }
        // The brush tools keep the left button for painting; Select clicks and boxes.
        self.brush_input(&response.ctx, response);
        self.spline_input(&response.ctx, response);
        self.selection_input(
            &response.ctx,
            response,
            rect,
            pixels_per_point,
            width,
            height,
        );
        if response.secondary_clicked() && response.ctx.input(|i| i.modifiers.shift) {
            self.context_pick = self.hover.clone();
            self.context_menu = response.interact_pointer_pos();
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
        // The hovered tile, outlined like RuneLite's tile indicator; with a brush tool, the tiles
        // the brush would paint (orange), or grey when nothing is armed.
        if self.prefs.hover_tile
            && let Some(tile) = &self.hover.tile
        {
            let brushing = self.prefs.tool != crate::paint::Tool::Select;
            let tiles = if brushing {
                self.prefs.brush.footprint_tiles(tile.tile)
            } else {
                vec![tile.tile]
            };
            let (fill, edge) = if !brushing {
                (
                    egui::Color32::from_rgba_unmultiplied(0, 255, 255, 28),
                    egui::Color32::from_rgb(0, 255, 255),
                )
            } else if self.prefs.brush.is_empty() {
                (
                    egui::Color32::from_rgba_unmultiplied(200, 200, 200, 22),
                    egui::Color32::from_gray(190),
                )
            } else {
                (
                    egui::Color32::from_rgba_unmultiplied(255, 170, 40, 40),
                    egui::Color32::from_rgb(255, 170, 40),
                )
            };
            for tile in tiles {
                let Some(corners) = interact::tile_corners(&self.infos, tile, self.view_plane)
                else {
                    continue;
                };
                let points: Vec<egui::Pos2> = corners
                    .iter()
                    .filter_map(|corner| {
                        pick::project(
                            &self.camera,
                            self.renderer.render_origin(),
                            width as f32,
                            height as f32,
                            *corner,
                        )
                    })
                    .map(|p| {
                        egui::pos2(
                            rect.min.x + p[0] / pixels_per_point,
                            rect.min.y + p[1] / pixels_per_point,
                        )
                    })
                    .collect();
                if points.len() == 4 {
                    painter.add(egui::Shape::convex_polygon(
                        points,
                        fill,
                        egui::Stroke::new(1.5, edge),
                    ));
                }
            }
        }
        // The in-game action text next to the cursor (RuneLite's mouse tooltips).
        if self.prefs.mouse_tooltips
            && self.prefs.tool == crate::paint::Tool::Select
            && self.context_menu.is_none()
            && let Some(pointer) = ui.input(|i| i.pointer.hover_pos())
            && rect.contains(pointer)
            && !ui.input(|i| i.pointer.any_down())
        {
            let entries = interact::entries(&self.infos, &self.hover);
            if let Some(top) = entries.first() {
                // Menu entries plus the always-present Cancel.
                let extra = interact::more_options(entries.len() + 1);
                let job = interact::entry_job(top, 13.5, extra.as_deref());
                let galley = painter.layout_job(job);
                let size = galley.size() + egui::vec2(12.0, 6.0);
                let mut at = pointer + egui::vec2(14.0, 18.0);
                if at.x + size.x > rect.right() {
                    at.x = pointer.x - size.x - 10.0;
                }
                if at.y + size.y > rect.bottom() {
                    at.y = pointer.y - size.y - 10.0;
                }
                let tip = egui::Rect::from_min_size(at, size);
                painter.rect_filled(tip, 3.0, egui::Color32::from_black_alpha(190));
                painter.rect_stroke(
                    tip,
                    3.0,
                    egui::Stroke::new(1.0, egui::Color32::from_white_alpha(40)),
                    egui::StrokeKind::Inside,
                );
                painter.galley(at + egui::vec2(6.0, 3.0), galley, egui::Color32::WHITE);
            }
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

    /// Fire the editor's own bound actions (none while a text field has keyboard focus, and none
    /// while a binding is being captured in the Keyboard Shortcuts window).
    fn editor_actions(&mut self, ctx: &egui::Context) {
        self.tool_release(ctx);
        if ctx.egui_wants_keyboard_input() || self.capturing_binding.is_some() {
            return;
        }
        let pressed = |id: &str| ctx.input(|i| self.keymap.pressed(i, id));
        let mut open_teleport = false;
        let mut toggle_panel = false;
        let mut toggle_camera = false;
        let mut focus = false;
        let mut clear = false;
        let mut plane = None;
        if pressed("teleport.open") {
            open_teleport = true;
        }
        if pressed("panel.properties") {
            toggle_panel = true;
        }
        if pressed("camera.mode") {
            toggle_camera = true;
        }
        if pressed("camera.focus") {
            focus = true;
        }
        if pressed("selection.clear") {
            clear = true;
        }
        let toggle_map = pressed("worldmap.toggle");

        let open_settings = pressed("settings.open");
        let switch_menu = pressed("contextmenu.style");
        for (index, id) in ["plane.0", "plane.1", "plane.2", "plane.3"]
            .into_iter()
            .enumerate()
        {
            if pressed(id) {
                plane = Some(index as u8);
            }
        }
        if toggle_map {
            self.toggle_world_map();
        }
        if open_settings {
            self.open_settings(None);
            self.settings_focus_search = true;
        }
        if switch_menu {
            self.prefs.context_menu = match self.prefs.context_menu {
                crate::settings::ContextMenuStyle::Editor => {
                    crate::settings::ContextMenuStyle::Game
                }
                crate::settings::ContextMenuStyle::Game => {
                    crate::settings::ContextMenuStyle::Editor
                }
            };
            self.notice = Some((
                format!("Context menu: {}", self.prefs.context_menu.label()),
                Instant::now(),
            ));
        }
        if open_teleport {
            self.open_teleport();
        }
        if toggle_panel {
            self.prefs.properties_panel = !self.prefs.properties_panel;
        }
        if toggle_camera {
            self.camera.mode = match self.camera.mode {
                CameraMode::Fly => CameraMode::Orbit,
                CameraMode::Orbit => CameraMode::Fly,
            };
        }
        if focus {
            self.focus_selection();
        }
        if clear {
            self.escape_action();
        }
        if let Some(plane) = plane {
            self.view_plane = plane;
        }
        self.tool_actions(ctx);
        self.edit_actions(ctx);
    }

    fn handle_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
        let delta = response.drag_delta();
        let shift = ctx.input(|i| i.modifiers.shift);
        // Right drag moves the view in both modes; the left button is left to selection and tools.
        match self.camera.mode {
            CameraMode::Fly => {
                if response.dragged_by(egui::PointerButton::Secondary) && !shift {
                    self.camera.look(delta.x, delta.y);
                }
                if response.dragged_by(egui::PointerButton::Middle)
                    || (response.dragged_by(egui::PointerButton::Secondary) && shift)
                {
                    self.camera.pan(delta.x, delta.y);
                }
            }
            CameraMode::Orbit => {
                if response.dragged_by(egui::PointerButton::Middle)
                    || (response.dragged_by(egui::PointerButton::Secondary) && shift)
                {
                    self.camera.pan(delta.x, delta.y);
                } else if response.dragged_by(egui::PointerButton::Secondary) {
                    self.camera.orbit(delta.x, delta.y);
                }
            }
        }
        if response.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            match self.camera.mode {
                CameraMode::Fly => {
                    if scroll != 0.0 {
                        self.camera.speed =
                            (self.camera.speed * (1.0 + scroll * 0.002)).clamp(50.0, 20_000.0);
                    }
                }
                CameraMode::Orbit => {
                    let pinch = ctx.input(|i| i.zoom_delta());
                    self.camera.zoom(scroll + (pinch - 1.0) * 500.0);
                }
            }
        }
        if ctx.egui_wants_keyboard_input() || self.capturing_binding.is_some() {
            return;
        }
        ctx.input(|i| {
            let axis = |plus: &str, minus: &str| {
                f32::from(self.keymap.held(i, plus)) - f32::from(self.keymap.held(i, minus))
            };
            let boost = if i.modifiers.shift { 4.0 } else { 1.0 };
            let forward = axis("camera.forward", "camera.back") * boost;
            let right = axis("camera.right", "camera.left") * boost;
            let up = axis("camera.up", "camera.down") * boost;
            if forward != 0.0 || right != 0.0 || up != 0.0 {
                self.camera.translate(forward, right, up, dt);
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
        if let Some(tile) = self.smoke_select
            && let Some(pick) = self
                .infos
                .iter()
                .flat_map(|(region, info)| {
                    info.locs
                        .iter()
                        .enumerate()
                        .filter_map(move |(index, loc)| {
                            let named = loc
                                .definition
                                .name
                                .as_deref()
                                .is_some_and(|n| !n.is_empty());
                            let distance =
                                (loc.tile.0 - tile.0).abs().max((loc.tile.1 - tile.1).abs());
                            (named && distance <= 4).then_some((
                                distance,
                                ObjectPick {
                                    region: *region,
                                    index,
                                },
                            ))
                        })
                })
                .min_by_key(|(distance, _)| *distance)
                .map(|(_, pick)| pick)
        {
            // `RUSTOSRS_CONTEXT=1` also opens the context menu on it, to look at the menus.
            if std::env::var_os("RUSTOSRS_CONTEXT").is_some() {
                let anchor = self
                    .infos
                    .get(&pick.region)
                    .and_then(|info| info.locs.get(pick.index))
                    .map(|loc| pick::TilePick {
                        tile: loc.tile,
                        at: (loc.tile.0 as f32 + 0.5, loc.tile.1 as f32 + 0.5),
                    });
                self.context_pick = Pick {
                    object: Some(pick.clone()),
                    tile: anchor,
                };
                self.context_menu = Some(egui::pos2(400.0, 300.0));
            }
            self.select_only(pick);
            self.smoke_select = None;
        }
        if let Some(tile) = self.smoke_paint
            && crate::pick::tile_values(&self.infos, tile, self.view_plane).is_some()
        {
            self.apply_brush_at(tile);
            self.smoke_paint = None;
        }
        if let Some(points) = self.smoke_spline.take() {
            self.spline.points = points;
        }
        if self.prefs.tool == crate::paint::Tool::Spline {
            self.spline_update();
        }
        if let Some(at) = self.smoke_lay_at
            && self.frames >= at
            && !self.spline.points.is_empty()
        {
            self.commit_spline();
            self.smoke_lay_at = None;
        }
        if let Some((x0, y0, x1, y1)) = self.smoke_tiles.take() {
            for x in x0..=x1 {
                for y in y0..=y1 {
                    self.selection_tiles.insert((x, y));
                }
            }
        }
        // Smoke-test hook: undo the last stroke on this frame (`RUSTOSRS_UNDO_AT=frame`).
        if let Some(at) = self.smoke_undo_at
            && self.frames >= at
            && self.can_undo()
        {
            self.undo();
            self.smoke_undo_at = None;
        }
        self.editor_actions(&ctx);
        self.plugin_actions(&ctx);
        if let Some((vx, vz)) = self.autofly {
            let step = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
            self.camera.x += vx * step;
            self.camera.z += vz * step;
        }
        self.rebase_if_needed();
        let phase = Instant::now();
        self.stream(&ctx);
        self.stats.phase("stream", phase);
        let phase = Instant::now();
        self.update_animations();
        self.stats.phase("anim", phase);
        let phase = Instant::now();
        self.run_plugins(|plugin, cx| plugin.update(cx));
        self.stats.phase("plugins", phase);
        self.peak_resident = self
            .peak_resident
            .max(self.loaded.values().flatten().count());

        self.poll_palette();
        self.menu_bar(ui);
        self.toolbar(ui);
        self.status_bar(ui);
        self.tool_strip(ui);
        self.tool_drawer(ui);
        self.dock(ui);
        self.tool_shelf(ui);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let available = ui.available_size();
                let (rect, response) =
                    ui.allocate_exact_size(available, egui::Sense::click_and_drag());
                let pixels_per_point = ctx.pixels_per_point();
                let width = ((rect.width() * pixels_per_point) as u32).max(16);
                let height = ((rect.height() * pixels_per_point) as u32).max(16);

                self.handle_input(&ctx, &response);
                let phase = Instant::now();
                self.update_picking(&response, rect, pixels_per_point, width, height);
                self.stats.phase("pick", phase);

                if self.ready {
                    self.ensure_targets(width, height);
                    if self.prefs.render_profile == ProfileChoice::Hd {
                        self.update_hd_lighting();
                    }
                    if let Some(targets) = &self.targets {
                        let mut encoder = self.render_state.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("viewport"),
                            },
                        );
                        let phase = Instant::now();
                        self.renderer.set_hd_params(self.prefs.hd_params());
                        self.renderer.encode_frame(
                            &mut encoder,
                            &targets.color_view,
                            &targets.depth_view,
                            FrameParams {
                                camera: self.camera.reference(targets.width as f32),
                                view_plane: self.view_plane,
                                // The client's `gameCycle & 127`, at 50 cycles per second.
                                tick: ((self.started.elapsed().as_secs_f32() * 50.0) as u32) & 127,
                                brightness: self.prefs.brightness,
                                remove_color_banding: self.prefs.remove_color_banding,
                                bright_textures: self.prefs.bright_textures,
                                fog: self.prefs.fog(),
                                colorblind: self.prefs.colorblind(),
                                clear_color: self.prefs.sky_color.map(f64::from),
                                profile: self.prefs.render_profile.render_profile(),
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
                self.draw_selection_extras(
                    &ui.painter().with_clip_rect(rect),
                    rect,
                    pixels_per_point,
                    width,
                    height,
                );
                self.draw_spline(
                    &ui.painter().with_clip_rect(rect),
                    rect,
                    pixels_per_point,
                    width,
                    height,
                );
                self.viewport_overlays(ui, rect);
                self.floating_toolbar(&ctx, rect);
                self.context_menu(&ctx);
            });
        self.quick_tools_hud(&ctx);
        self.cache_welcome(&ctx);

        self.settings_window(&ctx);
        self.assets.poll(&ctx);
        self.world_map_window(&ctx);
        if let Some(destination) = self.teleport.show(&ctx) {
            self.teleport_to(destination);
        }
        self.autosave_settings();
        self.stats.end_frame(frame_started);
        self.frames += 1;
        if let Some(hook) = &mut self.screenshot {
            hook.tick(&ctx, self.frames);
        }
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
