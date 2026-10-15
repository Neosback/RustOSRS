//! Region streaming and animation upkeep: what loads, what rebuilds, what animates.

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
                    if let Err(error) = self.renderer.install_hd_materials(*data) {
                        self.ui.set_status(format!("HD materials failed: {error}"));
                    }
                }
                StreamEvent::HdFailed(error) => {
                    self.ui
                        .set_status(format!("117 HD data failed to load: {error}"));
                }
                StreamEvent::Ready {
                    textures,
                    profile_id,
                    build,
                } => {
                    // `Ready` is emitted exactly once by each newly opened cache worker. Visual
                    // presentation rebuilds do not emit it, so this is the cache/source boundary
                    // where old semantic state must be discarded.
                    self.session.reset();
                    self.cache_opened(ctx, profile_id, build);
                    self.renderer.set_textures(&textures);
                    self.palettes.set_textures(textures);
                    self.world.mark_ready();
                }
                StreamEvent::Region(region) => {
                    self.diagnostics.build(region.build_ms);
                    let key = (region.region.x, region.region.y);
                    // Retire the FIFO build ticket before checking worker presentation generation.
                    // Old results must consume their own ticket so a later request for the same
                    // region cannot be mistaken for this completion.
                    let scope = self.world.build_finished(key);
                    if !self.world.is_current_generation(region.generation) {
                        continue;
                    }
                    let Some(scope) = scope else {
                        continue;
                    };
                    // Accepted Object proof builds are full contextual/static worker results. Only
                    // after the existing request/generation gates pass do we compare them with the
                    // retained pre-edit producer output and possibly narrow the static install.
                    let scope = self
                        .world
                        .resolve_install_scope(key, scope, &region.geometry);
                    match crate::session::adapt_region_map(&region.source) {
                        Ok(snapshot) => {
                            let _ = self.session.ingest_region(&snapshot);
                        }
                        Err(error) => {
                            self.ui.set_status(format!(
                                "semantic region {},{} failed: {error:?}",
                                region.region.x, region.region.y
                            ));
                        }
                    }
                    match &scope {
                        world_runtime::RegionBuildScope::Full => {
                            self.renderer.upload_region(key, &region.geometry);
                        }
                        world_runtime::RegionBuildScope::Zones(zones) => {
                            self.renderer
                                .upload_region_zones(key, &region.geometry, zones);
                        }
                    }
                    // Installing new picker data also invalidates outline indices from the old build.
                    self.world.install_info(key, region.info);
                    self.world.region_installed(
                        key,
                        &scope,
                        &region.geometry,
                        &region.height_dependencies,
                        region.animations,
                    );
                    if matches!(scope, world_runtime::RegionBuildScope::Full) {
                        self.apply_project_region(RegionCoord::new(key.0, key.1));
                    }
                }
                StreamEvent::Outline {
                    region,
                    index,
                    triangles,
                } => {
                    self.world.install_outline((region, index), triangles);
                }
                StreamEvent::Empty(region) => {
                    let key = (region.x, region.y);
                    let Some(scope) = self.world.build_finished(key) else {
                        continue;
                    };
                    self.world.region_absent(key, &scope);
                    if matches!(scope, world_runtime::RegionBuildScope::Full) {
                        let _ = self.session.unload_region(region);
                    }
                }
                StreamEvent::Failed(region, error) => {
                    let key = (region.x, region.y);
                    let Some(scope) = self.world.build_finished(key) else {
                        continue;
                    };
                    self.world.region_absent(key, &scope);
                    self.ui
                        .set_status(format!("region {},{} failed: {error}", region.x, region.y));
                }
                StreamEvent::InitFailed(_) if self.project.cache_label().as_os_str().is_empty() => {
                }
                StreamEvent::InitFailed(error) => {
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

        let center = self.camera_region();
        let radius = self.settings.prefs.load_radius;
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
        if let Some(geometry) = self.world.poll_animation_geometry() {
            let phase = Instant::now();
            self.renderer.upload_transient(ANIMATED_KEY, &geometry);
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
