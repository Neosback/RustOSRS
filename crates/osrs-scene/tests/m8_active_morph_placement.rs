use osrs_core::{
    definitions::{
        DefinitionIdentity, LocType, ModelScale, ModelTranslation, ObjectDefinition, ObjectMorphs,
        ObjectPlacementFlags, VarbitDefinition,
    },
    ids::{ObjectId, VarbitId, VarpId},
    morph::MorphVariableState,
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};
use osrs_scene::{
    DynamicPlacementInput, Footprint, ObjectDefinitionLookup, PlacementKind,
    resolve_dynamic_placement,
};
use std::{collections::BTreeMap, error::Error};

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str = "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

#[derive(Default)]
struct RuntimeState {
    varps: BTreeMap<VarpId, i32>,
    varbits: BTreeMap<VarbitId, VarbitDefinition>,
}

impl MorphVariableState for RuntimeState {
    fn varp_value(&self, id: VarpId) -> Option<i32> {
        self.varps.get(&id).copied()
    }

    fn varbit_definition(&self, id: VarbitId) -> Option<&VarbitDefinition> {
        self.varbits.get(&id)
    }
}

#[derive(Default)]
struct Definitions(BTreeMap<ObjectId, ObjectDefinition>);

impl ObjectDefinitionLookup for Definitions {
    fn object_definition(&self, id: ObjectId) -> Option<&ObjectDefinition> {
        self.0.get(&id)
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

fn object_definition(
    id: u32,
    size_x: u16,
    size_y: u16,
    morphs: Option<ObjectMorphs>,
) -> Result<ObjectDefinition, Box<dyn Error>> {
    Ok(ObjectDefinition {
        identity: DefinitionIdentity::new(ObjectId::new(id), provenance()?),
        name: None,
        models: None,
        size_x,
        size_y,
        placement: ObjectPlacementFlags {
            interact_type: 0,
            blocks_projectiles: false,
            clipped: false,
            model_clipped: false,
            obstructs_ground: false,
            solid: false,
        },
        decoration_displacement: 16,
        support_items: None,
        is_rotated: false,
        non_flat_shading: false,
        contour_clip: None,
        animation: None,
        ambient: 0,
        contrast: 0,
        scale: ModelScale::IDENTITY,
        translation: ModelTranslation::ZERO,
        recolors: Vec::new(),
        retextures: Vec::new(),
        morphs,
        map_scene: None,
        map_icon: None,
        category: None,
        actions: std::array::from_fn(|_| None),
    })
}

fn heights(width: u32, height: u32) -> Vec<Vec<i32>> {
    (0..=width)
        .map(|x| {
            (0..=height)
                .map(|y| (x as i32) * 100 + (y as i32) * 10)
                .collect()
        })
        .collect()
}

#[test]
fn active_morph_recomputes_rotated_footprint_height_and_center() -> Result<(), Box<dyn Error>> {
    let source = object_definition(
        1,
        1,
        1,
        Some(ObjectMorphs {
            transform_varbit: None,
            transform_varp: Some(VarpId::new(7)),
            transforms: vec![Some(ObjectId::new(2)), None],
            fallback: None,
        }),
    )?;
    let active = object_definition(2, 2, 3, None)?;

    let mut definitions = Definitions::default();
    definitions.0.insert(ObjectId::new(2), active);

    let mut state = RuntimeState::default();
    state.varps.insert(VarpId::new(7), 0);
    let height_grid = heights(4, 4);

    let resolved = resolve_dynamic_placement(
        &definitions,
        &state,
        DynamicPlacementInput {
            source_definition: &source,
            loc_type: LocType::new(10),
            orientation: 1,
            tile: osrs_core::coords::SceneTile::new(0, 0),
            heights: &height_grid,
            scene_width: 4,
            scene_height: 4,
            existing_wall_displacement: None,
        },
    )?;

    let Some(resolved) = resolved else {
        panic!("selector 0 must resolve the active transformed definition");
    };

    assert_eq!(resolved.active_definition.identity.id, ObjectId::new(2));
    assert_eq!(
        resolved.placement.rotated_definition_footprint,
        Footprint::new(3, 2)
    );
    assert_eq!(resolved.placement.model_center.x.units(), 192);
    assert_eq!(resolved.placement.model_center.y.units(), 160);
    assert_eq!(resolved.placement.model_center.z.units(), 128);

    let PlacementKind::GameObject(game) = resolved.placement.kind else {
        panic!("type 10 must remain game-object storage");
    };
    assert_eq!(game.storage_footprint, Footprint::new(3, 2));
    assert_eq!(
        resolved.placement.storage_center,
        resolved.placement.model_center
    );
    Ok(())
}

#[test]
fn null_active_morph_produces_no_dynamic_placement() -> Result<(), Box<dyn Error>> {
    let source = object_definition(
        1,
        2,
        3,
        Some(ObjectMorphs {
            transform_varbit: None,
            transform_varp: Some(VarpId::new(7)),
            transforms: vec![Some(ObjectId::new(2)), None],
            fallback: None,
        }),
    )?;
    let definitions = Definitions::default();
    let mut state = RuntimeState::default();
    state.varps.insert(VarpId::new(7), 1);
    let height_grid = heights(4, 4);

    let resolved = resolve_dynamic_placement(
        &definitions,
        &state,
        DynamicPlacementInput {
            source_definition: &source,
            loc_type: LocType::new(10),
            orientation: 0,
            tile: osrs_core::coords::SceneTile::new(0, 0),
            heights: &height_grid,
            scene_width: 4,
            scene_height: 4,
            existing_wall_displacement: None,
        },
    )?;

    assert!(resolved.is_none());
    Ok(())
}
