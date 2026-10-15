//! Loading, applying and saving the persisted settings.

use super::*;

fn finite_f32_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

/// Clamp persisted Spline settings to the same narrow domain the simplified UI exposes.
fn sanitize_loaded_settings(settings: Settings) -> Settings {
    let mut settings = settings.sanitized();
    let defaults = crate::paint::SplineSettings::default();
    let spline = &mut settings.spline;
    spline.width = finite_f32_or(spline.width, defaults.width).clamp(0.5, 24.0);
    spline.edge_weight = finite_f32_or(spline.edge_weight, defaults.edge_weight).clamp(0.0, 2.0);
    settings
}

impl EditorApp {
    /// Load persisted settings (skipped in smoke runs unless `RUSTOSRS_SETTINGS` is set).
    pub(super) fn init_settings(&mut self, ctx: &egui::Context) {
        let smoke = self.diagnostics.timed_exit() && std::env::var("RUSTOSRS_SETTINGS").is_err();
        if smoke {
            return;
        }
        let Some(loaded) = self.settings.load() else {
            return;
        };
        self.apply_settings(loaded, ctx);
        self.settings.mark_loaded(self.settings_snapshot());
    }

    /// The settings as they are right now: the prefs plus the runtime state that persists.
    pub(super) fn settings_snapshot(&self) -> Settings {
        Settings {
            fov_degrees: self.interaction.camera().fov_degrees,
            fly_speed: self.interaction.camera().speed,
            orbit_camera: self.interaction.camera().mode == CameraMode::Orbit,
            keymap: self.settings.keymap.to_saved(),
            plugins: self.plugins.save_settings(),
            ..self.settings.prefs.clone()
        }
    }

    /// Adopt loaded settings: store them and push them into the runtime state that mirrors them.
    pub(super) fn apply_settings(&mut self, settings: Settings, ctx: &egui::Context) {
        let settings = sanitize_loaded_settings(settings);
        crate::theme::set_theme(ctx, crate::theme::ALL[settings.appearance]);
        self.interaction.set_camera_settings(
            settings.fov_degrees,
            settings.fly_speed,
            if settings.orbit_camera {
                CameraMode::Orbit
            } else {
                CameraMode::Fly
            },
        );
        self.settings.keymap.load(&settings.keymap);
        self.plugins.load_settings(&settings.plugins);
        if settings.smooth_terrain != self.settings.prefs.smooth_terrain {
            self.world.set_smooth_terrain(settings.smooth_terrain);
        }
        self.settings.prefs = settings;
    }

    /// Save the settings when they changed (checked once a second).
    pub(super) fn autosave_settings(&mut self) {
        if self.settings.save_due() {
            self.settings.save_changed(self.settings_snapshot());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaded_spline_settings_are_clamped_to_simple_fitter_ranges() {
        let mut settings = Settings::default();
        settings.spline.width = 999.0;
        settings.spline.edge_weight = -5.0;
        settings.spline.snap_points = true;

        let settings = sanitize_loaded_settings(settings);

        assert_eq!(settings.spline.width, 24.0);
        assert_eq!(settings.spline.edge_weight, 0.0);
        assert!(settings.spline.snap_points);
    }

    #[test]
    fn loaded_spline_settings_recover_from_non_finite_values() {
        let mut settings = Settings::default();
        settings.spline.width = f32::NAN;
        settings.spline.edge_weight = f32::INFINITY;
        let defaults = crate::paint::SplineSettings::default();

        let settings = sanitize_loaded_settings(settings);

        assert_eq!(settings.spline.width, defaults.width);
        assert_eq!(settings.spline.edge_weight, defaults.edge_weight);
    }
}
