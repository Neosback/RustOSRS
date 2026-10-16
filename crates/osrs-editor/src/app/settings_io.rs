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
    let clamp = |value: f32, fallback: f32, range: std::ops::RangeInclusive<f32>| {
        finite_f32_or(value, fallback).clamp(*range.start(), *range.end())
    };
    spline.width = clamp(
        spline.width,
        defaults.width,
        crate::paint::SplineSettings::WIDTH,
    );
    spline.edge_smoothing = clamp(
        spline.edge_smoothing,
        defaults.edge_smoothing,
        crate::paint::SplineSettings::EDGE,
    );
    spline.height_blend = clamp(
        spline.height_blend,
        defaults.height_blend,
        crate::paint::SplineSettings::BLEND,
    );
    spline.height_passes = spline.height_passes.clamp(
        *crate::paint::SplineSettings::PASSES.start(),
        *crate::paint::SplineSettings::PASSES.end(),
    );
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
    fn loaded_spline_settings_are_clamped_to_the_sliders_ranges() {
        let mut settings = Settings::default();
        settings.spline.width = 999.0;
        settings.spline.edge_smoothing = -3.0;
        settings.spline.height_passes = 100_000;
        settings.spline.height_blend = 500.0;
        settings.spline.smooth_height = true;

        let settings = sanitize_loaded_settings(settings);

        assert_eq!(settings.spline.width, 20.0);
        assert_eq!(settings.spline.edge_smoothing, 0.0);
        assert_eq!(settings.spline.height_passes, 250);
        assert_eq!(settings.spline.height_blend, 30.0);
        assert!(settings.spline.smooth_height);
        let mut low = Settings::default();
        low.spline.height_passes = 0;
        low.spline.width = 0.1;
        let low = sanitize_loaded_settings(low);
        assert_eq!((low.spline.height_passes, low.spline.width), (1, 1.0));
    }

    #[test]
    fn loaded_spline_settings_recover_from_non_finite_values() {
        let mut settings = Settings::default();
        settings.spline.width = f32::NAN;
        settings.spline.edge_smoothing = f32::INFINITY;
        settings.spline.height_blend = f32::NAN;
        let defaults = crate::paint::SplineSettings::default();

        let settings = sanitize_loaded_settings(settings);

        assert_eq!(settings.spline.width, defaults.width);
        assert_eq!(settings.spline.edge_smoothing, defaults.edge_smoothing);
        assert_eq!(settings.spline.height_blend, defaults.height_blend);
    }

    #[test]
    fn spline_settings_from_older_versions_load_with_defaults() -> Result<(), serde_json::Error> {
        // The per-point and census-era fields are gone; files that still name them load.
        let settings: crate::paint::SplineSettings = serde_json::from_str(
            r#"{ "width": 5.0, "smooth_ground": true, "wander": "Natural" }"#,
        )?;
        assert_eq!(settings.width, 5.0);
        assert!(settings.smooth_curve && !settings.smooth_height);
        Ok(())
    }
}
