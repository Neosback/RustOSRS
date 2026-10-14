//! Region residency, stream requests, worker communication, and animation scheduling.
//!
//! `WorldRuntime` owns the disposable world-side projection state: which regions are resident,
//! which builds are in flight, which regions wait for a rebuild after an edit, and the animation
//! worker's schedule. It never owns edited map semantics; those live in `EditorSession`.

use osrs_core::coords::RegionCoord;
use osrs_edit::ChangeImpact;
use osrs_render::SceneGeometry;
use osrs_world::{
    AnimatedInstance, AnimationWorker, LocEdit, LocInfo, RegionStreamer, StreamEvent,
    TerrainPresentation, TileEdit,
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

/// What the status bar needs to know about a resident region.
#[derive(Clone, Copy)]
pub(super) struct RegionStats {
    pub(super) vertices: usize,
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
    loaded: HashMap<(i32, i32), Option<RegionStats>>,
    in_flight: HashSet<(i32, i32)>,
    /// Regions waiting for a rebuild after an edit.
    dirty_regions: HashSet<(i32, i32)>,
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
            in_flight: HashSet::new(),
            dirty_regions: HashSet::new(),
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

    /// Swap in a streamer for a newly opened cache and forget everything tied to the old one.
    /// The caller drops the returned streamer off-thread (it finishes its current job first).
    pub(super) fn replace_streamer(&mut self, streamer: RegionStreamer) -> RegionStreamer {
        self.dirty_regions.clear();
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
    pub(super) fn begin_rebuild_all(&mut self, smooth_terrain: bool, hd: bool) -> Vec<(i32, i32)> {
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
        self.in_flight.clear();
        self.animation.dirty = true;
        keys
    }

    /// A region build finished and its geometry is on the GPU.
    pub(super) fn region_installed(
        &mut self,
        key: (i32, i32),
        geometry: &SceneGeometry,
        animations: Vec<AnimatedInstance>,
    ) {
        self.animations.insert_region(key, animations);
        self.animation.dirty = true;
        self.loaded.insert(
            key,
            Some(RegionStats {
                vertices: geometry.vertex_count(),
            }),
        );
    }

    /// The cache has no map data at `key` (or the build failed).
    pub(super) fn region_absent(&mut self, key: (i32, i32)) {
        self.in_flight.remove(&key);
        self.loaded.insert(key, None);
    }

    pub(super) fn build_finished(&mut self, key: (i32, i32)) {
        self.in_flight.remove(&key);
    }

    /// Request the nearest regions around `center` that are neither resident nor in flight.
    pub(super) fn request_missing(&mut self, center: (i32, i32), radius: i32) {
        let wanted = wanted_regions(center, radius);
        for key in wanted {
            if self.in_flight.len() >= MAX_IN_FLIGHT {
                break;
            }
            if !self.loaded.contains_key(&key) && !self.in_flight.contains(&key) {
                self.in_flight.insert(key);
                self.streamer.request(RegionCoord::new(key.0, key.1));
            }
        }
    }

    /// Drop regions farther than `radius + 1` from `center`; returns them so the caller can
    /// unload their semantic, render, and inspection projections.
    pub(super) fn unload_stale(&mut self, center: (i32, i32), radius: i32) -> Vec<(i32, i32)> {
        let stale = stale_regions(self.loaded.keys().copied(), center, radius);
        for &key in &stale {
            self.loaded.remove(&key);
            self.animations.remove_region(key);
            self.animation.dirty = true;
        }
        stale
    }

    pub(super) fn request_outline(&self, region: (i32, i32), index: usize, loc: LocInfo) {
        self.streamer.request_outline(region, index, loc);
    }

    // ------------------------------------------------------- edit projection mirror

    /// Resident regions with a loaded (non-empty) build.
    pub(super) fn resident_regions(&self) -> usize {
        self.loaded.values().flatten().count()
    }

    pub(super) fn resident_vertices(&self) -> usize {
        self.loaded.values().flatten().map(|s| s.vertices).sum()
    }

    pub(super) fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    /// Mirror live/legacy tile edits and derive rebuild scope from their adapter coordinates.
    pub(super) fn mirror_tile_edits(
        &mut self,
        edits: Vec<TileEdit>,
        affected: impl Fn(&TileEdit) -> Vec<(i32, i32)>,
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

    /// Translate one finalized semantic edit into region-level projection invalidation.
    ///
    /// Object-only changes keep the historical behavior of rebuilding the touched regions. Terrain
    /// geometry, materials, normals, or lighting can affect derived data across a region boundary,
    /// so the current region-level implementation conservatively rebuilds one neighboring ring.
    /// A later zone/chunk checkpoint can tighten this without changing semantic edit metadata.
    pub(super) fn invalidate_semantic_edit(
        &mut self,
        impact: ChangeImpact,
        touched_regions: &[RegionCoord],
    ) {
        for region in invalidated_regions(impact, touched_regions) {
            self.mark_dirty((region.x, region.y));
        }
    }

    /// Queue `region` for rebuild if it is resident.
    pub(super) fn mark_dirty(&mut self, region: (i32, i32)) {
        if self.loaded.contains_key(&region) {
            self.dirty_regions.insert(region);
        }
    }

    /// Request a rebuild of every dirty region that is not already being built; the old geometry
    /// stays on screen until the new one arrives.
    pub(super) fn flush_dirty_regions(&mut self) {
        let ready: Vec<(i32, i32)> = self
            .dirty_regions
            .iter()
            .copied()
            .filter(|key| !self.in_flight.contains(key))
            .collect();
        for key in ready {
            self.dirty_regions.remove(&key);
            self.in_flight.insert(key);
            self.streamer.request(RegionCoord::new(key.0, key.1));
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

/// Region-level projection invalidation derived only from authoritative semantic edit metadata.
fn invalidated_regions(impact: ChangeImpact, touched_regions: &[RegionCoord]) -> Vec<RegionCoord> {
    if impact.is_empty() || touched_regions.is_empty() {
        return Vec::new();
    }

    let crosses_region_boundary = impact.intersects(ChangeImpact::TERRAIN_GEOMETRY)
        || impact.intersects(ChangeImpact::TERRAIN_MATERIAL)
        || impact.intersects(ChangeImpact::NORMALS)
        || impact.intersects(ChangeImpact::LIGHTING);
    let reach = if crosses_region_boundary { 1 } else { 0 };
    let mut regions = BTreeSet::new();
    for region in touched_regions {
        for dx in -reach..=reach {
            for dy in -reach..=reach {
                regions.insert(RegionCoord::new(region.x + dx, region.y + dy));
            }
        }
    }
    regions.into_iter().collect()
}

/// Regions within `radius` of `center`, nearest first (ring distance, then Manhattan distance).
fn wanted_regions(center: (i32, i32), radius: i32) -> Vec<(i32, i32)> {
    let (cx, cy) = center;
    let mut wanted: Vec<(i32, i32)> = Vec::new();
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
    loaded: impl Iterator<Item = (i32, i32)>,
    center: (i32, i32),
    radius: i32,
) -> Vec<(i32, i32)> {
    let (cx, cy) = center;
    let keep = radius + 1;
    loaded
        .filter(|&(x, y)| (x - cx).abs().max((y - cy).abs()) > keep)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wanted_regions_start_at_the_camera_region() {
        let wanted = wanted_regions((50, 50), 2);
        assert_eq!(wanted.len(), 25);
        assert_eq!(wanted[0], (50, 50));
        let ring = |&(x, y): &(i32, i32)| (x - 50).abs().max((y - 50).abs());
        assert!(wanted.windows(2).all(|w| ring(&w[0]) <= ring(&w[1])));
    }

    #[test]
    fn stale_regions_keep_one_ring_of_hysteresis() {
        let loaded = [(50, 50), (53, 50), (54, 50), (50, 47)];
        let mut stale = stale_regions(loaded.into_iter(), (50, 50), 2);
        stale.sort_unstable();
        assert_eq!(stale, vec![(54, 50)]);
    }

    #[test]
    fn object_only_invalidation_stays_on_touched_regions() {
        let regions = invalidated_regions(
            ChangeImpact::LOCATIONS | ChangeImpact::COLLISION | ChangeImpact::BOUNDS,
            &[RegionCoord::new(50, 50), RegionCoord::new(50, 50)],
        );
        assert_eq!(regions, vec![RegionCoord::new(50, 50)]);
    }

    #[test]
    fn terrain_derived_invalidation_expands_one_region_ring() {
        let regions = invalidated_regions(
            ChangeImpact::TERRAIN_MATERIAL | ChangeImpact::NORMALS | ChangeImpact::LIGHTING,
            &[RegionCoord::new(50, 50)],
        );
        assert_eq!(regions.len(), 9);
        assert_eq!(regions.first(), Some(&RegionCoord::new(49, 49)));
        assert_eq!(regions.last(), Some(&RegionCoord::new(51, 51)));
    }

    #[test]
    fn empty_impact_never_invalidates_projection_regions() {
        assert!(invalidated_regions(ChangeImpact::NONE, &[RegionCoord::new(50, 50)]).is_empty());
    }
}
