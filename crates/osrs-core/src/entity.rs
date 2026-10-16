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
    pub const fn select_action_and_pose(
        self,
        idle_sequence: Option<SequenceId>,
        action_model_usable: bool,
        pose_model_usable: bool,
    ) -> ActorAnimationSelection {
        let action = if self.action_delay_cycles == 0
            && self.action.is_active()
            && action_model_usable
        {
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
        assert_eq!(
            state.select_action_and_pose(None, false, true).action,
            None
        );
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
