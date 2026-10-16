//! Building the editor application.

use super::*;

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

        Self::from_source(
            &cc.egui_ctx,
            render_state,
            osrs_cache::source::CacheSource::Dir(cache_dir.clone()),
            cache_dir,
            base_x,
            base_y,
            true,
        )
    }

    /// Composition shared with isolated read-only export inspection; no authoring state is moved.
    pub(super) fn from_source(
        ctx: &egui::Context,
        render_state: eframe::egui_wgpu::RenderState,
        source: osrs_cache::source::CacheSource,
        cache_dir: PathBuf,
        base_x: i32,
        base_y: i32,
        authoring: bool,
    ) -> Result<Self, String> {
        // Start above the middle of the requested window's region, looking north.
        let start_region = (base_x.div_euclid(64) + 1, base_y.div_euclid(64) + 1);
        let origin = (start_region.0 * REGION_UNITS, start_region.1 * REGION_UNITS);
        let renderer = render_runtime::RenderRuntime::new(render_state, origin);
        let camera = FlyCamera::new(
            0.45 * REGION_UNITS as f32,
            -2600.0,
            0.3 * REGION_UNITS as f32,
        );
        let initial_dir = cache_dir.clone();
        let initial_source = source;
        let mut world = world_runtime::WorldRuntime::new(
            world_runtime::WorldRuntime::spawn_streamer(initial_source.clone()),
        );

        let diagnostics = diagnostics::DiagnosticsRuntime::new(authoring);
        world.set_diagnostics(diagnostics.sink());
        let mut session = edit_runtime::EditRuntime::default();
        session.set_diagnostics(diagnostics.sink());
        let mut app = Self {
            renderer,
            settings: settings_runtime::SettingsRuntime::new(),
            ui: ui_runtime::UiRuntime::new(),
            placer: object_tool::PlaceState::default(),
            exports: export::ExportRuntime::default(),
            project: project_runtime::ProjectRuntime::new(
                initial_source.clone(),
                initial_dir.clone(),
                (!cache_picker::looks_like_cache(&initial_dir)).then(|| {
                    if initial_dir.as_os_str().is_empty() {
                        "No cache is open yet.".to_owned()
                    } else {
                        format!("No cache found at {}.", initial_dir.display())
                    }
                }),
            ),
            interaction: interaction_runtime::InteractionRuntime::new(camera),
            session,
            terrain: editing::TerrainController::default(),
            plugins: plugin_runtime::PluginRuntime::new(vec![
                Box::new(GridPlugin::new()),
                Box::new(AnimationSmoothingPlugin::new()),
                Box::new(crate::wiki::WikiPlugin::new()),
            ]),
            assets: Assets::new(initial_source.clone()),
            palettes: palette_runtime::PaletteRuntime::default(),
            npcs: npc_runtime::NpcRuntime::default(),
            audio: audio_runtime::AudioRuntime::default(),
            player_model: player_runtime::PlayerRuntime::default(),
            names: names::NameRuntime::default(),
            hd: hd::HdRuntime::new(),
            world,
            diagnostics,
            #[cfg(target_arch = "wasm32")]
            web_pick: None,
            script: script::ScriptRunner::default(),
            started: Instant::now(),
        };
        debug_assert_eq!(app.session.revision(), 0);
        for plugin in app.plugins.plugins() {
            app.settings.keymap.register(plugin.actions());
        }
        if authoring {
            app.init_settings(ctx);
            if cache_picker::looks_like_cache(&initial_dir) {
                app.ui.begin_loading();
            }
        }
        // Start-up script (`RUSTOSRS_SCRIPT="wait 120; tool paint; radius 2"`), for smoke tests.
        let script = Self::env_script();
        if authoring && !script.is_empty() {
            app.run_script(&script);
        }
        app.start_catalog_loader(initial_source.clone());
        #[cfg(not(target_arch = "wasm32"))]
        app.palettes.load(initial_source);
        #[cfg(target_arch = "wasm32")]
        if let Some(base) = crate::platform::web::cache_url_param() {
            app.pick_cache_url(ctx, base);
        }
        if !authoring {
            app.ui.settings_open = false;
            app.ui.quick_open = None;
        }
        Ok(app)
    }
}
