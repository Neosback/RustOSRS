//! Region residency, stream requests, worker communication, and animation scheduling.
//!
//! `WorldRuntime` owns the disposable world-side projection state: which regions are resident,
//! which builds are in flight, which regions or zones wait for a rebuild after an edit, and the
//! animation worker's schedule. It never owns edited map semantics; those live in `EditorSession`.

mod builds;
mod height_dependencies;
mod invalidation;
mod stats;

use self::{
    builds::BuildTracker,
    height_dependencies::{
        HeightDependencyCollection, HeightDependencyLookup, HeightDependencyStamp,
    },
    invalidation::{ProjectionInvalidation, projection_invalidation, zone_region},
    stats::RegionStats,
};
use super::edit_runtime::EditTouches;
use osrs_core::coords::RegionCoord;
use osrs_edit::{ChangeImpact, HeightVertexCoord};
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
/// installation while source, picker, animation, and Height-dependency projections remain complete.
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
    streamer: RegionStreamer,
    animations: AnimationWorker,
    animation: AnimationSchedule,
    presentation_generation: u32,
    presentation_hd: bool,
    /// Regions resident on the GPU (`None` = the cache has no map data there).
    loaded: HashMap<RegionKey, Option<RegionStats>>,
    /// Projection-only Height dependency data installed from accepted current worker builds.
    height_dependencies: HashMap<RegionKey, HeightDependencyCollection>,
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
    pub(super) fn new(streamer: RegionStreamer) -> Self {
        streamer.set_presentation(Self::presentation(false, false), 1);
        Self {
            streamer,
            animations: AnimationWorker::spawn(),
            animation: AnimationSchedule {
                dirty: true,
                ..AnimationSchedule::default()
            },
            presentation_generation: 1,
            presentation_hd: false,
            loaded: HashMap::new(),
            height_dependencies: HashMap::new(),
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
        self.builds.cancel_all();
        self.dirty_regions.clear();
        self.dirty_zones.clear();
        self.animation.dirty = true;
        keys
    }

    /// One accepted current region build finished; static status accounting follows the same
    /// full/zone scope used by the renderer while animation and Height dependency state are replaced
    /// from the complete contextual worker result.
    pub(super) fn region_installed(
        &mut self,
        key: RegionKey,
        scope: &RegionBuildScope,
        geometry: &SceneGeometry,
        dependencies: &HeightDependencySnapshot,
        animations: Vec<AnimatedInstance>,
    ) {
        self.animations.insert_region(key, animations);
        self.install_height_dependencies(key, dependencies);
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

    fn install_height_dependencies(
        &mut self,
        key: RegionKey,
        snapshot: &HeightDependencySnapshot,
    ) {
        let producer = RegionCoord::new(key.0, key.1);
        let stamp = HeightDependencyStamp::new(
            self.builds.projection_epoch(key),
            self.presentation_generation,
        );
        let mut dependencies = HeightDependencyCollection::new(producer, stamp);
        for (source, zones) in &snapshot.by_source {
            let source = HeightVertexCoord::new(source.corner, source.plane);
            for &zone in zones {
                if dependencies.record(source, zone).is_err() {
                    self.height_dependencies.remove(&key);
                    return;
                }
            }
        }
        if snapshot.complete {
            dependencies.mark_complete();
        }
        self.height_dependencies.insert(key, dependencies);
    }

    /// Mark a current build as absent. Stale completions are filtered by [`Self::build_finished`]
    /// before this method is called, so they cannot overwrite newer residency state.
    pub(super) fn region_absent(&mut self, key: RegionKey, scope: &RegionBuildScope) {
        if matches!(scope, RegionBuildScope::Full) {
            self.loaded.insert(key, None);
            self.height_dependencies.remove(&key);
        }
    }

    /// Match one FIFO Region/Empty/Failed event to its editor-side build ticket.
    ///
    /// Returns `None` when the request was cancelled/replaced, a newer edit advanced the region's
    /// projection epoch, or the request belongs to an older presentation generation.
    pub(super) fn build_finished(&mut self, key: RegionKey) -> Option<RegionBuildScope> {
        self.builds.finish(key, self.presentation_generation)
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
            self.height_dependencies.remove(&key);
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

    /// Translate one finalized semantic touch set into projection invalidation.
    ///
    /// Height edits use exact world zones only when every resident non-empty producer region has a
    /// current, complete dependency snapshot. Missing, stale, or incomplete metadata fails closed
    /// to the previous region policy. Object edits remain region-conservative until their separate
    /// dependency subsystem exists.
    pub(super) fn invalidate_semantic_edit(
        &mut self,
        impact: ChangeImpact,
        touched_regions: &EditTouches,
    ) {
        let proven_height_zones = if touched_regions.footprint.height_vertices.is_empty() {
            None
        } else {
            self.proven_height_zones(&touched_regions.footprint.height_vertices)
        };
        match projection_invalidation(
            impact,
            touched_regions,
            &touched_regions.footprint,
            proven_height_zones.as_ref(),
        ) {
            ProjectionInvalidation::None => {}
            ProjectionInvalidation::Regions(regions) => {
                for region in regions {
                    self.mark_dirty((region.x, region.y));
                }
            }
            ProjectionInvalidation::Zones(zones) => {
                for zone in zones {
                    self.mark_zone_dirty(zone);
                }
            }
        }
    }

    /// Resolve source Height vertices through every resident producer. Any producer without a
    /// current complete read-set makes the whole transaction fall back to region invalidation.
    fn proven_height_zones(
        &self,
        sources: &[HeightVertexCoord],
    ) -> Option<BTreeSet<ZoneKey>> {
        let mut zones = BTreeSet::new();
        for (&key, stats) in &self.loaded {
            if stats.is_none() {
                continue;
            }
            let dependencies = self.height_dependencies.get(&key)?;
            let current = HeightDependencyStamp::new(
                self.builds.projection_epoch(key),
                self.presentation_generation,
            );
            for &source in sources {
                match dependencies.lookup(source, current) {
                    HeightDependencyLookup::Current(Some(found)) => {
                        zones.extend(found.iter().copied());
                    }
                    HeightDependencyLookup::Current(None) => {}
                    HeightDependencyLookup::Stale | HeightDependencyLookup::Incomplete => {
                        return None;
                    }
                }
            }
        }
        Some(zones)
    }

    /// Queue a full `region` rebuild if it is resident. Full invalidation subsumes queued zones.
    pub(super) fn mark_dirty(&mut self, region: RegionKey) {
        if self.loaded.contains_key(&region) {
            self.builds.invalidate(region);
            self.dirty_zones.remove(&region);
            self.dirty_regions.insert(region);
        }
    }

    /// Queue one world 8x8 zone when its owning region is resident and no full rebuild supersedes it.
    fn mark_zone_dirty(&mut self, zone: ZoneKey) {
        let region = zone_region(zone);
        if self.dirty_regions.contains(&region)
            || !matches!(self.loaded.get(&region), Some(Some(_)))
        {
            return;
        }
        self.builds.invalidate(region);
        self.dirty_zones.entry(region).or_default().insert(zone);
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
    pub(super) fn poll_animation_geometry(&mut self) -> Option<SceneGeometry> {
        self.animations.poll().and_then(|update| update.geometry)
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
        // Smoothing blends inside a frame, so it needs every game cycle.
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
