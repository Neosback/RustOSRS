//! Runtime state of the HD profile in the editor: the area environment around the camera's focal
//! point, blended over time (`EnvironmentState`), turned into the renderer's lighting each frame.

use super::EditorApp;
use crate::camera::CameraMode;
use osrs_hd::{
    environment::{EnvironmentSet, EnvironmentState},
    expr::NoVariables,
};
use osrs_render::gpu::HdLighting;
use std::sync::Arc;
use web_time::Instant;

/// The environment table and the transition state driven by it.
pub struct HdRuntime {
    environments: Option<Arc<EnvironmentSet>>,
    state: EnvironmentState,
    started: Instant,
    last_update: Instant,
}

impl HdRuntime {
    pub fn new() -> Self {
        Self {
            environments: None,
            state: EnvironmentState::new(),
            started: Instant::now(),
            last_update: Instant::now(),
        }
    }

    /// Install the environment table once the plugin data has loaded.
    pub fn set_environments(&mut self, environments: Arc<EnvironmentSet>) {
        self.environments = Some(environments);
        self.state.reset();
    }

    /// Name of the environment the camera is in (status text, diagnostics).
    pub fn current_name(&self) -> Option<String> {
        let set = self.environments.as_ref()?;
        Some(self.state.target(set).label().to_owned())
    }
}

impl EditorApp {
    /// Advance the environment to the camera's focal tile and hand the renderer its lighting.
    pub(super) fn update_hd_lighting(&mut self) {
        let Some(set) = self.hd.environments.clone() else {
            return;
        };
        let now = Instant::now();
        let delta = now
            .duration_since(self.hd.last_update)
            .as_secs_f32()
            .min(0.25);
        self.hd.last_update = now;

        // The focal point: the orbit pivot, or the camera itself when flying.
        let origin = self.renderer.render_origin();
        let (x, z) = match self.interaction.camera().mode {
            CameraMode::Orbit => {
                let pivot = self.interaction.camera().pivot();
                (pivot[0], pivot[2])
            }
            CameraMode::Fly => (self.interaction.camera().x, self.interaction.camera().z),
        };
        let focal = [
            ((f64::from(origin.0) + f64::from(x)) / 128.0).floor() as i32,
            ((f64::from(origin.1) + f64::from(z)) / 128.0).floor() as i32,
            i32::from(self.interaction.view_plane()),
        ];
        let mut settings = self.settings.prefs.hd_environment;
        settings.host_sky = self.settings.prefs.sky_color;
        let elapsed = f64::from(
            self.diagnostics
                .scene_seconds(now.duration_since(self.hd.started).as_secs_f32()),
        );
        self.hd
            .state
            .update(&set, &settings, focal, &NoVariables, elapsed, delta);
        self.renderer.set_hd_shadows(self.settings.prefs.hd_shadows);
        self.renderer.set_hd_lighting(HdLighting {
            lighting: self.hd.state.lighting(
                &set,
                &settings,
                self.settings.prefs.draw_distance_tiles as f32,
            ),
            normal_mapping: self.settings.prefs.hd_normal_mapping,
            parallax_mapping: self.settings.prefs.hd_parallax_mapping,
            underwater_caustics: self.settings.prefs.hd_underwater_caustics,
            gamma: 100.0 / self.settings.prefs.hd_brightness.max(1.0),
            elapsed_seconds: (elapsed % 4096.0) as f32,
            debug_view: 0,
        });
    }
}
