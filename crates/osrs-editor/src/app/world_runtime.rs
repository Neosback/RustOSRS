//! Region residency, stream requests, worker communication, and animation scheduling.
//!
//! `WorldRuntime` owns the disposable world-side projection state: which regions are resident,
//! which builds are in flight, which regions or zones wait for a rebuild after an edit, and the
//! animation worker's schedule. It never owns edited map semantics; those live in `EditorSession`.

mod builds;
mod height_dependencies;
mod inspection;
pub(in crate::app) use inspection::CollisionInspection;
mod invalidation;
mod object_output_store;
mod stats;

use self::{
    builds::{BuildTracker, ObjectOutputStamp},
    height_dependencies::{HeightDependencyStamp, HeightDependencyStore},
    object_output_store::ObjectOutputStore,
    stats::RegionStats,
};
use osrs_core::coords::RegionCoord;
use osrs_render::SceneGeometry;
use osrs_world::{
    AnimatedInstance, AnimationWorker, HeightDependencySnapshot, LocEdit, LocInfo, RegionStreamer,
    StreamEvent, TerrainPresentation, TileEdit,
};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::Duration;

/// Regions being built at once.
const MAX_IN_FLIGHT: usize = 2;
/// Animated locs farther than this many tiles from the camera are not drawn.
const ANIMATION_RADIUS_TILES: i32 = 80;
/// Animated geometry is rebuilt at most once per this many game cycles (20 ms each).
const ANIMATION_STRIDE_CYCLES: u64 = 2;
/// How far (tiles) the camera moves before the animated set is re-centred.
const ANIMATION_RECENTRE_TILES: i32 = 8;

type RegionKey = (i32, i32);
pub(super) type ZoneKey = (i32, i32);

/// What part of a resident region the current worker build is intended to refresh.
///
/// The worker always assembles the full contextual scene. `Zones` narrows static extraction and GPU
/// installation while source, picker, animation, and dependency projections remain complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RegionBuildScope {
    Full,
    Zones(BTreeSet<ZoneKey>),
}

/// When animated geometry was last requested, and what forces another request.
#[derive(Default)]
struct AnimationSchedule {
    smoothing: bool,
    frozen: bool,
    cycle: u64,
    center: (i32, i32),
    dirty: bool,
}

pub(super) struct WorldRuntime {
    inspection: inspection::InspectionProjection,
    streamer: RegionStreamer,
    animations: AnimationWorker,
    animation: AnimationSchedule,
    presentation_generation: u32,
    presentation_hd: bool,
    /// Regions resident on the GPU (`None` = the cache has no map data there).
    loaded: HashMap<RegionKey, Option<RegionStats>>,
    /// Projection-only Height dependency data installed from accepted current worker builds.
    height_dependencies: HeightDependencyStore,
    /// Accepted Object producer output plus pending pre/post comparison state.
    object_outputs: ObjectOutputStore,
    /// FIFO request identity and per-region projection epochs for stale-result rejection.
    builds: BuildTracker,
    /// Regions waiting for a conservative full rebuild after an edit.
    dirty_regions: HashSet<RegionKey>,
    /// Fine-grained zones waiting for refresh, grouped by their owning region.
    dirty_zones: HashMap<RegionKey, BTreeSet<ZoneKey>>,
    /// Whether the cache worker has reported ready.
    ready: bool,
}

impl WorldRuntime {
    pub(super) fn set_diagnostics(&mut self, sink: super::diagnostics::DiagnosticSink) {
        self.builds.set_diagnostics(sink);
    }

    pub(super) fn new(streamer: RegionStreamer) -> Self {
        streamer.set_presentation(Self::presentation(false, false), 1);
        Self {
            inspection: inspection::InspectionProjection::default(),
            streamer,
            animations: AnimationWorker::spawn(),
            animation: AnimationSchedule {
                dirty: true,
                ..AnimationSchedule::default()
            },
            presentation_generation: 1,
            presentation_hd: false,
            loaded: HashMap::new(),
            height_dependencies: HeightDependencyStore::default(),
            object_outputs: ObjectOutputStore::default(),
            builds: BuildTracker::default(),
            dirty_regions: HashSet::new(),
            dirty_zones: HashMap::new(),
            ready: false,
        }
    }

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

    // ------------------------------------------------------------------ lifecycle

    pub(super) const fn is_ready(&self) -> bool {
        self.ready
    }

    pub(super) fn mark_ready(&mut self) {
        self.ready = true;
    }

    pub(super) fn spawn_streamer(source: osrs_cache::source::CacheSource) -> RegionStreamer {
        RegionStreamer::spawn(source, Self::presentation(false, false))
    }

    /// Swap in a streamer for a newly opened cache and forget queued edit rebuilds from the old one.
    /// The caller drops the returned streamer off-thread (it finishes its current job first).
    pub(super) fn replace_streamer(&mut self, streamer: RegionStreamer) -> RegionStreamer {
        self.dirty_regions.clear();
        self.dirty_zones.clear();
        self.height_dependencies.clear();
        self.object_outputs.clear();
        self.builds.reset_source();
        self.ready = false;
        std::mem::replace(&mut self.streamer, streamer)
    }

    // ----------------------------------------------------------------- streaming

    pub(super) fn pump(&mut self, budget: Duration) {
        self.streamer.pump(budget);
    }

    pub(super) fn try_recv(&mut self) -> Option<StreamEvent> {
        self.streamer.try_recv()
    }

    /// Whether the active presentation no longer matches the requested HD profile.
    pub(super) const fn hd_mismatch(&self, want_hd: bool) -> bool {
        want_hd != self.presentation_hd
    }

    /// A finished build is current only if it was started under the active presentation.
    pub(super) const fn is_current_generation(&self, generation: u32) -> bool {
        generation == self.presentation_generation
    }

    pub(super) fn set_smooth_terrain(&self, smooth: bool) {
        self.streamer.set_presentation(
            Self::presentation(smooth, self.presentation_hd),
            self.presentation_generation,
        );
    }

    /// Start a new presentation generation and forget every resident region. Returns the keys that
    /// were resident so the caller can drop their render and inspection projections.
    pub(super) fn begin_rebuild_all(&mut self, smooth_terrain: bool, hd: bool) -> Vec<RegionKey> {
        self.presentation_generation += 1;
        self.presentation_hd = hd;
        self.streamer.set_presentation(
            Self::presentation(smooth_terrain, hd),
            self.presentation_generation,
        );
        let keys: Vec<_> = self.loaded.keys().copied().collect();
        for &key in &keys {
            self.animations.remove_region(key);
        }
        self.loaded.clear();
        self.height_dependencies.clear();
        self.object_outputs.clear();
        self.builds.cancel_all();
        self.dirty_regions.clear();
        self.dirty_zones.clear();
        self.animation.dirty = true;
        keys
    }

    /// One accepted current region build finished; static status accounting follows the resolved
    /// full/zone install scope while animation and dependency state are updated from the accepted
    /// contextual worker result.
    pub(super) fn region_installed(
        &mut self,
        key: RegionKey,
        scope: &RegionBuildScope,
        geometry: &SceneGeometry,
        dependencies: &HeightDependencySnapshot,
        animations: Vec<AnimatedInstance>,
    ) {
        self.animations.insert_region(key, animations);
        let projection_epoch = self.builds.projection_epoch(key);
        let height_stamp =
            HeightDependencyStamp::new(projection_epoch, self.presentation_generation);
        let _ = self
            .height_dependencies
            .install(key, height_stamp, dependencies);
        self.object_outputs.install(
            key,
            ObjectOutputStamp::new(projection_epoch, self.presentation_generation),
            scope,
            geometry,
        );
        self.animation.dirty = true;
        match scope {
            RegionBuildScope::Full => {
                self.loaded
                    .insert(key, Some(RegionStats::from_geometry(geometry)));
            }
            RegionBuildScope::Zones(_) => {
                let slot = self
                    .loaded
                    .entry(key)
                    .or_insert_with(|| Some(RegionStats::default()));
                let stats = slot.get_or_insert_with(RegionStats::default);
                stats.refresh(scope, geometry);
            }
        }
    }

    /// Mark a current build as absent. Stale completions are filtered by [`Self::build_finished`]
    /// before this method is called, so they cannot overwrite newer residency state.
    pub(super) fn region_absent(&mut self, key: RegionKey, scope: &RegionBuildScope) {
        if matches!(scope, RegionBuildScope::Full) {
            self.loaded.insert(key, None);
            self.height_dependencies.remove(key);
            self.object_outputs.remove(key);
        }
    }

    /// Match one FIFO Region/Empty/Failed event to its editor-side build ticket.
    ///
    /// Returns `None` when the request was cancelled/replaced, a newer edit advanced the region's
    /// projection epoch, or the request belongs to an older presentation generation.
    pub(super) fn build_finished(&mut self, key: RegionKey) -> Option<RegionBuildScope> {
        self.builds.finish(key, self.presentation_generation)
    }

    /// Resolve one accepted worker result into the static install scope. Normal builds keep their
    /// requested scope; a full build queued as an Object proof narrows to the exact changed zones
    /// only when the retained pre-edit baseline is still exact and complete.
    pub(super) fn resolve_install_scope(
        &mut self,
        key: RegionKey,
        requested: RegionBuildScope,
        geometry: &SceneGeometry,
    ) -> RegionBuildScope {
        self.object_outputs.resolve_scope(key, requested, geometry)
    }

    /// Request the nearest regions around `center` that are neither resident nor in flight.
    pub(super) fn request_missing(&mut self, center: RegionKey, radius: i32) {
        let wanted = wanted_regions(center, radius);
        for key in wanted {
            if self.builds.len() >= MAX_IN_FLIGHT {
                break;
            }
            if !self.loaded.contains_key(&key) && !self.builds.contains(&key) {
                self.builds
                    .request(key, RegionBuildScope::Full, self.presentation_generation);
                self.streamer.request(RegionCoord::new(key.0, key.1));
            }
        }
    }

    /// Drop regions farther than `radius + 1` from `center`; returns them so the caller can
    /// unload their semantic, render, and inspection projections.
    pub(super) fn unload_stale(&mut self, center: RegionKey, radius: i32) -> Vec<RegionKey> {
        let stale = stale_regions(self.loaded.keys().copied(), center, radius);
        for &key in &stale {
            self.loaded.remove(&key);
            self.height_dependencies.remove(key);
            self.object_outputs.remove(key);
            self.builds.cancel(key);
            self.dirty_regions.remove(&key);
            self.dirty_zones.remove(&key);
            self.animations.remove_region(key);
            self.animation.dirty = true;
        }
        stale
    }

    pub(super) fn request_outline(&self, region: RegionKey, index: usize, loc: LocInfo) {
        self.streamer.request_outline(region, index, loc);
    }

    // ------------------------------------------------------- edit projection mirror

    /// Resident regions with a loaded (non-empty) build.
    pub(super) fn resident_regions(&self) -> usize {
        self.loaded.values().flatten().count()
    }

    pub(super) fn resident_vertices(&self) -> usize {
        self.loaded
            .values()
            .flatten()
            .map(RegionStats::vertices)
            .sum()
    }

    pub(super) fn in_flight_count(&self) -> usize {
        self.builds.len()
    }

    /// Mirror live/legacy tile edits and derive rebuild scope from their adapter coordinates.
    pub(super) fn mirror_tile_edits(
        &mut self,
        edits: Vec<TileEdit>,
        affected: impl Fn(&TileEdit) -> Vec<RegionKey>,
    ) {
        for edit in &edits {
            for region in affected(edit) {
                self.mark_dirty(region);
            }
        }
        self.project_tile_edits(edits);
    }

    /// Mirror live/legacy loc edits and derive rebuild scope from their adapter coordinates.
    pub(super) fn mirror_loc_edits(&mut self, edits: Vec<LocEdit>) {
        for edit in &edits {
            let (LocEdit::Remove(spec) | LocEdit::Add(spec)) = edit;
            self.mark_dirty(spec.region());
        }
        self.project_loc_edits(edits);
    }

    /// Mirror already-finalized semantic tile edits without deriving invalidation from legacy data.
    pub(super) fn project_tile_edits(&self, edits: Vec<TileEdit>) {
        if !edits.is_empty() {
            self.streamer.edit(edits);
        }
    }

    /// Mirror already-finalized semantic loc edits without deriving invalidation from legacy data.
    pub(super) fn project_loc_edits(&self, edits: Vec<LocEdit>) {
        if !edits.is_empty() {
            self.streamer.edit_locs(edits);
        }
    }

    /// Request rebuild work that is not already in flight. Full refreshes request the whole static
    /// region; zone refreshes carry their exact zone set through worker extraction and GPU install.
    pub(super) fn flush_dirty_regions(&mut self) {
        let full_ready: Vec<RegionKey> = self
            .dirty_regions
            .iter()
            .copied()
            .filter(|key| !self.builds.contains(key))
            .collect();
        for key in full_ready {
            if self.builds.len() >= MAX_IN_FLIGHT {
                break;
            }
            self.dirty_regions.remove(&key);
            self.dirty_zones.remove(&key);
            self.builds
                .request(key, RegionBuildScope::Full, self.presentation_generation);
            self.streamer.request(RegionCoord::new(key.0, key.1));
        }

        let zone_ready: Vec<RegionKey> = self
            .dirty_zones
            .keys()
            .copied()
            .filter(|key| !self.dirty_regions.contains(key) && !self.builds.contains(key))
            .collect();
        for key in zone_ready {
            if self.builds.len() >= MAX_IN_FLIGHT {
                break;
            }
            let Some(zones) = self.dirty_zones.remove(&key) else {
                continue;
            };
            if zones.is_empty() {
                continue;
            }
            let requested = zones.clone();
            self.builds.request(
                key,
                RegionBuildScope::Zones(zones),
                self.presentation_generation,
            );
            self.streamer
                .request_zones(RegionCoord::new(key.0, key.1), requested);
        }
    }

    // ----------------------------------------------------------------- animation

    pub(super) fn mark_animation_dirty(&mut self) {
        self.animation.dirty = true;
    }

    pub(super) fn set_animation_smoothing(&mut self, enabled: bool) {
        self.animation.smoothing = enabled;
        self.animations.set_smoothing(enabled);
    }

    /// Geometry the animation worker finished since the last poll.
    pub(super) fn poll_animation_geometry(&mut self) -> Option<(SceneGeometry, f32)> {
        let update = self.animations.poll()?;
        update
            .geometry
            .map(|geometry| (geometry, update.rebuild_ms))
    }

    /// Request the next animation step if the schedule calls for one.
    pub(super) fn schedule_animation(
        &mut self,
        animate_objects: bool,
        elapsed_secs: f32,
        camera_tile: (i32, i32),
    ) {
        let frozen = !animate_objects;
        if frozen != self.animation.frozen {
            self.animation.frozen = frozen;
            self.animations.set_frozen(frozen);
            self.animation.dirty = true;
        }
        if !self.animations.is_idle() {
            return;
        }
        let cycle = (elapsed_secs * 50.0) as u64;
        let moved = (camera_tile.0 - self.animation.center.0)
            .abs()
            .max((camera_tile.1 - self.animation.center.1).abs())
            >= ANIMATION_RECENTRE_TILES;
        let stride = if self.animation.smoothing {
            1
        } else {
            ANIMATION_STRIDE_CYCLES
        };
        if (frozen || cycle < self.animation.cycle + stride) && !self.animation.dirty && !moved {
            return;
        }
        let delta = (cycle - self.animation.cycle).min(500) as i32;
        self.animation.cycle = cycle;
        if self.animation.dirty || moved {
            self.animation.dirty = false;
            self.animation.center = camera_tile;
            self.animations.force_rebuild();
        }
        self.animations
            .request(delta, self.animation.center, ANIMATION_RADIUS_TILES);
    }
}

/// Regions within `radius` of `center`, nearest first (ring distance, then Manhattan distance).
fn wanted_regions(center: RegionKey, radius: i32) -> Vec<RegionKey> {
    let (cx, cy) = center;
    let mut wanted: Vec<RegionKey> = Vec::new();
    for dx in -radius..=radius {
        for dy in -radius..=radius {
            wanted.push((cx + dx, cy + dy));
        }
    }
    wanted.sort_by_key(|&(x, y)| {
        (x - cx).abs().max((y - cy).abs()) * 100 + (x - cx).abs() + (y - cy).abs()
    });
    wanted
}

/// Resident regions beyond one ring of hysteresis past the load radius.
fn stale_regions(
    loaded: impl Iterator<Item = RegionKey>,
    center: RegionKey,
    radius: i32,
) -> Vec<RegionKey> {
    let (cx, cy) = center;
    let keep = radius + 1;
    loaded
        .filter(|&(x, y)| (x - cx).abs().max((y - cy).abs()) > keep)
        .collect()
}

#[cfg(test)]
mod tests;
