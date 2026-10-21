//! Region streaming and animation upkeep: what loads, what rebuilds, what animates.

#[cfg(all(test, not(target_arch = "wasm32")))]
mod animation_tests;
mod region_install;

use super::*;
use osrs_world::StreamEvent;

impl EditorApp {
    /// Rebuild every resident region (after a presentation change).
    pub(super) fn rebuild_all(&mut self) {
        let hd = self.settings.prefs.render_profile == ProfileChoice::Hd;
        for key in self
            .world
            .begin_rebuild_all(self.settings.prefs.smooth_terrain, hd)
        {
            self.renderer.remove_region(key);
            self.world.remove_info(key);
        }
        self.renderer
            .upload_transient(ANIMATED_KEY, &osrs_render::SceneGeometry::default());
        self.world.clear_outlines();
        self.clear_selection();
    }

    /// Apply finished builds, then request and unload regions around the camera.
    pub(super) fn stream(&mut self, ctx: &egui::Context) {
        // The HD profile needs geometry with the HD vertex stream: rebuild when it is switched.
        if self
            .world
            .hd_mismatch(self.settings.prefs.render_profile == ProfileChoice::Hd)
        {
            self.rebuild_all();
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.poll_web_pick(ctx);
            self.palettes.run_job(self.world.is_ready());
        }
        // Browser: no worker threads, so run the streamer's queue for a slice of this frame.
        self.world.pump(std::time::Duration::from_millis(8));
        while let Some(event) = self.world.try_recv() {
            match event {
                StreamEvent::HdMaterials(data) => {
                    self.hd.set_environments(Arc::clone(&data.environments));
                    self.hd.set_lights(Arc::clone(&data.lights));
                    if let Err(error) = self.renderer.install_hd_materials(*data) {
                        self.ui.set_status(format!("HD materials failed: {error}"));
                    }
                }
                StreamEvent::NpcCatalog { entries, errors } => {
                    self.npcs.catalog_arrived(entries, errors);
                }
                StreamEvent::NpcAssets(update) => self.npcs.assets_arrived(update),
                StreamEvent::PlayerGeometry(update) => {
                    self.player_model.arrived(update);
                }
                StreamEvent::Sound { id, result } => self.audio.arrived(id, result),
                StreamEvent::NpcGeometry(update) => {
                    self.npcs.geometry_arrived(update);
                }
                StreamEvent::HdFailed(error) => {
                    self.ui
                        .set_status(format!("117 HD data failed to load: {error}"));
                }
                StreamEvent::Ready {
                    textures,
                    profile_id,
                    build,
                    fingerprint,
                    floors,
                } => {
                    self.world.set_floors(floors);
                    // `Ready` is emitted exactly once by each newly opened cache worker. Visual
                    // presentation rebuilds do not emit it, so this is the cache/source boundary
                    // where old semantic state must be discarded.
                    self.session.reset();
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Some(appearance) = self.project.take_pending_appearance()
                        && let Err(error) = self.session.load_appearance(appearance)
                    {
                        self.ui
                            .notify(format!("Project appearance could not load: {error}"));
                    }
                    self.ui.themes.source_opened();
                    self.renderer.cancel_theme_preparation();
                    self.audio.source_opened();
                    self.player_model.source_opened(&mut self.renderer);
                    if self.prefabs.stage_region.is_none() {
                        self.names.source_opened(self.project.cache_label(), build);
                        self.npcs.source_opened(
                            fingerprint,
                            build,
                            &self.world,
                            &mut self.renderer,
                        );
                    }
                    self.cache_opened(ctx, profile_id, build);
                    self.renderer.set_textures(&textures);
                    self.palettes.set_textures(textures);
                    self.world.mark_ready();
                }
                StreamEvent::Region(region) => {
                    region_install::install_region(self, *region);
                }
                StreamEvent::Outline {
                    identity,
                    region,
                    index,
                    triangles,
                } => {
                    if let Some(latency_ms) =
                        self.world
                            .install_outline((region, index), identity, triangles)
                    {
                        self.diagnostics.event("outline.latency", latency_ms);
                    }
                }
                StreamEvent::Empty(region) => {
                    region_install::finish_unavailable_region(self, region, None);
                }
                StreamEvent::Failed(region, error) => {
                    region_install::finish_unavailable_region(self, region, Some(&error));
                }
                StreamEvent::InitFailed(_) if self.project.cache_label().as_os_str().is_empty() => {
                }
                StreamEvent::InitFailed(error) => {
                    self.ui.cancel_loading();
                    // The card explains it; the status bar stays quiet.
                    self.project.set_cache_error(if error.contains("No such file") {
                        "The cache files could not be read. Is this the folder that holds main_file_cache.dat2?".to_owned()
                    } else {
                        error
                    });
                }
            }
        }
        if !self.world.is_ready() {
            return;
        }
        self.world.set_semantic_revision(self.session.revision());
        self.world.flush_dirty_regions();

        let center = self
            .prefabs
            .stage_region
            .map_or_else(|| self.camera_region(), |r| (r.x, r.y));
        let radius = if self.prefabs.stage_region.is_some() {
            1
        } else {
            self.settings.prefs.load_radius
        };
        self.world.request_missing(center, radius);
        for key in self.world.unload_stale(center, radius) {
            let _ = self.session.unload_region(RegionCoord::new(key.0, key.1));
            self.renderer.remove_region(key);
            self.world.remove_info(key);
        }
    }

    /// Advance animated locs to the current game cycle on the animation worker and upload the
    /// geometry it finishes.
    pub(super) fn update_animations(&mut self) {
        self.world
            .set_animation_playback(self.settings.prefs.animate_objects);
        if let Some(artifact) = self.world.poll_animation_geometry() {
            self.diagnostics.event("anim.rebuild", artifact.rebuild_ms);
            let phase = Instant::now();
            self.renderer.upload_animation(&artifact);
            self.diagnostics.phase("anim-upload", phase);
        }
        let (world_x, world_z) = self.world_position();
        self.world.schedule_animation(
            self.settings.prefs.animate_objects,
            self.diagnostics
                .scene_seconds(self.started.elapsed().as_secs_f32()),
            ((world_x / 128.0) as i32, (world_z / 128.0) as i32),
        );
    }
}
