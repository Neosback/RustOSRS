//! Runtime playback of animated locs (`DynamicObject`).
//!
//! The worker thread produces [`AnimatedInstance`]s: the lit base model, its decoded legacy
//! frames, and the placement. [`AnimationSystem`] (main thread) advances every instance's
//! sequence at the 50 Hz game-cycle rate, poses the shared lit base into a private copy for the
//! active frame, applies ground contouring, and emits geometry for the instances near the camera.
//!
//! Skeletal sequences (a small minority of animated objects) are not posed; those locs render
//! their static base model instead.

use osrs_core::{
    animation::{SequencePlaybackState, advance_dynamic_sequence},
    animation_pose::{LegacyAnimationFrame, interpolate_legacy_frames, pose_legacy_object_model},
    contour::{ContourGroundInput, contour_ground_in_place},
    definitions::SequenceDefinition,
    lighting::ReferenceLitModel,
};
use osrs_render::{ModelPlacement, SceneGeometry};
use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::Arc,
};

/// Ground contour inputs of one animated loc.
#[derive(Debug, Clone)]
pub struct AnimatedContour {
    pub clip: i32,
    /// Plane height grid of the scene window the loc was built in, indexed `[x][z]`.
    pub heights: Arc<Vec<Vec<i32>>>,
    pub origin_x: i32,
    pub base_height: i32,
    pub origin_z: i32,
}

/// The renderable model of an animated loc.
#[derive(Debug, Clone)]
pub struct AnimatedModel {
    /// Lit, unposed, uncontoured base model (shared between locs of the same definition).
    pub base: Arc<ReferenceLitModel>,
    pub sequence: Arc<SequenceDefinition>,
    pub frames: Vec<Arc<LegacyAnimationFrame>>,
    /// Loc orientation used by the pose's pre/post rotation.
    pub orientation: u8,
    pub contour: Option<AnimatedContour>,
    /// Looping sequences start at a random frame (`ObjectComposition.boolean3`).
    pub random_start: bool,
}

/// One placed animated model slot.
#[derive(Debug, Clone)]
pub struct AnimatedInstance {
    pub model: Arc<AnimatedModel>,
    /// World zone coordinates of the owning zone.
    pub zone: (i32, i32),
    pub level: u8,
    pub min_plane: u8,
    /// Model origin in zone-local units.
    pub local: (i32, i32, i32),
    /// Extra instance rotation in JAU.
    pub rotation: u16,
}

struct Live {
    instance: AnimatedInstance,
    state: SequencePlaybackState,
    /// Frame index last emitted; `None` when playback stopped.
    frame: Option<u32>,
}

/// Start state of a freshly placed animated loc. `DynamicObject`'s constructor starts looping
/// sequences (`frameCount != -1`) of objects without opcode 89 at a random frame and a random cycle
/// inside it, so neighbouring fires and torches are out of step. The client uses `Math.random()`;
/// a hash of the placement keeps rebuilds of the same region stable.
fn initial_playback(instance: &AnimatedInstance) -> (SequencePlaybackState, u32) {
    let sequence = &instance.model.sequence;
    let eligible = instance.model.random_start
        && sequence.frame_step.is_some()
        && sequence.skeletal_animation.is_none()
        && !sequence.frame_delays.is_empty();
    if !eligible {
        return (SequencePlaybackState::new(), 0);
    }
    let mut hasher = DefaultHasher::new();
    (
        instance.zone,
        instance.local,
        instance.level,
        sequence.identity.id.get(),
    )
        .hash(&mut hasher);
    let bits = hasher.finish();
    let frame = (bits % sequence.frame_delays.len() as u64) as usize;
    let delay = u64::from(sequence.frame_delays[frame]).max(1);
    let cycle = ((bits >> 32) % delay) as i32;
    (
        SequencePlaybackState::starting_at(frame as u32, cycle),
        frame as u32,
    )
}

/// Playback state for every resident region's animated locs.
#[derive(Default)]
pub struct AnimationSystem {
    regions: HashMap<(i32, i32), Vec<Live>>,
    /// Blend each frame toward the next (RuneLite's Animation Smoothing plugin).
    smoothing: bool,
}

/// Sequences the Animation Smoothing plugin leaves alone (`isAnimationInterpolatable` returns
/// false): interpolating them distorts rotating wheels, sails and a few NPC cycles.
const NOT_SMOOTHED: [u32; 23] = [
    6566, 8266, 8267, 8270, 8271, 4652, 5530, 5531, 5583, 6818, 244, 1051, 5857, 6495, 7898, 9450,
    8977, 367, 3558, 4519, 9493, 8499, 11291,
];

impl AnimationSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Turn frame blending on or off.
    pub fn set_smoothing(&mut self, enabled: bool) {
        self.smoothing = enabled;
    }

    pub fn instance_count(&self) -> usize {
        self.regions.values().map(Vec::len).sum()
    }

    /// Diagnostic: `(object-less summaries)` of instances whose playback has stopped, as
    /// `(sequence id, frame count, frame_step, max_loops, total delay)`.
    pub fn stopped(&self) -> Vec<(u32, usize, Option<u16>, u16, u32)> {
        self.regions
            .values()
            .flatten()
            .filter(|live| live.frame.is_none())
            .map(|live| {
                let seq = &live.instance.model.sequence;
                (
                    seq.identity.id.get(),
                    seq.frame_ids.len(),
                    seq.frame_step,
                    seq.max_loops,
                    seq.frame_delays.iter().map(|d| u32::from(*d)).sum(),
                )
            })
            .collect()
    }

    pub fn insert_region(&mut self, key: (i32, i32), instances: Vec<AnimatedInstance>) {
        if instances.is_empty() {
            self.regions.remove(&key);
            return;
        }
        let live = instances
            .into_iter()
            .map(|instance| {
                let (state, frame) = initial_playback(&instance);
                Live {
                    instance,
                    state,
                    frame: Some(frame),
                }
            })
            .collect();
        self.regions.insert(key, live);
    }

    pub fn remove_region(&mut self, key: (i32, i32)) {
        self.regions.remove(&key);
    }

    /// Advance every instance by `cycles` game cycles (20 ms each). Returns whether any
    /// instance's displayed frame changed.
    pub fn advance(&mut self, cycles: i32) -> bool {
        if cycles <= 0 {
            return false;
        }
        let mut changed = false;
        for live in self.regions.values_mut().flatten() {
            let model = &live.instance.model;
            let result = advance_dynamic_sequence(&model.sequence, &mut live.state, cycles);
            // A sequence without a loop-back step plays once; `DynamicObject.getModel` then
            // resets it and the loc shows its static, unposed model from then on.
            let frame = match result {
                Ok(_) => live.state.frame(),
                Err(_) => {
                    live.state.reset();
                    None
                }
            };
            if frame != live.frame {
                live.frame = frame;
                changed = true;
            }
        }
        changed
    }

    /// The current frame blended toward the next by the elapsed share of its length, when
    /// smoothing applies to this loc.
    fn smoothed_frame(&self, live: &Live) -> Option<LegacyAnimationFrame> {
        if !self.smoothing {
            return None;
        }
        let model = &live.instance.model;
        let sequence = &model.sequence;
        if NOT_SMOOTHED.contains(&sequence.identity.id.get()) {
            return None;
        }
        let current = live.frame? as usize;
        let length = model.frames.len();
        let mut next = current + 1;
        if next >= length {
            // Loop back like the client (`frame -= frameStep`); sequences without a step end here.
            let step = usize::from(sequence.frame_step?);
            next = next.checked_sub(step).filter(|&n| n < length)?;
        }
        let delay = f32::from(*sequence.frame_delays.get(current)?).max(1.0);
        let t = live.state.frame_cycle() as f32 / delay;
        interpolate_legacy_frames(&model.frames[current], &model.frames[next], t)
    }

    /// Geometry of the instances whose zone origin lies within `radius_tiles` of
    /// `center_tile` (world tile coordinates).
    pub fn build_geometry(&self, center_tile: (i32, i32), radius_tiles: i32) -> SceneGeometry {
        let mut geometry = SceneGeometry::default();
        let radius_zones = radius_tiles / 8 + 1;
        let center_zone = (center_tile.0 / 8, center_tile.1 / 8);
        for live in self.regions.values().flatten() {
            let instance = &live.instance;
            if (instance.zone.0 - center_zone.0).abs() > radius_zones
                || (instance.zone.1 - center_zone.1).abs() > radius_zones
            {
                continue;
            }
            let model = &instance.model;
            let frame = live
                .frame
                .and_then(|index| model.frames.get(index as usize));
            let blended = self.smoothed_frame(live);
            let posed = blended
                .as_ref()
                .or(frame.map(|frame| &**frame))
                .and_then(|frame| {
                    pose_legacy_object_model(&model.base, frame, model.orientation).ok()
                })
                .map(std::borrow::Cow::Owned);
            let mut posed = match posed {
                Some(posed) => posed,
                None if model.contour.is_some() => std::borrow::Cow::Owned((*model.base).clone()),
                None => std::borrow::Cow::Borrowed(&*model.base),
            };
            if let Some(contour) = &model.contour {
                let _ = contour_ground_in_place(
                    posed.to_mut(),
                    ContourGroundInput {
                        heights: &contour.heights,
                        origin_x: contour.origin_x,
                        base_height: contour.base_height,
                        origin_z: contour.origin_z,
                        clip: contour.clip,
                    },
                );
            }
            let zone = geometry.zone_mut(instance.zone.0, instance.zone.1);
            zone.group_mut(instance.level, instance.min_plane)
                .push_model(
                    &posed,
                    ModelPlacement {
                        x: instance.local.0,
                        y: instance.local.1,
                        z: instance.local.2,
                        orientation: instance.rotation,
                    },
                );
        }
        geometry
    }
}
