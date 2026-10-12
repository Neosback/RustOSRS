//! The eframe application: scene loading, the wgpu viewport, and fly-camera input.

mod cache_picker;
mod camera_ops;
mod context_menu;
mod drawer;
mod editing;
mod floating;
mod gizmo;
mod hd;
mod inspector;
mod map_icons;
mod menu;
mod minimap;
pub(crate) mod modal;
mod object_tool;
mod overlay;
mod path_commit;
mod plugins;
mod script;
mod selection;
mod settings_io;
mod settings_window;
mod shelf;
mod spline_tool;
mod status;
mod streaming;
mod tool_strip;
mod toolbar;
mod user_tools;
mod viewport;
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
use crate::style::ColorKey;
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
    /// The selected objects being dragged by the Objects tool.
    object_drag: Option<object_tool::ObjectDrag>,
    /// The Objects tool's transform gizmo, its drag in progress, and whether it has the pointer.
    gizmo: transform_gizmo_egui::Gizmo,
    gizmo_drag: Option<gizmo::GizmoDrag>,
    gizmo_busy: bool,
    /// Placements to select once their rebuilt region arrives (after an object edit).
    pending_reselect: Vec<osrs_world::LocSpec>,
    /// The script language's queue, its window and text.
    script: script::ScriptRunner,
    script_open: bool,
    /// The Plugins window (opened from the menu bar).
    plugins_open: bool,
    script_text: String,
    /// A user tool being made or edited.
    tool_draft: Option<crate::user_tools::ToolDraft>,
    /// Where the floating picker rail is (the HUDs keep clear of it).
    /// Where the stats / tile HUD was drawn last frame (the pointer over it still picks the world).
    hud_rect: Option<egui::Rect>,
    rail_rect: Option<egui::Rect>,
    /// Where the quick tools HUD (Space) is open.
    quick_open: Option<egui::Pos2>,
    /// The last tile the cursor was over (the Tile tab keeps showing it after the cursor leaves).
    last_hover_tile: Option<(i32, i32)>,
    /// The tile the Inspector shows when it is not following the cursor: the last one selected
    /// (or the tile under the last object selected).
    inspect_tile: Option<(i32, i32)>,
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
        crate::theme::set_theme(&cc.egui_ctx, crate::theme::ALL[crate::theme::DEFAULT]);
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
            object_drag: None,
            gizmo: Self::new_gizmo(),
            gizmo_drag: None,
            gizmo_busy: false,
            pending_reselect: Vec::new(),
            script: script::ScriptRunner::default(),
            script_open: false,
            plugins_open: false,
            script_text: String::new(),
            tool_draft: None,
            hud_rect: None,
            rail_rect: None,
            quick_open: std::env::var_os("RUSTOSRS_QUICK").map(|_| egui::pos2(520.0, 300.0)),
            last_hover_tile: None,
            inspect_tile: None,
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
        // Start-up script (`RUSTOSRS_SCRIPT="wait 120; tool paint; radius 2"`), for smoke tests.
        let script = Self::env_script();
        if !script.is_empty() {
            app.run_script(&script);
        }
        #[cfg(not(target_arch = "wasm32"))]
        app.start_palette_loader(initial_source);
        #[cfg(target_arch = "wasm32")]
        if let Some(base) = crate::platform::web::cache_url_param() {
            app.pick_cache_url(&cc.egui_ctx, base);
        }
        Ok(app)
    }

    /// Fire the editor's own bound actions (none while a text field has keyboard focus, and none
    /// while a binding is being captured in the Keyboard Shortcuts window).
    fn editor_actions(&mut self, ctx: &egui::Context) {
        self.tool_release(ctx);
        if self.keys_blocked(ctx) {
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
        self.script_frame(ctx);
        self.object_actions(ctx);
        self.resolve_reselect();
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
        if self.keys_blocked(ctx) {
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
        if self.prefs.tool == crate::paint::Tool::Spline {
            self.spline_update();
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
                // The viewport renders at the window's pixel density times the resolution scale; every
                // pick and overlay maps target pixels back to points through the same factor.
                let pixels_per_point = ctx.pixels_per_point() * self.prefs.render_scale;
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
                self.draw_map_icons_3d(
                    &ui.painter().with_clip_rect(rect),
                    rect,
                    pixels_per_point,
                    width,
                    height,
                );
                self.draw_object_drag(
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
                self.object_gizmo(ui, rect, &response);
                self.viewport_overlays(ui, rect);
                self.floating_toolbar(&ctx, rect);
                self.context_menu(&ctx);
            });
        self.quick_tools_hud(&ctx);
        self.cache_welcome(&ctx);

        self.plugins_window(&ctx);
        self.script_window(&ctx);
        self.tool_draft_window(&ctx);
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
