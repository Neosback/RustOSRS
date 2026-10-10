//! Runtime object-morph selection for M8.
//!
//! Object definitions preserve transform metadata during decode, but the active
//! target is runtime state. This module resolves that state without depending on
//! cache transport, scene storage, renderer types, or editor preview storage.

use crate::definitions::{ObjectMorphs, VarbitDefinition};
use crate::ids::{ObjectId, VarbitId, VarpId};
use core::fmt;

/// Runtime variable access required by object morph resolution.
///
/// Implementations may be backed by client state, an editor preview, a test
/// fixture, or another semantic store. `osrs-core` deliberately does not own
/// that storage.
pub trait MorphVariableState {
    /// Return the current signed reference varp value.
    fn varp_value(&self, id: VarpId) -> Option<i32>;

    /// Return the canonical definition required to extract a varbit selector.
    fn varbit_definition(&self, id: VarbitId) -> Option<&VarbitDefinition>;
}

/// Invalid or incomplete runtime state for exact morph selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MorphResolveError {
    MissingVarbitDefinition(VarbitId),
    VarbitDefinitionMismatch {
        requested: VarbitId,
        actual: VarbitId,
    },
    MissingVarpValue(VarpId),
    InvalidVarbitRange {
        varbit: VarbitId,
        start_bit: u8,
        end_bit: u8,
    },
}

impl fmt::Display for MorphResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingVarbitDefinition(id) => {
                write!(
                    formatter,
                    "missing varbit definition {id} for morph selection"
                )
            }
            Self::VarbitDefinitionMismatch { requested, actual } => write!(
                formatter,
                "morph state returned varbit definition {actual} for requested {requested}"
            ),
            Self::MissingVarpValue(id) => {
                write!(formatter, "missing varp value {id} for morph selection")
            }
            Self::InvalidVarbitRange {
                varbit,
                start_bit,
                end_bit,
            } => write!(
                formatter,
                "varbit {varbit} has invalid inclusive bit range {start_bit}..={end_bit}"
            ),
        }
    }
}

impl std::error::Error for MorphResolveError {}

/// Resolve the active transformed object id.
///
/// Exact `MORPH-001` ordering is preserved:
///
/// 1. varbit wins when present;
/// 2. otherwise varp supplies the selector;
/// 3. an in-range non-negative selector addresses `transforms`;
/// 4. every other selector uses `fallback`;
/// 5. `None` is the semantic null-transform result.
pub fn resolve_object_morph<S: MorphVariableState + ?Sized>(
    morphs: &ObjectMorphs,
    state: &S,
) -> Result<Option<ObjectId>, MorphResolveError> {
    let selector = if let Some(varbit) = morphs.transform_varbit {
        let definition = state
            .varbit_definition(varbit)
            .ok_or(MorphResolveError::MissingVarbitDefinition(varbit))?;
        if definition.identity.id != varbit {
            return Err(MorphResolveError::VarbitDefinitionMismatch {
                requested: varbit,
                actual: definition.identity.id,
            });
        }

        let backing_value = state
            .varp_value(definition.base_varp)
            .ok_or(MorphResolveError::MissingVarpValue(definition.base_varp))?;
        extract_varbit_value(definition, backing_value)?
    } else if let Some(varp) = morphs.transform_varp {
        state
            .varp_value(varp)
            .ok_or(MorphResolveError::MissingVarpValue(varp))?
    } else {
        -1
    };

    Ok(select_morph_target(morphs, selector))
}

/// Apply the decoded inclusive varbit range to one signed reference varp value.
///
/// The bitwise behavior matches Java `int` semantics. A full-width `0..=31`
/// range therefore preserves the signed bit pattern rather than widening it.
pub fn extract_varbit_value(
    definition: &VarbitDefinition,
    varp_value: i32,
) -> Result<i32, MorphResolveError> {
    if definition.start_bit > definition.end_bit || definition.end_bit >= 32 {
        return Err(MorphResolveError::InvalidVarbitRange {
            varbit: definition.identity.id,
            start_bit: definition.start_bit,
            end_bit: definition.end_bit,
        });
    }

    let width = u32::from(definition.end_bit - definition.start_bit) + 1;
    let mask = if width == 32 {
        u32::MAX
    } else {
        (1_u32 << width) - 1
    };
    let shifted = (varp_value >> definition.start_bit) as u32;
    Ok((shifted & mask) as i32)
}

/// Select one decoded transform target from an already resolved selector.
///
/// This helper is public so deterministic preview/runtime code can test target
/// selection independently from variable-storage integration.
pub fn select_morph_target(morphs: &ObjectMorphs, selector: i32) -> Option<ObjectId> {
    if selector >= 0
        && let Some(target) = morphs.transforms.get(selector as usize)
    {
        return *target;
    }
    morphs.fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definitions::DefinitionIdentity;
    use crate::provenance::{CacheFingerprint, ProfileDigest, TargetProvenance};
    use std::{collections::BTreeMap, error::Error};

    const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
    const CACHE_FINGERPRINT: &str =
        "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

    #[derive(Default)]
    struct TestState {
        varps: BTreeMap<VarpId, i32>,
        varbits: BTreeMap<VarbitId, VarbitDefinition>,
    }

    impl MorphVariableState for TestState {
        fn varp_value(&self, id: VarpId) -> Option<i32> {
            self.varps.get(&id).copied()
        }

        fn varbit_definition(&self, id: VarbitId) -> Option<&VarbitDefinition> {
            self.varbits.get(&id)
        }
    }

    fn provenance() -> Result<TargetProvenance, Box<dyn Error>> {
        Ok(TargetProvenance::new(
            "osrs-live-241-2026-09-30-openrs2-2727",
            ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
            CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
            1,
        )?)
    }

    fn varbit(
        id: u32,
        base_varp: u32,
        start_bit: u8,
        end_bit: u8,
    ) -> Result<VarbitDefinition, Box<dyn Error>> {
        Ok(VarbitDefinition {
            identity: DefinitionIdentity::new(VarbitId::new(id), provenance()?),
            base_varp: VarpId::new(base_varp),
            start_bit,
            end_bit,
        })
    }

    fn morphs(transform_varbit: Option<VarbitId>, transform_varp: Option<VarpId>) -> ObjectMorphs {
        ObjectMorphs {
            transform_varbit,
            transform_varp,
            transforms: vec![Some(ObjectId::new(100)), Some(ObjectId::new(101)), None],
            fallback: Some(ObjectId::new(200)),
        }
    }

    #[test]
    fn inclusive_varbit_range_extracts_reference_selector() -> Result<(), Box<dyn Error>> {
        let definition = varbit(7, 9, 3, 5)?;
        let value = extract_varbit_value(&definition, 0b1110_1000)?;
        assert_eq!(value, 5);
        Ok(())
    }

    #[test]
    fn full_width_varbit_preserves_signed_java_int_bits() -> Result<(), Box<dyn Error>> {
        let definition = varbit(7, 9, 0, 31)?;
        assert_eq!(extract_varbit_value(&definition, -1)?, -1);
        Ok(())
    }

    #[test]
    fn varbit_wins_over_varp_when_both_are_present() -> Result<(), Box<dyn Error>> {
        let mut state = TestState::default();
        state
            .varbits
            .insert(VarbitId::new(7), varbit(7, 9, 1, 2)?);
        state.varps.insert(VarpId::new(9), 0b0010);
        state.varps.insert(VarpId::new(8), 2);

        let target = resolve_object_morph(
            &morphs(Some(VarbitId::new(7)), Some(VarpId::new(8))),
            &state,
        );
        assert_eq!(target, Ok(Some(ObjectId::new(101))));
        Ok(())
    }

    #[test]
    fn varp_selector_and_out_of_range_fallback_are_exact() {
        let mut state = TestState::default();
        state.varps.insert(VarpId::new(8), 0);
        let definition = morphs(None, Some(VarpId::new(8)));

        assert_eq!(
            resolve_object_morph(&definition, &state),
            Ok(Some(ObjectId::new(100)))
        );

        state.varps.insert(VarpId::new(8), 3);
        assert_eq!(
            resolve_object_morph(&definition, &state),
            Ok(Some(ObjectId::new(200)))
        );
    }

    #[test]
    fn null_transform_and_null_fallback_remain_semantic_absence() {
        let definition = ObjectMorphs {
            transform_varbit: None,
            transform_varp: Some(VarpId::new(8)),
            transforms: vec![Some(ObjectId::new(100)), None],
            fallback: None,
        };
        let mut state = TestState::default();

        state.varps.insert(VarpId::new(8), 1);
        assert_eq!(resolve_object_morph(&definition, &state), Ok(None));

        state.varps.insert(VarpId::new(8), -1);
        assert_eq!(resolve_object_morph(&definition, &state), Ok(None));
    }

    #[test]
    fn missing_runtime_state_and_invalid_ranges_fail_explicitly() -> Result<(), Box<dyn Error>> {
        let state = TestState::default();
        assert_eq!(
            resolve_object_morph(&morphs(Some(VarbitId::new(7)), None), &state),
            Err(MorphResolveError::MissingVarbitDefinition(VarbitId::new(7)))
        );

        let invalid = varbit(7, 9, 6, 5)?;
        assert_eq!(
            extract_varbit_value(&invalid, 0),
            Err(MorphResolveError::InvalidVarbitRange {
                varbit: VarbitId::new(7),
                start_bit: 6,
                end_bit: 5,
            })
        );
        Ok(())
    }
}
