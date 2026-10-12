//! Loading, applying and saving the persisted settings.

use super::*;

impl EditorApp {
    /// Load persisted settings (skipped in smoke runs unless `RUSTOSRS_SETTINGS` is set).
    pub(super) fn init_settings(&mut self, ctx: &egui::Context) {
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
    pub(super) fn settings_snapshot(&self) -> Settings {
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
    pub(super) fn apply_settings(&mut self, settings: Settings, ctx: &egui::Context) {
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
    pub(super) fn autosave_settings(&mut self) {
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
}
