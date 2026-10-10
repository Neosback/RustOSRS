use osrs_core::coords::ModelPoint;
use osrs_core::definitions::{
    DefinitionIdentity, LocType, ModelScale, ModelTranslation, ObjectDefinition,
    ObjectPlacementFlags, RecolorPair, RetexturePair,
};
use osrs_core::ids::{ModelId, ObjectId, TextureId};
use osrs_core::model::{
    FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts, Triangle,
};
use osrs_core::model_construction::{AssembledModel, apply_object_model_instance_transforms};
use osrs_core::provenance::{CacheFingerprint, ProfileDigest, TargetProvenance};

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str = "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

fn provenance() -> Result<TargetProvenance, Box<dyn std::error::Error>> {
    Ok(TargetProvenance::new(
        "osrs-live-241-2026-09-30-openrs2-2727",
        ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
        CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
        1,
    )?)
}

fn definition() -> Result<ObjectDefinition, Box<dyn std::error::Error>> {
    Ok(ObjectDefinition {
        identity: DefinitionIdentity::new(ObjectId::new(1), provenance()?),
        name: Some("transform-fixture".to_owned()),
        models: None,
        size_x: 1,
        size_y: 1,
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
        contour_clip: None,
        full_recolor: None,
        ground_raise: 0,
        animation: None,
        ambient: 0,
        contrast: 0,
        scale: ModelScale::IDENTITY,
        translation: ModelTranslation::ZERO,
        recolors: Vec::new(),
        retextures: Vec::new(),
        morphs: None,
        map_scene: None,
        map_icon: None,
        category: None,
        actions: [None, None, None, None, None],
    })
}

fn source_model(
    id: u32,
    vertices: Vec<ModelPoint>,
    faces: Vec<Triangle>,
    face_colors: Vec<u16>,
    face_textures: Option<Vec<Option<TextureId>>>,
) -> Result<SourceModel, Box<dyn std::error::Error>> {
    Ok(SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(id), provenance()?),
        format: ModelFormatIdentity {
            encoding: ModelEncoding::TrailerFfFd,
            version: None,
        },
        vertices,
        faces,
        face_colors,
        default_priority: FacePriority::ZERO,
        face_render_types: None,
        face_priorities: None,
        face_alphas: None,
        face_textures,
        texture_face_selectors: None,
        face_biases: None,
        texture_triangles: Vec::new(),
        vertex_skins: None,
        face_skins: None,
        skeletal_vertices: None,
    })?)
}

#[test]
fn combined_pipeline_matches_reference_order_exactly() -> Result<(), Box<dyn std::error::Error>> {
    let source = source_model(
        10,
        vec![
            ModelPoint::new(128, 64, 0),
            ModelPoint::new(0, 0, 0),
            ModelPoint::new(0, 128, 0),
        ],
        vec![Triangle::new(0, 1, 2)],
        vec![100],
        Some(vec![Some(TextureId::new(7))]),
    )?;
    let original_vertices = source.vertices().to_vec();
    let original_colors = source.face_colors().to_vec();
    let original_textures = source.face_textures().map(ToOwned::to_owned);

    let mut object = definition()?;
    object.recolors = vec![
        RecolorPair { from: 100, to: 200 },
        RecolorPair { from: 200, to: 300 },
    ];
    object.retextures = vec![
        RetexturePair { from: 7, to: 8 },
        RetexturePair { from: 8, to: 9 },
    ];
    object.scale = ModelScale {
        x: 256,
        y: 64,
        z: 128,
    };
    object.translation = ModelTranslation {
        x: 10,
        y: -5,
        z: 20,
    };

    let mut model = AssembledModel::from_source(&source);
    apply_object_model_instance_transforms(&mut model, &object, LocType::new(4), 5);

    assert_eq!(
        model.vertices(),
        &[
            ModelPoint::new(-262, 27, -115),
            ModelPoint::new(-80, -5, -25),
            ModelPoint::new(-80, 59, -25),
        ]
    );
    assert_eq!(model.face_colors(), &[300]);
    assert_eq!(model.face_textures(), Some(&[Some(TextureId::new(9))][..]));

    assert_eq!(source.vertices(), original_vertices.as_slice());
    assert_eq!(source.face_colors(), original_colors.as_slice());
    assert_eq!(source.face_textures(), original_textures.as_deref());
    Ok(())
}

#[test]
fn ordinary_orientation_masks_to_quarter_turns() -> Result<(), Box<dyn std::error::Error>> {
    let source = source_model(
        11,
        vec![ModelPoint::new(3, 4, 5)],
        Vec::new(),
        Vec::new(),
        None,
    )?;
    let object = definition()?;

    for (orientation, expected) in [
        (0, ModelPoint::new(3, 4, 5)),
        (1, ModelPoint::new(5, 4, -3)),
        (2, ModelPoint::new(-3, 4, -5)),
        (3, ModelPoint::new(-5, 4, 3)),
        (4, ModelPoint::new(3, 4, 5)),
    ] {
        let mut model = AssembledModel::from_source(&source);
        apply_object_model_instance_transforms(&mut model, &object, LocType::new(10), orientation);
        assert_eq!(model.vertices(), &[expected]);
    }

    let min_source = source_model(
        12,
        vec![ModelPoint::new(i32::MIN, 0, 0)],
        Vec::new(),
        Vec::new(),
        None,
    )?;
    let mut min_model = AssembledModel::from_source(&min_source);
    apply_object_model_instance_transforms(&mut min_model, &object, LocType::new(10), 1);
    assert_eq!(min_model.vertices(), &[ModelPoint::new(0, 0, i32::MIN)]);
    Ok(())
}

#[test]
fn retexture_preserves_signed_short_sentinel_behavior() -> Result<(), Box<dyn std::error::Error>> {
    let source = source_model(
        13,
        vec![
            ModelPoint::new(0, 0, 0),
            ModelPoint::new(1, 0, 0),
            ModelPoint::new(0, 1, 0),
            ModelPoint::new(1, 1, 0),
        ],
        vec![Triangle::new(0, 1, 2), Triangle::new(1, 3, 2)],
        vec![1, 1],
        Some(vec![None, Some(TextureId::new(7))]),
    )?;
    let mut object = definition()?;
    object.retextures = vec![
        RetexturePair {
            from: u16::MAX,
            to: 5,
        },
        RetexturePair {
            from: 7,
            to: u16::MAX,
        },
    ];

    let mut model = AssembledModel::from_source(&source);
    apply_object_model_instance_transforms(&mut model, &object, LocType::new(10), 0);
    assert_eq!(
        model.face_textures(),
        Some(&[Some(TextureId::new(5)), None][..])
    );
    Ok(())
}

#[test]
fn two_instances_transform_independently_from_one_source() -> Result<(), Box<dyn std::error::Error>>
{
    let source = source_model(
        14,
        vec![ModelPoint::new(10, 0, 20)],
        Vec::new(),
        Vec::new(),
        None,
    )?;
    let original = source.vertices().to_vec();
    let object = definition()?;

    let mut first = AssembledModel::from_source(&source);
    let mut second = AssembledModel::from_source(&source);
    apply_object_model_instance_transforms(&mut first, &object, LocType::new(10), 1);
    apply_object_model_instance_transforms(&mut second, &object, LocType::new(10), 3);

    assert_eq!(first.vertices(), &[ModelPoint::new(20, 0, -10)]);
    assert_eq!(second.vertices(), &[ModelPoint::new(-20, 0, 10)]);
    assert_eq!(source.vertices(), original.as_slice());
    Ok(())
}
