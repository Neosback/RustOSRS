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
        let initial_source = osrs_cache::source::CacheSource::Dir(cache_dir);
        let world = world_runtime::WorldRuntime::new(world_runtime::WorldRuntime::spawn_streamer(
            initial_source.clone(),
        ));

        let mut app = Self {
            renderer,
            export_job: None,
            interaction: interaction_runtime::InteractionRuntime::new(camera),
            session: EditorSession::default(),
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
            hd: hd::HdRuntime::new(),
            infos: Infos::new(),
            floor_textures: None,
            texture_handles: HashMap::new(),
            palette_search: String::new(),
            height_edited: HashSet::new(),
            stroke_counter: 0,
            outlines: HashMap::new(),
            outline_requested: HashSet::new(),
            context_menu: None,
            keymap: Keymap::new(),
            // Smoke-test hook: open the settings window at start (`RUSTOSRS_OPEN_SETTINGS=1`).
            settings_open: std::env::var_os("RUSTOSRS_OPEN_SETTINGS").is_some(),
            settings_search: String::new(),
            settings_focus_search: false,
            capturing_binding: None,
            world,
            notice: None,
            status: None,
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
            cache_error: (!cache_picker::looks_like_cache(&initial_dir)).then(|| {
                if initial_dir.as_os_str().is_empty() {
                    "No cache is open yet.".to_owned()
                } else {
                    format!("No cache found at {}.", initial_dir.display())
                }
            }),
            cache_dir: initial_dir,
            cache_build: None,
            script: script::ScriptRunner::default(),
            script_open: false,
            plugins_open: false,
            script_text: String::new(),
            tool_draft: None,
            hud_rect: None,
            rail_rect: None,
            quick_open: std::env::var_os("RUSTOSRS_QUICK").map(|_| egui::pos2(520.0, 300.0)),
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
        debug_assert_eq!(app.session.revision(), 0);
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
}
