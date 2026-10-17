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
    lights: Option<Arc<osrs_hd::lights::LightSet>>,
    state: EnvironmentState,
    started: Instant,
    last_update: Instant,
    resolved: osrs_hd::theme::ThemeOverrides,
}

impl HdRuntime {
    pub fn new() -> Self {
        Self {
            environments: None,
            lights: None,
            state: EnvironmentState::new(),
            started: Instant::now(),
            last_update: Instant::now(),
            resolved: Default::default(),
        }
    }

    pub(super) fn params(
        &self,
        mut base: osrs_render::gpu::HdParams,
    ) -> osrs_render::gpu::HdParams {
        if let Some(value) = self.resolved.exposure {
            base.exposure = value.get();
        }
        base
    }

    /// Install the environment table once the plugin data has loaded.
    pub fn set_environments(&mut self, environments: Arc<EnvironmentSet>) {
        self.environments = Some(environments);
        self.state.reset();
    }

    /// Install the loaded point and placed light set.
    pub fn set_lights(&mut self, lights: Arc<osrs_hd::lights::LightSet>) {
        self.lights = Some(lights);
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
        let mut elapsed = f64::from(
            self.diagnostics
                .scene_seconds(now.duration_since(self.hd.started).as_secs_f32()),
        );
        if self.ui.themes.compare && self.ui.themes.preview_time.is_none() {
            self.ui.themes.preview_time = Some(elapsed as f32);
        }
        if let Some(time) = self.ui.themes.preview_time {
            elapsed = f64::from(time);
        }
        self.hd
            .state
            .update(&set, &settings, focal, &NoVariables, elapsed, delta);
        self.renderer.set_hd_shadows(self.settings.prefs.hd_shadows);
        let query = osrs_hd::theme::AppearanceQuery {
            tile: osrs_core::coords::MapTile::new(focal[0], focal[1]),
            plane: osrs_core::coords::PlaneIndex::new(self.interaction.view_plane())
                .unwrap_or_else(|| unreachable!("view plane is clamped to 0–3")),
            underlay: None,
            overlay: None,
            object: None,
            kind: None,
            orientation: None,
        };
        let values = self.ui.themes.values(self.session.appearance(), &query);
        let mut lighting = self.hd.state.lighting(
            &set,
            &settings,
            self.settings.prefs.draw_distance_tiles as f32,
        );
        values.apply_lighting(
            &mut lighting,
            self.settings.prefs.draw_distance_tiles as f32,
        );
        self.renderer.set_hd_lighting(HdLighting {
            lighting,
            normal_mapping: values
                .normal_mapping
                .unwrap_or(self.settings.prefs.hd_normal_mapping),
            parallax_mapping: values
                .parallax_mapping
                .unwrap_or(self.settings.prefs.hd_parallax_mapping),
            underwater_caustics: values
                .underwater_caustics
                .unwrap_or(self.settings.prefs.hd_underwater_caustics),
            gamma: 100.0
                / values
                    .brightness
                    .map_or(self.settings.prefs.hd_brightness, |n| n.get())
                    .max(1.0),
            elapsed_seconds: (elapsed % 4096.0) as f32,
            debug_view: 0,
        });
        if let Some(lights) = &self.hd.lights {
            let visible = lights.collect_visible(
                i32::from(self.interaction.view_plane()),
                (f64::from(origin.0) + f64::from(x)) as f32,
                (f64::from(origin.1) + f64::from(z)) as f32,
                self.settings.prefs.draw_distance_tiles as f32,
                elapsed,
                100,
            );
            let gpu_lights: Vec<osrs_render::gpu::hd::layout::GpuPointLight> = visible
                .into_iter()
                .map(|l| osrs_render::gpu::hd::layout::GpuPointLight {
                    position: [
                        l.world_x - origin.0 as f32,
                        l.world_y,
                        l.world_z - origin.1 as f32,
                        l.radius_squared(),
                    ],
                    color: [
                        l.color[0] * l.strength,
                        l.color[1] * l.strength,
                        l.color[2] * l.strength,
                        0.0,
                    ],
                })
                .collect();
            self.renderer.set_hd_point_lights(&gpu_lights);
        }
        self.hd.resolved = values;
    }
}
