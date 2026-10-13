//! Region streaming and animation upkeep: what loads, what rebuilds, what animates.

use super::*;
use osrs_world::StreamEvent;

impl EditorApp {
    /// Rebuild every resident region (after a presentation change).
    pub(super) fn rebuild_all(&mut self) {
        let hd = self.prefs.render_profile == ProfileChoice::Hd;
        for key in self.world.begin_rebuild_all(self.prefs.smooth_terrain, hd) {
            self.renderer.remove_region(key);
            self.infos.remove(&key);
            self.infos_version += 1;
        }
        self.outlines.clear();
        self.outline_requested.clear();
        self.clear_selection();
    }

    /// Apply finished builds, then request and unload regions around the camera.
    pub(super) fn stream(&mut self, ctx: &egui::Context) {
        // The HD profile needs geometry with the HD vertex stream: rebuild when it is switched.
        if self
            .world
            .hd_mismatch(self.prefs.render_profile == ProfileChoice::Hd)
        {
            self.rebuild_all();
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.poll_web_pick(ctx);
            self.run_palette_job();
        }
        // Browser: no worker threads, so run the streamer's queue for a slice of this frame.
        self.world.pump(std::time::Duration::from_millis(8));
        while let Some(event) = self.world.try_recv() {
            match event {
                StreamEvent::HdMaterials(data) => {
                    self.hd.set_environments(Arc::clone(&data.environments));
                    if let Err(error) = self.renderer.install_hd_materials(*data) {
                        self.status = Some(format!("HD materials failed: {error}"));
                    }
                }
                StreamEvent::HdFailed(error) => {
                    self.status = Some(format!("117 HD data failed to load: {error}"));
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
                    self.floor_textures = Some(textures);
                    self.world.mark_ready();
                }
                StreamEvent::Region(region) => {
                    let key = (region.region.x, region.region.y);
                    if !self.world.is_current_generation(region.generation) {
                        continue;
                    }
                    self.world.build_finished(key);
                    match crate::session::adapt_region_map(&region.source) {
                        Ok(snapshot) => {
                            let _ = self.session.ingest_region(&snapshot);
                        }
                        Err(error) => {
                            self.status = Some(format!(
                                "semantic region {},{} failed: {error:?}",
                                region.region.x, region.region.y
                            ));
                        }
                    }
                    // Object indices in a rebuilt region are new (an edit shifts them), so what was
                    // outlined for the old ones is gone: the selection's highlight is made afresh.
                    self.outlines.retain(|(r, _), _| *r != key);
                    self.outline_requested.retain(|(r, _)| *r != key);
                    self.renderer.upload_region(key, &region.geometry);
                    self.infos.insert(key, region.info);
                    self.infos_version += 1;
                    self.world
                        .region_installed(key, &region.geometry, region.animations);
                }
                StreamEvent::Outline {
                    region,
                    index,
                    triangles,
                } => {
                    self.outlines.insert((region, index), Arc::new(triangles));
                }
                StreamEvent::Empty(region) => {
                    let key = (region.x, region.y);
                    let _ = self.session.unload_region(region);
                    self.world.region_absent(key);
                }
                StreamEvent::Failed(region, error) => {
                    self.world.region_absent((region.x, region.y));
                    self.status = Some(format!("region {},{} failed: {error}", region.x, region.y));
                }
                StreamEvent::InitFailed(_) if self.cache_dir.as_os_str().is_empty() => {}
                StreamEvent::InitFailed(error) => {
                    // The card explains it; the status bar stays quiet.
                    self.cache_error = Some(if error.contains("No such file") {
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
        self.world.flush_dirty_regions();

        let center = self.camera_region();
        let radius = self.prefs.load_radius;
        self.world.request_missing(center, radius);
        for key in self.world.unload_stale(center, radius) {
            let _ = self.session.unload_region(RegionCoord::new(key.0, key.1));
            self.renderer.remove_region(key);
            self.infos.remove(&key);
            self.infos_version += 1;
        }
    }

    /// Advance animated locs to the current game cycle on the animation worker and upload the
    /// geometry it finishes.
    pub(super) fn update_animations(&mut self) {
        if let Some(geometry) = self.world.poll_animation_geometry() {
            let phase = Instant::now();
            self.renderer.upload_region(ANIMATED_KEY, &geometry);
            self.stats.phase("anim-upload", phase);
        }
        let (world_x, world_z) = self.world_position();
        self.world.schedule_animation(
            self.prefs.animate_objects,
            self.started.elapsed().as_secs_f32(),
            ((world_x / 128.0) as i32, (world_z / 128.0) as i32),
        );
    }
}
