//! Region streaming and animation upkeep: what loads, what rebuilds, what animates.

use super::*;

impl EditorApp {
    pub(super) fn presentation(smooth_terrain: bool, hd_geometry: bool) -> TerrainPresentation {
        TerrainPresentation {
            smooth_terrain,
            hd_geometry,
            // Always on, no switch: lifts diagonal wall-decoration plates off their wall so the
            // depth buffer does not hide their recessed faces (the software client painted
            // decorations over their wall regardless of depth).
            flush_diagonal_decorations: true,
            ..TerrainPresentation::default()
        }
    }

    /// Rebuild every resident region (after a presentation change).
    pub(super) fn rebuild_all(&mut self) {
        self.presentation_generation += 1;
        self.presentation_hd = self.prefs.render_profile == ProfileChoice::Hd;
        self.streamer.set_presentation(
            Self::presentation(self.prefs.smooth_terrain, self.presentation_hd),
            self.presentation_generation,
        );
        for key in self.loaded.keys().copied().collect::<Vec<_>>() {
            self.renderer.remove_region(key);
            self.animations.remove_region(key);
            self.infos.remove(&key);
            self.infos_version += 1;
        }
        self.loaded.clear();
        self.outlines.clear();
        self.outline_requested.clear();
        self.clear_selection();
        self.in_flight.clear();
        self.animation_dirty = true;
    }

    /// Apply finished builds, then request and unload regions around the camera.
    pub(super) fn stream(&mut self, ctx: &egui::Context) {
        // The HD profile needs geometry with the HD vertex stream: rebuild when it is switched.
        if (self.prefs.render_profile == ProfileChoice::Hd) != self.presentation_hd {
            self.rebuild_all();
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.poll_web_pick(ctx);
            self.run_palette_job();
        }
        // Browser: no worker threads, so run the streamer's queue for a slice of this frame.
        self.streamer.pump(std::time::Duration::from_millis(8));
        while let Some(event) = self.streamer.try_recv() {
            match event {
                StreamEvent::HdMaterials(data) => {
                    self.hd.set_environments(Arc::clone(&data.environments));
                    if let Err(error) = data.install(&mut self.renderer) {
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
                    self.cache_opened(ctx, profile_id, build);
                    self.renderer.set_textures(&textures);
                    self.floor_textures = Some(textures);
                    self.ready = true;
                }
                StreamEvent::Region(region) => {
                    let key = (region.region.x, region.region.y);
                    if region.generation != self.presentation_generation {
                        continue;
                    }
                    self.in_flight.remove(&key);
                    self.renderer.upload_region(key, &region.geometry);
                    self.animations.insert_region(key, region.animations);
                    self.infos.insert(key, region.info);
                    self.infos_version += 1;
                    self.animation_dirty = true;
                    self.last_build_ms = region.build_ms;
                    self.loaded.insert(
                        key,
                        Some(RegionStats {
                            vertices: region.geometry.vertex_count(),
                        }),
                    );
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
                    self.in_flight.remove(&key);
                    self.loaded.insert(key, None);
                }
                StreamEvent::Failed(region, error) => {
                    self.in_flight.remove(&(region.x, region.y));
                    self.loaded.insert((region.x, region.y), None);
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
        if !self.ready {
            return;
        }
        self.flush_dirty_regions();

        let (cx, cy) = self.camera_region();
        let radius = self.prefs.load_radius;
        let mut wanted: Vec<(i32, i32)> = Vec::new();
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                wanted.push((cx + dx, cy + dy));
            }
        }
        wanted.sort_by_key(|&(x, y)| {
            (x - cx).abs().max((y - cy).abs()) * 100 + (x - cx).abs() + (y - cy).abs()
        });
        for key in wanted {
            if self.in_flight.len() >= MAX_IN_FLIGHT {
                break;
            }
            if !self.loaded.contains_key(&key) && !self.in_flight.contains(&key) {
                self.in_flight.insert(key);
                self.streamer.request(RegionCoord::new(key.0, key.1));
            }
        }

        let keep = radius + 1;
        let stale: Vec<(i32, i32)> = self
            .loaded
            .keys()
            .copied()
            .filter(|&(x, y)| (x - cx).abs().max((y - cy).abs()) > keep)
            .collect();
        for key in stale {
            self.loaded.remove(&key);
            self.renderer.remove_region(key);
            self.animations.remove_region(key);
            self.infos.remove(&key);
            self.infos_version += 1;
            self.animation_dirty = true;
        }
    }

    /// Advance animated locs to the current game cycle on the animation worker and upload the
    /// geometry it finishes.
    pub(super) fn update_animations(&mut self) {
        if let Some(update) = self.animations.poll()
            && let Some(geometry) = update.geometry
        {
            let phase = Instant::now();
            self.renderer.upload_region(ANIMATED_KEY, &geometry);
            self.stats.phase("anim-upload", phase);
        }
        let frozen = !self.prefs.animate_objects;
        if frozen != self.animations_frozen {
            self.animations_frozen = frozen;
            self.animations.set_frozen(frozen);
            self.animation_dirty = true;
        }
        if !self.animations.is_idle() {
            return;
        }
        let cycle = (self.started.elapsed().as_secs_f32() * 50.0) as u64;
        let (world_x, world_z) = self.world_position();
        let center = ((world_x / 128.0) as i32, (world_z / 128.0) as i32);
        let moved = (center.0 - self.animation_center.0)
            .abs()
            .max((center.1 - self.animation_center.1).abs())
            >= 8;
        // Smoothing blends inside a frame, so it needs every game cycle.
        let stride = if self.animation_smoothing {
            1
        } else {
            ANIMATION_STRIDE_CYCLES
        };
        if (frozen || cycle < self.animation_cycle + stride) && !self.animation_dirty && !moved {
            return;
        }
        let delta = (cycle - self.animation_cycle).min(500) as i32;
        self.animation_cycle = cycle;
        if self.animation_dirty || moved {
            self.animation_dirty = false;
            self.animation_center = center;
            self.animations.force_rebuild();
        }
        self.animations
            .request(delta, self.animation_center, ANIMATION_RADIUS_TILES);
    }
}
