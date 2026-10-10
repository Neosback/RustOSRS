use osrs_core::{
    definitions::{DefinitionIdentity, ObjectMorphs, VarbitDefinition},
    ids::{ObjectId, VarbitId, VarpId},
    morph::{MorphVariableState, resolve_object_morph},
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};
use std::{collections::BTreeMap, error::Error};

const PROFILE_DIGEST: &str =
    "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str =
    "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

#[derive(Default)]
struct FixtureState {
    varps: BTreeMap<VarpId, i32>,
    varbits: BTreeMap<VarbitId, VarbitDefinition>,
}

impl MorphVariableState for FixtureState {
    fn varp_value(&self, id: VarpId) -> Option<i32> {
        self.varps.get(&id).copied()
    }

    fn varbit_definition(&self, id: VarbitId) -> Option<&VarbitDefinition> {
        self.varbits.get(&id)
    }
}

#[test]
fn m8_morph_resolution_obeys_varbit_varp_fallback_and_null_contract()
-> Result<(), Box<dyn Error>> {
    let varbit_id = VarbitId::new(70);
    let selector_varp = VarpId::new(90);
    let direct_varp = VarpId::new(80);
    let mut state = FixtureState::default();
    state.varbits.insert(
        varbit_id,
        VarbitDefinition {
            identity: DefinitionIdentity::new(varbit_id, provenance()?),
            base_varp: selector_varp,
            start_bit: 2,
            end_bit: 3,
        },
    );
    state.varps.insert(selector_varp, 0b0100);
    state.varps.insert(direct_varp, 2);

    let both_selectors = ObjectMorphs {
        transform_varbit: Some(varbit_id),
        transform_varp: Some(direct_varp),
        transforms: vec![
            Some(ObjectId::new(1_000)),
            Some(ObjectId::new(1_001)),
            Some(ObjectId::new(1_002)),
        ],
        fallback: Some(ObjectId::new(1_099)),
    };

    // The varbit extracts selector 1. The competing varp contains 2, proving
    // the audited varbit-first ordering rather than merely an in-range lookup.
    assert_eq!(
        resolve_object_morph(&both_selectors, &state)?,
        Some(ObjectId::new(1_001))
    );

    let varp_only = ObjectMorphs {
        transform_varbit: None,
        transform_varp: Some(direct_varp),
        transforms: vec![Some(ObjectId::new(2_000)), Some(ObjectId::new(2_001))],
        fallback: Some(ObjectId::new(2_099)),
    };

    state.varps.insert(direct_varp, 1);
    assert_eq!(
        resolve_object_morph(&varp_only, &state)?,
        Some(ObjectId::new(2_001))
    );

    state.varps.insert(direct_varp, 2);
    assert_eq!(
        resolve_object_morph(&varp_only, &state)?,
        Some(ObjectId::new(2_099))
    );

    let nullable = ObjectMorphs {
        transform_varbit: None,
        transform_varp: Some(direct_varp),
        transforms: vec![None],
        fallback: None,
    };

    state.varps.insert(direct_varp, 0);
    assert_eq!(resolve_object_morph(&nullable, &state)?, None);

    state.varps.insert(direct_varp, -1);
    assert_eq!(resolve_object_morph(&nullable, &state)?, None);
    Ok(())
}

fn provenance() -> Result<TargetProvenance, Box<dyn Error>> {
    Ok(TargetProvenance::new(
        "osrs-live-241-2026-09-30-openrs2-2727",
        ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
        CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
        1,
    )?)
}
