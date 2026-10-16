//! Cache-independent entity definition inputs for actor/effect rendering.
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

    #[test]
    fn entity_scale_identity_matches_reference_definition_default() {
        assert_eq!(EntityScale::IDENTITY.horizontal, 128);
        assert_eq!(EntityScale::IDENTITY.vertical, 128);
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
