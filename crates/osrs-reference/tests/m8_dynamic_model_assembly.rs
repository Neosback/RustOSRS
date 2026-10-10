use osrs_core::{
    animation_pose::{LegacyAnimationFrame, LegacyFrameTransform, LegacySkeletonTransform},
    coords::{ModelPoint, SceneTile},
    definitions::{
        DefinitionIdentity, LocType, ModelScale, ModelTranslation, ObjectDefinition, ObjectModels,
        ObjectMorphs, ObjectPlacementFlags, VarbitDefinition,
    },
    ids::{ModelId, ObjectId, VarbitId, VarpId},
    lighting::ReferenceLitModel,
    model::{FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts},
    model_identity::ModelSemanticIdentity,
    morph::MorphVariableState,
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};
use osrs_scene::{
    DynamicLitModelLookup, DynamicModelInput, DynamicPlacementInput, ModelRequest,
    ObjectDefinitionLookup, resolve_dynamic_model,
};
use std::{borrow::Cow, cell::Cell, collections::BTreeMap, error::Error};

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str = "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

#[derive(Default)]
struct FixtureDefinitions {
    objects: BTreeMap<ObjectId, ObjectDefinition>,
}

impl ObjectDefinitionLookup for FixtureDefinitions {
    fn object_definition(&self, id: ObjectId) -> Option<&ObjectDefinition> {
        self.objects.get(&id)
    }
}

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

struct FixtureModels {
    expected_object: ObjectId,
    base: ReferenceLitModel,
    calls: Cell<u32>,
    last_object: Cell<Option<ObjectId>>,
    last_request: Cell<Option<ModelRequest>>,
}

impl DynamicLitModelLookup for FixtureModels {
    fn dynamic_lit_model(
        &self,
        definition: &ObjectDefinition,
        request: ModelRequest,
    ) -> Option<&ReferenceLitModel> {
        self.calls.set(self.calls.get() + 1);
        self.last_object.set(Some(definition.identity.id));
        self.last_request.set(Some(request));
        (definition.identity.id == self.expected_object).then_some(&self.base)
    }
}

#[test]
fn active_morph_pose_then_contour_uses_private_lit_model_and_active_footprint()
-> Result<(), Box<dyn Error>> {
    let selector = VarpId::new(9);
    let source = object_definition(
        ObjectId::new(100),
        1,
        1,
        None,
        Some(ObjectMorphs {
            transform_varbit: None,
            transform_varp: Some(selector),
            transforms: vec![Some(ObjectId::new(200))],
            fallback: None,
        }),
    )?;
    let active = object_definition(ObjectId::new(200), 2, 1, Some(0), None)?;

    let mut definitions = FixtureDefinitions::default();
    definitions.objects.insert(active.identity.id, active);
    let mut state = FixtureState::default();
    state.varps.insert(selector, 0);

    let base = lit_model(vec![ModelPoint::new(0, -100, 0)], Some(vec![0]))?;
    let original_base = base.clone();
    let models = FixtureModels {
        expected_object: ObjectId::new(200),
        base,
        calls: Cell::new(0),
        last_object: Cell::new(None),
        last_request: Cell::new(None),
    };
    let heights = x_gradient_heights(6);
    let frame = LegacyAnimationFrame {
        skeleton: vec![LegacySkeletonTransform {
            transform_type: 1,
            labels: vec![0],
        }],
        transforms: vec![LegacyFrameTransform {
            skeleton_transform: 0,
            x: 128,
            y: 0,
            z: 0,
        }],
    };
    let request = ModelRequest {
        loc_type: LocType::new(10),
        orientation: 0,
    };

    let resolved = resolve_dynamic_model(
        &definitions,
        &state,
        &models,
        DynamicModelInput {
            placement: DynamicPlacementInput {
                source_definition: &source,
                loc_type: LocType::new(10),
                orientation: 0,
                tile: SceneTile::new(1, 1),
                heights: &heights,
                scene_width: 6,
                scene_height: 6,
                existing_wall_displacement: None,
            },
            model_request: request,
            legacy_frame: Some(&frame),
        },
    )?
    .ok_or("active morph unexpectedly produced no model")?;

    assert_eq!(resolved.active_definition.identity.id, ObjectId::new(200));
    assert_eq!(resolved.placement.rotated_definition_footprint.width, 2);
    assert_eq!(resolved.placement.rotated_definition_footprint.depth, 1);
    assert_eq!(resolved.placement.model_center.x.units(), 256);
    assert_eq!(resolved.placement.model_center.y.units(), 200);
    assert_eq!(resolved.placement.model_center.z.units(), 192);
    assert_eq!(models.calls.get(), 1);
    assert_eq!(models.last_object.get(), Some(ObjectId::new(200)));
    assert_eq!(models.last_request.get(), Some(request));

    let Cow::Owned(dynamic) = resolved.model else {
        panic!("animation plus contour must create a private runtime model");
    };
    assert_eq!(dynamic.vertices, vec![ModelPoint::new(128, 0, 0)]);
    assert_eq!(
        models.base, original_base,
        "cached lit base must remain immutable"
    );
    Ok(())
}

#[test]
fn null_morph_short_circuits_before_cached_model_lookup() -> Result<(), Box<dyn Error>> {
    let selector = VarpId::new(10);
    let source = object_definition(
        ObjectId::new(300),
        1,
        1,
        None,
        Some(ObjectMorphs {
            transform_varbit: None,
            transform_varp: Some(selector),
            transforms: vec![None],
            fallback: None,
        }),
    )?;
    let definitions = FixtureDefinitions::default();
    let mut state = FixtureState::default();
    state.varps.insert(selector, 0);
    let models = FixtureModels {
        expected_object: ObjectId::new(999),
        base: lit_model(vec![ModelPoint::new(0, 0, 0)], None)?,
        calls: Cell::new(0),
        last_object: Cell::new(None),
        last_request: Cell::new(None),
    };
    let heights = x_gradient_heights(4);

    let resolved = resolve_dynamic_model(
        &definitions,
        &state,
        &models,
        DynamicModelInput {
            placement: DynamicPlacementInput {
                source_definition: &source,
                loc_type: LocType::new(10),
                orientation: 0,
                tile: SceneTile::new(1, 1),
                heights: &heights,
                scene_width: 4,
                scene_height: 4,
                existing_wall_displacement: None,
            },
            model_request: ModelRequest {
                loc_type: LocType::new(10),
                orientation: 0,
            },
            legacy_frame: None,
        },
    )?;

    assert!(resolved.is_none());
    assert_eq!(models.calls.get(), 0);
    Ok(())
}

#[test]
fn no_pose_and_no_contour_borrows_cached_lit_base() -> Result<(), Box<dyn Error>> {
    let source = object_definition(ObjectId::new(400), 1, 1, None, None)?;
    let definitions = FixtureDefinitions::default();
    let state = FixtureState::default();
    let models = FixtureModels {
        expected_object: ObjectId::new(400),
        base: lit_model(vec![ModelPoint::new(7, -3, 5)], None)?,
        calls: Cell::new(0),
        last_object: Cell::new(None),
        last_request: Cell::new(None),
    };
    let heights = x_gradient_heights(4);

    let resolved = resolve_dynamic_model(
        &definitions,
        &state,
        &models,
        DynamicModelInput {
            placement: DynamicPlacementInput {
                source_definition: &source,
                loc_type: LocType::new(10),
                orientation: 0,
                tile: SceneTile::new(1, 1),
                heights: &heights,
                scene_width: 4,
                scene_height: 4,
                existing_wall_displacement: None,
            },
            model_request: ModelRequest {
                loc_type: LocType::new(10),
                orientation: 0,
            },
            legacy_frame: None,
        },
    )?
    .ok_or("static dynamic-model fast path unexpectedly absent")?;

    assert!(matches!(resolved.model, Cow::Borrowed(_)));
    assert_eq!(resolved.model.vertices, models.base.vertices);
    assert_eq!(models.calls.get(), 1);
    Ok(())
}

fn object_definition(
    id: ObjectId,
    size_x: u16,
    size_y: u16,
    contour_clip: Option<u32>,
    morphs: Option<ObjectMorphs>,
) -> Result<ObjectDefinition, Box<dyn Error>> {
    Ok(ObjectDefinition {
        identity: DefinitionIdentity::new(id, provenance()?),
        name: Some(format!("fixture-{}", id.get())),
        models: Some(ObjectModels::Untyped(vec![ModelId::new(77)])),
        size_x,
        size_y,
        placement: ObjectPlacementFlags {
            interact_type: 2,
            blocks_projectiles: true,
            clipped: true,
            model_clipped: false,
            obstructs_ground: false,
            solid: false,
        },
        decoration_displacement: 16,
        support_items: None,
        is_rotated: false,
        non_flat_shading: false,
        contour_clip,
        full_recolor: None,
        ground_raise: 0,
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
        actions: [None, None, None, None, None],
    })
}

fn lit_model(
    vertices: Vec<ModelPoint>,
    vertex_skins: Option<Vec<i32>>,
) -> Result<ReferenceLitModel, Box<dyn Error>> {
    let format = ModelFormatIdentity {
        encoding: ModelEncoding::Legacy,
        version: None,
    };
    let source = SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(77), provenance()?),
        format,
        vertices: vertices.clone(),
        faces: Vec::new(),
        face_colors: Vec::new(),
        default_priority: FacePriority::ZERO,
        face_render_types: None,
        face_priorities: None,
        face_alphas: None,
        face_textures: None,
        texture_face_selectors: None,
        face_biases: None,
        texture_triangles: Vec::new(),
        vertex_skins: vertex_skins.clone(),
        face_skins: None,
        skeletal_vertices: None,
    })?;

    Ok(ReferenceLitModel {
        identity: ModelSemanticIdentity::from_source(&source),
        format: Some(format),
        vertices,
        faces: Vec::new(),
        face_colors: Vec::new(),
        default_priority: FacePriority::ZERO,
        face_render_types: None,
        face_priorities: None,
        face_alphas: None,
        face_textures: None,
        texture_triangles: Vec::new(),
        texture_faces: None,
        face_biases: None,
        vertex_skins,
        face_skins: None,
        skeletal_vertices: None,
    })
}

fn x_gradient_heights(scene_extent: u32) -> Vec<Vec<i32>> {
    (0..=scene_extent)
        .map(|x| vec![(x as i32) * 100; (scene_extent + 1) as usize])
        .collect()
}

fn provenance() -> Result<TargetProvenance, Box<dyn Error>> {
    Ok(TargetProvenance::new(
        "osrs-live-241-2026-09-30-openrs2-2727",
        ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
        CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
        1,
    )?)
}
