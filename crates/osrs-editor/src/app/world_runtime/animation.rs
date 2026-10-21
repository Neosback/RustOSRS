//! Scheduling and acceptance of the disposable animated-loc batch.
use super::*;
use std::collections::BTreeMap;

/// Input descriptors only: no semantic document, model, scene or GPU resource is retained.
type AnimationMembers = BTreeMap<RegionKey, Arc<RegionArtifactIdentity>>;

#[derive(Default)]
pub(super) struct AnimationSchedule {
    pub(super) smoothing: bool,
    pub(super) frozen: bool,
    pub(super) cycle: u64,
    pub(super) center: (i32, i32),
    pub(super) dirty: bool,
    pub(super) discard_in_flight: bool,
    members: Arc<AnimationMembers>,
    /// Matches the worker's sole physical FIFO rebuild slot, including no-op completions.
    in_flight: Option<Arc<AnimationArtifactIdentity>>,
}

impl AnimationSchedule {
    pub(super) fn new() -> Self {
        Self {
            dirty: true,
            ..Self::default()
        }
    }
}

/// Exact dispatch context of an aggregate animated-loc rebuild. The cycle is observational;
/// ordinary clock/camera lag does not make an otherwise current completion obsolete.
#[derive(Debug, PartialEq)]
pub(in crate::app) struct AnimationArtifactIdentity {
    source: u64,
    presentation: u32,
    cycle: u64,
    advance_cycles: i32,
    center: (i32, i32),
    radius: i32,
    smoothing: bool,
    frozen: bool,
    members: Arc<AnimationMembers>,
}

pub(in crate::app) struct AnimationArtifact {
    pub(in crate::app) identity: Arc<AnimationArtifactIdentity>,
    pub(in crate::app) geometry: SceneGeometry,
    pub(in crate::app) rebuild_ms: f32,
}

impl AnimationArtifactIdentity {
    /// Bounded diagnostic view for one inspected region; the aggregate retains every member.
    pub(in crate::app) fn describe(&self, tile: osrs_core::coords::MapTile) -> Vec<String> {
        let mut lines = vec![
            format!(
                "animation source {}; presentation {}; cycle {}; advance {}",
                self.source, self.presentation, self.cycle, self.advance_cycles,
            ),
            format!(
                "center {:?}; radius {}; smoothing {}; frozen {}; {} region inputs",
                self.center,
                self.radius,
                self.smoothing,
                self.frozen,
                self.members.len(),
            ),
        ];
        let region = tile.split_region().0;
        if let Some(member) = self.members.get(&(region.x, region.y)) {
            lines.extend(member.describe());
        } else {
            lines.push("No animated-loc input from this region.".to_owned());
        }
        lines
    }
}

impl WorldRuntime {
    pub(super) fn reset_animation_regions(&mut self) {
        for &key in self.animation.members.keys() {
            self.animations.remove_region(key);
        }
        self.animation.members = Arc::default();
        self.animation_inputs_changed();
    }

    pub(super) fn install_animation_region(
        &mut self,
        identity: Arc<RegionArtifactIdentity>,
        instances: Vec<osrs_world::AnimatedInstance>,
    ) {
        let region = identity.region();
        let key = (region.x, region.y);
        let members = Arc::make_mut(&mut self.animation.members);
        if instances.is_empty() {
            members.remove(&key);
        } else {
            members.insert(key, identity);
        }
        self.animations.insert_region(key, instances);
        self.animation_inputs_changed();
    }

    pub(super) fn remove_animation_region(&mut self, key: RegionKey) {
        Arc::make_mut(&mut self.animation.members).remove(&key);
        self.animations.remove_region(key);
        self.animation_inputs_changed();
    }

    /// Membership or playback settings changed after a batch may already have been requested.
    /// Its old completion drains the single slot without publishing over replacement inputs.
    pub(super) fn animation_inputs_changed(&mut self) {
        self.animation.discard_in_flight |= !self.animations.is_idle();
        self.animation.dirty = true;
    }

    pub(in crate::app) fn mark_animation_dirty(&mut self) {
        self.animation.dirty = true;
    }

    pub(in crate::app) fn set_animation_smoothing(&mut self, enabled: bool) {
        if self.animation.smoothing == enabled {
            return;
        }
        self.animation.smoothing = enabled;
        self.animations.set_smoothing(enabled);
        self.animation_inputs_changed();
    }

    /// Reconcile the current frame's setting before polling, including a freeze/unfreeze toggle
    /// while an older request is physically in flight. FIFO still drains that request first.
    pub(in crate::app) fn set_animation_playback(&mut self, animate_objects: bool) {
        let frozen = !animate_objects;
        if frozen != self.animation.frozen {
            self.animation.frozen = frozen;
            self.animations.set_frozen(frozen);
            self.animation_inputs_changed();
        }
    }

    /// Geometry the animation worker finished since the last poll.
    pub(in crate::app) fn poll_animation_geometry(&mut self) -> Option<AnimationArtifact> {
        let update = self.animations.poll()?;
        let identity = self.animation.in_flight.take();
        if std::mem::take(&mut self.animation.discard_in_flight) {
            return None;
        }
        let identity = identity?;
        if identity.source != self.builds.source_generation()
            || identity.presentation != self.presentation_generation
        {
            self.animation.dirty = true;
            return None;
        }
        update.geometry.map(|geometry| AnimationArtifact {
            identity,
            geometry,
            rebuild_ms: update.rebuild_ms,
        })
    }

    /// Request the next animation step if the schedule calls for one.
    pub(in crate::app) fn schedule_animation(
        &mut self,
        animate_objects: bool,
        elapsed_secs: f32,
        camera_tile: (i32, i32),
    ) {
        self.set_animation_playback(animate_objects);
        let frozen = self.animation.frozen;
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
        if self
            .animations
            .request(delta, self.animation.center, ANIMATION_RADIUS_TILES)
        {
            self.animation.in_flight = Some(Arc::new(AnimationArtifactIdentity {
                source: self.builds.source_generation(),
                presentation: self.presentation_generation,
                cycle,
                advance_cycles: delta,
                center: self.animation.center,
                radius: ANIMATION_RADIUS_TILES,
                smoothing: self.animation.smoothing,
                frozen,
                members: Arc::clone(&self.animation.members),
            }));
        }
    }
}

#[cfg(test)]
pub(in crate::app) mod tests;
