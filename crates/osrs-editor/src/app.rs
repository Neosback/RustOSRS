//! The eframe application: scene loading, the wgpu viewport, and fly-camera input.

mod diagnostics;
mod init;
mod input;
mod interaction_runtime;

mod audio_runtime;
mod building_pick;
mod cache_picker;
mod camera_ops;
mod context_menu;
mod drawer;
mod edit_runtime;
mod editing;
#[cfg_attr(target_arch = "wasm32", path = "app/export_web.rs")]
mod export;
mod floating;
mod gameframe;
mod gizmo;
mod hd;
mod inspector;
mod map_icons;
mod menu;
mod minimap;
pub(crate) mod modal;
mod names;
mod npc_panel;
mod npc_runtime;
mod object_tool;
mod overlay;
#[allow(dead_code)]
mod paint_commit;
mod palette_runtime;
mod path_commit;
mod player_panel;
mod player_runtime;
mod player_window;
mod plugin_runtime;
mod plugins;
mod prefab_gallery;
mod prefab_panel;
mod prefab_runtime;
mod prefab_studio;
#[cfg_attr(target_arch = "wasm32", path = "app/project_web.rs")]
mod project;
mod project_runtime;
mod render_runtime;
mod roof_view;
mod script;
mod selection;
mod settings_io;
mod settings_runtime;
mod settings_window;
mod shelf;
mod sound_panel;
mod splash;
mod spline_tool;
mod status;
mod streaming;
mod theme_studio;
mod tile_clipboard;
mod tool_strip;
mod toolbar;
mod ui_runtime;
mod user_tools;
mod viewer;
mod viewport;
mod widgets;
mod workshop;
mod workspace;
mod world_runtime;
mod worldmap;

use crate::animation_smoothing::AnimationSmoothingPlugin;
use crate::assets::Assets;
use crate::camera::{CameraMode, FlyCamera};
use crate::grid::GridPlugin;
use crate::hud::Destination;
use crate::interact;
use crate::pick::{self, ObjectPick};
use crate::settings::ProfileChoice;
use crate::settings::Settings;
use crate::style::ColorKey;
use eframe::egui;
use osrs_core::coords::RegionCoord;
use std::{path::PathBuf, sync::Arc};
use web_time::Instant;

/// Local units per map region (64 tiles of 128).
const REGION_UNITS: i32 = 64 * 128;
/// Rebase the render origin once the camera is this far (local units) from it.
const REBASE_DISTANCE: f32 = 90_000.0;

/// Renderer key of the per-frame animated-loc geometry.
const ANIMATED_KEY: (i32, i32) = (i32::MIN, i32::MIN);

/// Composition root. Product state lives in the named runtime/controller owners below; this
/// object coordinates eframe lifecycle, UI actions and projection installation.
pub struct EditorApp {
    project: project_runtime::ProjectRuntime,
    session: edit_runtime::EditRuntime,
    terrain: editing::TerrainController,
    interaction: interaction_runtime::InteractionRuntime,
    world: world_runtime::WorldRuntime,
    renderer: render_runtime::RenderRuntime,
    ui: ui_runtime::UiRuntime,
    settings: settings_runtime::SettingsRuntime,
    diagnostics: diagnostics::DiagnosticsRuntime,
    plugins: plugin_runtime::PluginRuntime,
    palettes: palette_runtime::PaletteRuntime,
    npcs: npc_runtime::NpcRuntime,
    audio: audio_runtime::AudioRuntime,
    player_model: player_runtime::PlayerRuntime,
    viewer: viewer::Viewer,
    prefabs: prefab_runtime::PrefabRuntime,
    names: names::NameRuntime,
    assets: Assets,
    hd: hd::HdRuntime,
    placer: object_tool::PlaceState,
    exports: export::ExportRuntime,
    script: script::ScriptRunner,
    #[cfg(target_arch = "wasm32")]
    web_pick: Option<crate::platform::web::SharedPick>,
    /// Shared application uptime used by animation scheduling and smoke measurements.
    started: Instant,
}

impl eframe::App for EditorApp {
    fn on_exit(&mut self) {
        self.settings.save_on_exit(&self.settings_snapshot());
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        prefab_studio::window(self, &ctx);
        #[cfg(not(target_arch = "wasm32"))]
        if self.export_inspection_ui(ui) {
            self.diagnostics.finish_frame(&ctx);
            if self
                .diagnostics
                .expired(self.started.elapsed().as_secs_f32())
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }
        let frame_started = self.diagnostics.begin_frame();
        if self.settings.prefs.tool == crate::paint::Tool::Spline {
            self.spline_update();
        }
        self.quit_guard(&ctx);
        input::editor_actions(self, &ctx, true);
        self.plugin_actions(&ctx);
        if let Some((vx, vz)) = self.diagnostics.autofly() {
            let step = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
            self.interaction.shift_camera_origin([vx * step, vz * step]);
        }
        self.rebase_if_needed();
        let phase = Instant::now();
        self.stream(&ctx);
        self.diagnostics.phase("stream", phase);
        let phase = Instant::now();
        self.update_animations();
        self.diagnostics.phase("anim", phase);
        let phase = Instant::now();
        viewer::update(self, &ctx);
        npc_panel::update(self, &ctx);
        player_panel::update_model(self, &ctx);
        prefab_panel::update(self);
        prefab_panel::review_placement(self, &ctx);
        if self.settings.prefs.sound_overlay {
            sound_panel::refresh(self);
        }
        self.diagnostics.phase("npcs", phase);
        let phase = Instant::now();
        self.run_plugins(|plugin, cx| plugin.update(cx));
        self.diagnostics.phase("plugins", phase);
        self.diagnostics
            .observe_residency(self.world.resident_regions());

        let phase = Instant::now();
        self.palettes.poll();
        self.poll_catalog();
        self.poll_export();
        self.poll_project();
        self.names.poll();
        if self.viewer.enabled() {
            viewer::frame(self, ui, &ctx);
            self.cache_welcome(&ctx);
            self.assets.poll(&ctx);
            splash::draw(self, &ctx);
            self.diagnostics.end_frame(frame_started);
            self.diagnostics.finish_frame(&ctx);
            ctx.request_repaint();
            return;
        }
        self.menu_bar(ui);
        self.toolbar(ui);
        self.status_bar(ui);
        self.tool_strip(ui);
        self.tool_drawer(ui);
        self.dock(ui);
        self.tool_shelf(ui);
        self.diagnostics.phase("chrome", phase);

        viewport::frame(self, ui, &ctx);
        let phase = Instant::now();
        self.quick_tools_hud(&ctx);
        self.cache_welcome(&ctx);

        self.plugins_window(&ctx);
        if let Some(text) = self.ui.script_window(&ctx, &self.script) {
            self.run_script(&text);
        }
        self.tool_draft_window(&ctx);
        self.settings_window(&ctx);
        theme_studio::frame(self, &ctx);
        self.export_window(&ctx);
        self.assets.poll(&ctx);
        self.world_map_window(&ctx);
        prefab_gallery::window(self, &ctx);
        player_window::draw_player_window(self, &ctx);
        workshop::window(self, &ctx);
        if let Some(destination) = self.ui.teleport.show(&ctx) {
            self.teleport_to(destination);
        }
        self.autosave_settings();
        splash::draw(self, &ctx);
        self.diagnostics.phase("windows", phase);
        self.diagnostics.end_frame(frame_started);
        self.diagnostics.finish_frame(&ctx);
        if self
            .diagnostics
            .expired(self.started.elapsed().as_secs_f32())
        {
            self.diagnostics.report_smoke(diagnostics::SmokeSnapshot {
                seconds: self.started.elapsed().as_secs_f32(),
                regions: self.world.resident_regions(),
                vertex_bytes: self.renderer.resident_vertex_bytes(),
                zones: self.renderer.resident_zone_count(),
                tile: self.camera_tile(),
                region: self.camera_region(),
            });
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Continuous repaint while the viewport is live.
        ctx.request_repaint();
    }
}
