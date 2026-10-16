//! Cache-independent entity definition and actor-runtime inputs.
//!
//! These types intentionally describe semantic data, not revision-specific
//! config opcodes. The checked-in RuneLite source used for the current actor
//! study declares revision 235 while RustOSRS targets cache revision 241, so
//! target-byte decoding stays blocked until exact revision-241/JTS evidence is
//! pinned. Adding these representations does not claim decoder parity.

use crate::definitions::{DefinitionIdentity, RecolorPair, RetexturePair};
use crate::ids::{ModelId, NpcId, SequenceId, SpotAnimationId, VarbitId, VarpId};

/// Two-axis definition scale used by NPC and spot-animation model paths.
///
/// The client uses 128 as the identity value. `horizontal` applies to the two
/// ground-plane axes while `vertical` applies to model height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityScale {
    pub horizontal: u16,
    pub vertical: u16,
}

impl EntityScale {
    pub const IDENTITY: Self = Self {
        horizontal: 128,
        vertical: 128,
    };
}

/// One live actor animation-sequence slot.
///
/// This mirrors the cache-independent state shape of the pinned client's
/// `AnimationSequence`: sequence identity plus per-instance frame/cycle/count
/// state. Shared [`crate::definitions::SequenceDefinition`] data stays outside
/// the slot and is resolved by sequence id.
///
/// Runtime mutation/advancement belongs to the future playtest owner. This type
/// deliberately does not reuse [`crate::animation::SequencePlaybackState`],
/// whose current contract is the audited dynamic-loc progression path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ActorSequenceState {
    pub sequence: Option<SequenceId>,
    pub frame: Option<u32>,
    pub frame_cycle: i32,
    pub frame_count: i32,
    pub loop_count: i32,
}

impl ActorSequenceState {
    pub const fn is_active(self) -> bool {
        self.sequence.is_some()
    }

    /// Deactivate the slot without rewriting its frame bookkeeping.
    ///
    /// The pinned client's `AnimationSequence.clear()` only replaces the
    /// sequence id with its inactive sentinel. Keeping the remaining fields
    /// intact matters because clearing and resetting a frame are distinct
    /// source operations.
    pub fn clear(&mut self) {
        self.sequence = None;
    }

    /// Reset per-play frame state while preserving sequence and frame count.
    ///
    /// This mirrors `AnimationSequence.resetFrame()`. Rust uses `Some(0)` for
    /// the source's active frame zero rather than conflating it with the
    /// inactive/default `None` representation.
    pub fn reset_frame(&mut self) {
        self.frame = Some(0);
        self.frame_cycle = 0;
        self.loop_count = 0;
    }

    /// Change the slot's sequence using the pinned client's lifecycle rule.
    ///
    /// In the source, `setSequence` first asks the previous sequence to reset
    /// any cached-model resource state. If that external reset succeeds, the
    /// frame fields are intentionally preserved; otherwise `resetFrame()` is
    /// applied. Resource lookup/reset does not belong in `osrs-core`, so the
    /// caller supplies that audited outcome as `previous_cached_model_reset`.
    /// Supplying the current sequence is a no-op, matching the source.
    pub fn set_sequence(
        &mut self,
        sequence: Option<SequenceId>,
        previous_cached_model_reset: bool,
    ) {
        if self.sequence == sequence {
            return;
        }

        self.sequence = sequence;
        if !previous_cached_model_reset {
            self.reset_frame();
        }
    }

    /// Set a sequence only while the slot is inactive.
    ///
    /// An inactive source slot has no previous cached-model sequence to reset,
    /// so activation follows the ordinary frame-reset path.
    pub fn set_sequence_if_inactive(&mut self, sequence: Option<SequenceId>) {
        if !self.is_active() {
            self.set_sequence(sequence, false);
        }
    }
}

/// Player/NPC-owned animation slots before model assembly.
///
/// The pinned `Actor` owns three independent `AnimationSequence` instances:
/// `poseAnimationSequence`, `animationSequence`, and the still-obfuscated
/// `field1015`. The first two are kept as `pose` and `action`; the third remains
/// neutrally named `auxiliary` until target-revision/JTS evidence pins its full
/// lifecycle semantics. `action_delay_cycles` mirrors the source field that
/// gates action-sequence participation in model selection.
///
/// The renderer consumes resolved/posed output and must never own or advance
/// this state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ActorAnimationState {
    pub pose: ActorSequenceState,
    pub action: ActorSequenceState,
    pub auxiliary: ActorSequenceState,
    pub action_delay_cycles: i32,
}

/// Action and pose slots admitted by the actor's model-selection rules.
///
/// This is a selection result only. It neither advances animation state nor
/// resolves sequence definitions, frames, cached models, or renderer resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ActorAnimationSelection {
    pub action: Option<ActorSequenceState>,
    pub pose: Option<ActorSequenceState>,
}

impl ActorAnimationState {
    /// Select the action/pose slots that may participate in actor model work.
    ///
    /// This mirrors the pinned client's `Actor.method2507()` / `method2508()`:
    /// an action participates only when its delay is zero, it is active, and
    /// its sequence is model-usable; pose participates only when active and
    /// model-usable, with the idle pose suppressed while an admitted action is
    /// present. The `*_model_usable` inputs correspond to the source
    /// `SequenceDefinition.method4925()` predicate and stay external because it
    /// depends on resolved frame/cached-model resources rather than actor state.
    pub fn select_action_and_pose(
        self,
        idle_sequence: Option<SequenceId>,
        action_model_usable: bool,
        pose_model_usable: bool,
    ) -> ActorAnimationSelection {
        let action =
            if self.action_delay_cycles == 0 && self.action.is_active() && action_model_usable {
                Some(self.action)
            } else {
                None
            };

        let pose = if self.pose.is_active()
            && pose_model_usable
            && !(self.pose.sequence == idle_sequence && action.is_some())
        {
            Some(self.pose)
        } else {
            None
        };

        ActorAnimationSelection { action, pose }
    }
}

/// Movement/idle sequence references authored by an NPC definition.
///
/// This is the minimum source-audited set needed to initialize actor runtime
/// state. Current frame/cycle state belongs to the future playtest/runtime
/// owner, and the exact revision-241 definition may extend this set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct NpcMovementSequences {
    pub idle: Option<SequenceId>,
    pub turn_left: Option<SequenceId>,
    pub turn_right: Option<SequenceId>,
    pub walk: Option<SequenceId>,
    pub walk_back: Option<SequenceId>,
    pub walk_left: Option<SequenceId>,
    pub walk_right: Option<SequenceId>,
    pub run: [Option<SequenceId>; 4],
    pub crawl: [Option<SequenceId>; 4],
}

/// NPC transform selector metadata.
///
/// `None` entries remain semantic null transforms. Exact revision-241 decode
/// and runtime selector fixtures are still required before NPC morph parity is
/// promoted to verified.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NpcMorphs {
    pub transform_varbit: Option<VarbitId>,
    pub transform_varp: Option<VarpId>,
    pub transforms: Vec<Option<NpcId>>,
    pub fallback: Option<NpcId>,
}

/// Render/runtime-relevant semantic NPC definition state.
///
/// This is deliberately not presented as a complete revision-241 config codec.
/// Fields may be extended once the exact target deob/JTS definition layout is
/// pinned, without teaching renderer code about config opcodes or sentinels.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NpcDefinition {
    pub identity: DefinitionIdentity<NpcId>,
    pub name: Option<String>,
    pub body_models: Vec<ModelId>,
    pub head_models: Vec<ModelId>,
    pub size: u16,
    pub movement_sequences: NpcMovementSequences,
    pub recolors: Vec<RecolorPair>,
    pub retextures: Vec<RetexturePair>,
    pub combat_level: Option<u16>,
    pub scale: EntityScale,
    pub ambient: i16,
    pub contrast: i16,
    pub morphs: Option<NpcMorphs>,
    pub render: NpcRenderFlags,
    pub actions: [Option<String>; 5],
    pub full_recolor: Option<u16>,
}

/// Definition-controlled client rendering and turning behavior.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NpcRenderFlags {
    pub rotation_speed: u16,
    pub priority: u8,
    pub minimap_visible: bool,
    pub interactable: bool,
    pub clickable: bool,
    pub follower: bool,
    pub low_priority_follower_ops: bool,
    pub height: Option<u16>,
    pub footprint_size: Option<u16>,
    pub ready_animation_during_action: bool,
    pub hide_for_overlap: bool,
    pub overlap_tint: Option<u16>,
    pub depth_buffer: bool,
}

impl Default for NpcRenderFlags {
    fn default() -> Self {
        Self {
            rotation_speed: 32,
            priority: 0,
            minimap_visible: true,
            interactable: true,
            clickable: true,
            follower: false,
            low_priority_follower_ops: false,
            height: None,
            footprint_size: None,
            ready_animation_during_action: false,
            hide_for_overlap: false,
            overlap_tint: None,
            depth_buffer: true,
        }
    }
}

/// Canonical reusable spot-animation definition inputs.
///
/// Actor-attached effects, standalone graphics objects and projectiles should
/// all resolve through this definition rather than maintaining separate model
/// configuration paths. Runtime start/lifetime/frame/position state is not part
/// of this definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpotAnimationDefinition {
    pub identity: DefinitionIdentity<SpotAnimationId>,
    pub model: Option<ModelId>,
    pub sequence: Option<SequenceId>,
    pub scale: EntityScale,
    /// Definition-authored orientation consumed by the spot-model transform.
    /// The audited source uses quarter-turn values; exact target validation is
    /// a decoder concern.
    pub orientation: u16,
    pub ambient: i16,
    pub contrast: i16,
    pub recolors: Vec<RecolorPair>,
    pub retextures: Vec<RetexturePair>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor_slot(sequence: SequenceId) -> ActorSequenceState {
        ActorSequenceState {
            sequence: Some(sequence),
            frame: Some(0),
            ..ActorSequenceState::default()
        }
    }

    fn advanced_actor_slot(sequence: SequenceId) -> ActorSequenceState {
        ActorSequenceState {
            sequence: Some(sequence),
            frame: Some(7),
            frame_cycle: 8,
            frame_count: 9,
            loop_count: 10,
        }
    }

    #[test]
    fn entity_scale_identity_matches_reference_definition_default() {
        assert_eq!(EntityScale::IDENTITY.horizontal, 128);
        assert_eq!(EntityScale::IDENTITY.vertical, 128);
    }

    #[test]
    fn actor_sequence_default_matches_inactive_reference_slot() {
        let slot = ActorSequenceState::default();
        assert!(!slot.is_active());
        assert_eq!(slot.sequence, None);
        assert_eq!(slot.frame, None);
        assert_eq!(slot.frame_cycle, 0);
        assert_eq!(slot.frame_count, 0);
        assert_eq!(slot.loop_count, 0);
    }

    #[test]
    fn actor_sequence_clear_only_deactivates_slot() {
        let mut slot = advanced_actor_slot(SequenceId::new(100));
        slot.clear();

        assert_eq!(slot.sequence, None);
        assert_eq!(slot.frame, Some(7));
        assert_eq!(slot.frame_cycle, 8);
        assert_eq!(slot.frame_count, 9);
        assert_eq!(slot.loop_count, 10);
    }

    #[test]
    fn actor_sequence_reset_frame_preserves_sequence_and_frame_count() {
        let sequence = SequenceId::new(100);
        let mut slot = advanced_actor_slot(sequence);
        slot.reset_frame();

        assert_eq!(slot.sequence, Some(sequence));
        assert_eq!(slot.frame, Some(0));
        assert_eq!(slot.frame_cycle, 0);
        assert_eq!(slot.frame_count, 9);
        assert_eq!(slot.loop_count, 0);
    }

    #[test]
    fn actor_sequence_set_same_sequence_is_noop() {
        let sequence = SequenceId::new(100);
        let mut slot = advanced_actor_slot(sequence);
        let original = slot;
        slot.set_sequence(Some(sequence), false);

        assert_eq!(slot, original);
    }

    #[test]
    fn actor_sequence_change_resets_frame_without_cached_model_reset() {
        let mut slot = advanced_actor_slot(SequenceId::new(100));
        slot.set_sequence(Some(SequenceId::new(200)), false);

        assert_eq!(slot.sequence, Some(SequenceId::new(200)));
        assert_eq!(slot.frame, Some(0));
        assert_eq!(slot.frame_cycle, 0);
        assert_eq!(slot.frame_count, 9);
        assert_eq!(slot.loop_count, 0);
    }

    #[test]
    fn actor_sequence_change_preserves_frame_after_cached_model_reset() {
        let mut slot = advanced_actor_slot(SequenceId::new(100));
        slot.set_sequence(Some(SequenceId::new(200)), true);

        assert_eq!(slot.sequence, Some(SequenceId::new(200)));
        assert_eq!(slot.frame, Some(7));
        assert_eq!(slot.frame_cycle, 8);
        assert_eq!(slot.frame_count, 9);
        assert_eq!(slot.loop_count, 10);
    }

    #[test]
    fn actor_sequence_set_if_inactive_only_initializes_empty_slot() {
        let sequence = SequenceId::new(100);
        let mut inactive = ActorSequenceState {
            frame: Some(7),
            frame_cycle: 8,
            frame_count: 9,
            loop_count: 10,
            ..ActorSequenceState::default()
        };
        inactive.set_sequence_if_inactive(Some(sequence));

        assert_eq!(inactive.sequence, Some(sequence));
        assert_eq!(inactive.frame, Some(0));
        assert_eq!(inactive.frame_cycle, 0);
        assert_eq!(inactive.frame_count, 9);
        assert_eq!(inactive.loop_count, 0);

        let mut active = advanced_actor_slot(sequence);
        let original = active;
        active.set_sequence_if_inactive(Some(SequenceId::new(200)));
        assert_eq!(active, original);
    }

    #[test]
    fn actor_animation_state_keeps_three_independent_slots() {
        let state = ActorAnimationState {
            pose: ActorSequenceState {
                sequence: Some(SequenceId::new(100)),
                frame: Some(1),
                ..ActorSequenceState::default()
            },
            action: ActorSequenceState {
                sequence: Some(SequenceId::new(200)),
                frame: Some(2),
                ..ActorSequenceState::default()
            },
            auxiliary: ActorSequenceState {
                sequence: Some(SequenceId::new(300)),
                frame: Some(3),
                ..ActorSequenceState::default()
            },
            action_delay_cycles: 4,
        };

        assert_eq!(state.pose.sequence, Some(SequenceId::new(100)));
        assert_eq!(state.action.sequence, Some(SequenceId::new(200)));
        assert_eq!(state.auxiliary.sequence, Some(SequenceId::new(300)));
        assert_eq!(state.action_delay_cycles, 4);
    }

    #[test]
    fn actor_action_selection_requires_zero_delay_active_and_model_usable() {
        let action = actor_slot(SequenceId::new(200));
        let state = ActorAnimationState {
            action,
            ..ActorAnimationState::default()
        };

        assert_eq!(
            state.select_action_and_pose(None, true, true).action,
            Some(action)
        );
        assert_eq!(
            ActorAnimationState {
                action_delay_cycles: 1,
                ..state
            }
            .select_action_and_pose(None, true, true)
            .action,
            None
        );
        assert_eq!(state.select_action_and_pose(None, false, true).action, None);
        assert_eq!(
            ActorAnimationState {
                action: ActorSequenceState::default(),
                ..state
            }
            .select_action_and_pose(None, true, true)
            .action,
            None
        );
    }

    #[test]
    fn actor_pose_selection_suppresses_idle_only_for_admitted_action() {
        let idle = SequenceId::new(100);
        let pose = actor_slot(idle);
        let action = actor_slot(SequenceId::new(200));
        let state = ActorAnimationState {
            pose,
            action,
            ..ActorAnimationState::default()
        };

        let with_action = state.select_action_and_pose(Some(idle), true, true);
        assert_eq!(with_action.action, Some(action));
        assert_eq!(with_action.pose, None);

        let delayed = ActorAnimationState {
            action_delay_cycles: 1,
            ..state
        }
        .select_action_and_pose(Some(idle), true, true);
        assert_eq!(delayed.action, None);
        assert_eq!(delayed.pose, Some(pose));

        let non_idle_pose = actor_slot(SequenceId::new(101));
        let non_idle = ActorAnimationState {
            pose: non_idle_pose,
            ..state
        }
        .select_action_and_pose(Some(idle), true, true);
        assert_eq!(non_idle.action, Some(action));
        assert_eq!(non_idle.pose, Some(non_idle_pose));

        assert_eq!(
            state.select_action_and_pose(Some(idle), true, false).pose,
            None
        );
    }

    #[test]
    fn npc_morphs_preserve_null_targets_and_fallback() {
        let morphs = NpcMorphs {
            transform_varbit: Some(VarbitId::new(7)),
            transform_varp: Some(VarpId::new(8)),
            transforms: vec![Some(NpcId::new(100)), None],
            fallback: Some(NpcId::new(200)),
        };

        assert_eq!(morphs.transforms[0], Some(NpcId::new(100)));
        assert_eq!(morphs.transforms[1], None);
        assert_eq!(morphs.fallback, Some(NpcId::new(200)));
    }

    #[test]
    fn movement_sequence_defaults_are_semantic_absence() {
        assert_eq!(NpcMovementSequences::default().idle, None);
        assert_eq!(NpcMovementSequences::default().walk, None);
        assert_eq!(NpcMovementSequences::default().turn_left, None);
    }
}
