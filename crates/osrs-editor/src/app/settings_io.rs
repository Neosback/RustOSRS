//! Loading, applying and saving the persisted settings.

use super::*;

fn finite_f32_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

/// Settings are persisted independently of the controls that currently expose them. Clamp the
/// spline subtree to the editor's actual UI ranges before adopting it so stale or hand-edited
/// values cannot silently bypass the controls, especially the wall-band sizes used directly by
/// preview and commit generation.
fn sanitize_loaded_settings(settings: Settings) -> Settings {
    let mut settings = settings.sanitized();
    let defaults = crate::paint::SplineSettings::default();
    let spline = &mut settings.spline;

    spline.planner = spline.planner.sanitized();
    spline.profile = spline.profile.clone().sanitized();

    // The low-level planner accepts a wider domain than the interactive editor. Persisted editor
    // settings should still mean exactly what its sliders show.
    spline.planner.snap_radius = spline.planner.snap_radius.clamp(0.0, 12.0);
    spline.planner.route_corridor = spline.planner.route_corridor.clamp(1.0, 64.0);
    spline.planner.noise_strength = spline.planner.noise_strength.clamp(0.0, 3.0);
    spline.profile.base_width = spline.profile.base_width.clamp(0.5, 24.0);
    spline.profile.centerline_wander = spline.profile.centerline_wander.clamp(0.0, 1.5);
    spline.profile.edge_noise = spline.profile.edge_noise.clamp(0.0, 1.5);
    spline.profile.obstacle_clearance = spline.profile.obstacle_clearance.clamp(0.0, 4.0);

    spline.wall_thickness =
        finite_f32_or(spline.wall_thickness, defaults.wall_thickness).clamp(1.0, 12.0);
    spline.wall_height = finite_f32_or(spline.wall_height, defaults.wall_height).clamp(1.0, 3.0);
    spline.tower_size = finite_f32_or(spline.tower_size, defaults.tower_size).clamp(1.0, 4.0);

    settings
}

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
        let settings = sanitize_loaded_settings(settings);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaded_spline_settings_are_clamped_to_editor_ranges() {
        let mut settings = Settings::default();
        settings.spline.planner.sample_spacing = 0.001;
        settings.spline.planner.smoothness = 2.0;
        settings.spline.planner.supersample = 99;
        settings.spline.planner.snap_radius = 99.0;
        settings.spline.planner.route_corridor = 999.0;
        settings.spline.planner.noise_strength = 99.0;
        settings.spline.profile.base_width = 999.0;
        settings.spline.profile.edge_falloff = 2.0;
        settings.spline.profile.centerline_wander = 99.0;
        settings.spline.profile.width_variation = 2.0;
        settings.spline.profile.edge_noise = 99.0;
        settings.spline.profile.obstacle_clearance = 99.0;
        settings.spline.wall_thickness = 999.0;
        settings.spline.wall_height = 99.0;
        settings.spline.tower_size = 99.0;

        let settings = sanitize_loaded_settings(settings);
        let spline = settings.spline;

        assert_eq!(spline.planner.sample_spacing, 0.05);
        assert_eq!(spline.planner.smoothness, 1.0);
        assert_eq!(spline.planner.supersample, 16);
        assert_eq!(spline.planner.snap_radius, 12.0);
        assert_eq!(spline.planner.route_corridor, 64.0);
        assert_eq!(spline.planner.noise_strength, 3.0);
        assert_eq!(spline.profile.base_width, 24.0);
        assert_eq!(spline.profile.edge_falloff, 1.0);
        assert_eq!(spline.profile.centerline_wander, 1.5);
        assert_eq!(spline.profile.width_variation, 0.95);
        assert_eq!(spline.profile.edge_noise, 1.5);
        assert_eq!(spline.profile.obstacle_clearance, 4.0);
        assert_eq!(spline.wall_thickness, 12.0);
        assert_eq!(spline.wall_height, 3.0);
        assert_eq!(spline.tower_size, 4.0);
    }

    #[test]
    fn loaded_wall_sizes_recover_from_non_finite_values() {
        let mut settings = Settings::default();
        settings.spline.wall_thickness = f32::NAN;
        settings.spline.wall_height = f32::INFINITY;
        settings.spline.tower_size = f32::NEG_INFINITY;

        let settings = sanitize_loaded_settings(settings);
        let defaults = crate::paint::SplineSettings::default();

        assert_eq!(settings.spline.wall_thickness, defaults.wall_thickness);
        assert_eq!(settings.spline.wall_height, defaults.wall_height);
        assert_eq!(settings.spline.tower_size, defaults.tower_size);
    }
}
