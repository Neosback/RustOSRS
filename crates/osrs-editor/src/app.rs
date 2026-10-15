//! The eframe application: scene loading, the wgpu viewport, and fly-camera input.

mod init;
mod input;
mod interaction_runtime;

mod cache_picker;
mod camera_ops;
mod context_menu;
mod drawer;
mod edit_runtime;
mod editing;
#[cfg_attr(target_arch = "wasm32", path = "app/export_web.rs")]
mod export;
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
#[allow(dead_code)]
mod paint_commit;
mod path_commit;
mod plugins;
#[cfg_attr(target_arch = "wasm32", path = "app/project_web.rs")]
mod project;
mod render_runtime;
mod script;
mod selection;
mod settings_io;
mod settings_window;
mod shelf;
mod spline_tool;
mod status;
mod streaming;
mod tile_clipboard;
mod tool_strip;
mod toolbar;
mod user_tools;
mod viewport;
mod widgets;
mod workspace;
mod world_runtime;
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
use eframe::egui;
use osrs_core::coords::RegionCoord;
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

/// `(region, loc index)` of a picked object.
type OutlineKey = ((i32, i32), usize);

/// Renderer key of the per-frame animated-loc geometry.
const ANIMATED_KEY: (i32, i32) = (i32::MIN, i32::MIN);

pub struct EditorApp {
    renderer: render_runtime::RenderRuntime,
    /// Tiles copied with Cmd/Ctrl+C, ready to paste.
    clipboard: Option<tile_clipboard::TileClipboard>,
    /// Searchable object list and the object armed for placement.
    placer: object_tool::PlaceState,
    /// Export snapshot, preflight, background work, and persistent review/report.
    exports: export::ExportRuntime,
    /// The opened project's unapplied edits, its file, and a running save.
    project: project::ProjectState,
    /// Camera, picking, selection, and in-progress tool gestures.
    interaction: interaction_runtime::InteractionRuntime,
    /// Persistent authoritative semantic edit runtime. Existing callers still use the historical
    /// `session` field name while their transaction policy migrates behind the gateway.
    session: edit_runtime::EditRuntime,
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
    /// The HD profile's area environments and their transition state.
    hd: hd::HdRuntime,
    /// Background (and the colour holes in the terrain show). RuneLite's default sky is black.
    /// Inspection data of the resident regions (for picking).
    infos: Infos,
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
    /// Region residency, stream requests, and animation scheduling (projection state only).
    world: world_runtime::WorldRuntime,
    /// A short message shown in the status bar (menu choices that need a client).
    notice: Option<(String, Instant)>,
    status: Option<String>,
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
    /// The cache folder in use, the build and profile it matched, and why it could not be opened.
    cache_dir: PathBuf,
    cache_build: Option<(u32, String)>,
    cache_error: Option<String>,
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

impl eframe::App for EditorApp {
    fn on_exit(&mut self) {
        if let Some(path) = &self.settings_path {
            let _ = self.settings_snapshot().save(path);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        #[cfg(not(target_arch = "wasm32"))]
        if self.export_inspection_ui(ui) {
            self.frames += 1;
            if let Some(hook) = &mut self.screenshot {
                hook.tick(&ctx, self.frames);
            }
            if self
                .exit_after_seconds
                .is_some_and(|limit| self.started.elapsed().as_secs_f32() >= limit)
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }
        let frame_started = self.stats.begin_frame();
        if self.prefs.tool == crate::paint::Tool::Spline {
            self.spline_update();
        }
        self.quit_guard(&ctx);
        self.editor_actions(&ctx);
        self.plugin_actions(&ctx);
        if let Some((vx, vz)) = self.autofly {
            let step = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
            self.interaction.camera.x += vx * step;
            self.interaction.camera.z += vz * step;
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
        self.peak_resident = self.peak_resident.max(self.world.resident_regions());

        self.poll_palette();
        self.poll_catalog();
        self.poll_export();
        self.poll_project();
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

                self.render_viewport_image(ui, rect, width, height);
                self.draw_selection(ui, rect, pixels_per_point, width, height);
                self.draw_selection_extras(
                    &ui.painter().with_clip_rect(rect),
                    rect,
                    pixels_per_point,
                    width,
                    height,
                );
                self.draw_place_ghost(
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
        self.export_window(&ctx);
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
                self.world.resident_regions(),
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
