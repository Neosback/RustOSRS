use osrs_core::{
    coords::{LocalCoord, LocalXZ, ModelPoint},
    definitions::{DefinitionIdentity, TextureDefinition},
    ids::{ModelId, SpriteId, TextureId},
    lighting::{LightingParameters, light_model_data},
    model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts,
        TextureMappingParameters, TextureTriangle, TextureTriangleIndex, Triangle,
    },
    provenance::{CacheFingerprint, Digest256, ProfileDigest, TargetProvenance},
};
use osrs_render::{
    MaterialTable, MaterialTableError, ReferenceFaceUvs, ReferenceUv, ReferenceUvMode, RenderMesh,
    RenderOrigin, RenderPlacement, RenderPoint, TextureAnimationVector, prepare_reference_face_uvs,
};

fn provenance(target: &str) -> Result<TargetProvenance, Box<dyn std::error::Error>> {
    Ok(TargetProvenance::new(
        target,
        ProfileDigest::from_digest(Digest256::from_bytes([0x31; 32])),
        CacheFingerprint::from_digest(Digest256::from_bytes([0x42; 32])),
        1,
    )?)
}

fn texture_definition(
    id: u32,
    target: &str,
    direction: u8,
    speed: u8,
    opaque: bool,
) -> Result<TextureDefinition, Box<dyn std::error::Error>> {
    Ok(TextureDefinition {
        identity: DefinitionIdentity::new(TextureId::new(id), provenance(target)?),
        average_rgb: (id as u16).wrapping_mul(17),
        opaque,
        source_sprites: vec![SpriteId::new(id.wrapping_add(1))],
        combine_modes: vec![1, 2],
        combine_directions: vec![3],
        color_transforms: vec![0x1122_3344],
        animation_direction: direction,
        animation_speed: speed,
    })
}

#[test]
fn material_table_is_deterministic_full_width_and_preserves_animation_inputs()
-> Result<(), Box<dyn std::error::Error>> {
    let high = texture_definition(u32::MAX, "target-a", 4, 7, true)?;
    let low = texture_definition(2, "target-a", 1, 3, false)?;
    let table = MaterialTable::from_definitions(&[high, low])?;

    assert_eq!(table.materials().len(), 2);
    assert_eq!(table.materials()[0].texture_id(), TextureId::new(2));
    assert_eq!(table.materials()[1].texture_id(), TextureId::new(u32::MAX));

    let low_handle = table
        .handle_for_texture(TextureId::new(2))
        .ok_or("missing low texture handle")?;
    let high_handle = table
        .handle_for_texture(TextureId::new(u32::MAX))
        .ok_or("missing full-width texture handle")?;
    assert_eq!(low_handle.get(), 0);
    assert_eq!(high_handle.get(), 1);

    let low_material = table.material(low_handle).ok_or("missing low material")?;
    assert!(!low_material.opaque());
    assert_eq!(low_material.source_sprites(), &[SpriteId::new(3)]);
    assert_eq!(low_material.combine_modes(), &[1, 2]);
    assert_eq!(low_material.combine_directions(), &[3]);
    assert_eq!(low_material.color_transforms(), &[0x1122_3344]);
    assert_eq!(low_material.animation_direction(), 1);
    assert_eq!(low_material.animation_speed(), 3);
    assert_eq!(
        low_material.animation_vector(),
        TextureAnimationVector {
            u_units_per_tick: 0,
            v_units_per_tick: -3,
        }
    );
    assert_eq!(
        low_material.animation_vector().uv_offset_at_tick(128),
        (0.0, -3.0)
    );

    let high_material = table.material(high_handle).ok_or("missing high material")?;
    assert_eq!(
        high_material.animation_vector(),
        TextureAnimationVector {
            u_units_per_tick: 7,
            v_units_per_tick: 0,
        }
    );
    assert_eq!(
        high_material.animation_vector().uv_offset_at_tick(128),
        (7.0, 0.0)
    );
    Ok(())
}

#[test]
fn material_table_rejects_duplicate_ids_and_mixed_targets() -> Result<(), Box<dyn std::error::Error>>
{
    let duplicate_a = texture_definition(5, "target-a", 0, 0, true)?;
    let duplicate_b = texture_definition(5, "target-a", 1, 1, true)?;
    assert_eq!(
        MaterialTable::from_definitions(&[duplicate_a, duplicate_b]),
        Err(MaterialTableError::DuplicateTextureId {
            texture_id: TextureId::new(5),
        })
    );

    let first = texture_definition(5, "target-a", 0, 0, true)?;
    let other = texture_definition(6, "target-b", 0, 0, true)?;
    assert_eq!(
        MaterialTable::from_definitions(&[first, other]),
        Err(MaterialTableError::MixedTargetProvenance {
            first_texture: TextureId::new(5),
            other_texture: TextureId::new(6),
        })
    );
    Ok(())
}

#[test]
fn no_texture_selector_uses_reference_canonical_uvs() -> Result<(), Box<dyn std::error::Error>> {
    let mesh = mesh(false)?;
    assert_eq!(
        prepare_reference_face_uvs(&mesh, 0, ReferenceUvMode::Direct)?,
        Some(ReferenceFaceUvs::CANONICAL)
    );
    Ok(())
}

#[test]
fn explicit_texture_triangle_matches_direct_and_projected_reference_oracle()
-> Result<(), Box<dyn std::error::Error>> {
    let mesh = mesh(true)?;

    let direct = prepare_reference_face_uvs(&mesh, 0, ReferenceUvMode::Direct)?
        .ok_or("textured explicit face produced no UVs")?;
    assert_eq!(
        direct,
        ReferenceFaceUvs {
            a: ReferenceUv { u: 0.0, v: 0.0 },
            b: ReferenceUv { u: 0.5, v: 0.0 },
            c: ReferenceUv { u: 0.0, v: 0.5 },
        }
    );

    let projected = prepare_reference_face_uvs(
        &mesh,
        0,
        ReferenceUvMode::ProjectedFromCamera {
            camera: RenderPoint { x: 0, y: 0, z: 8 },
        },
    )?
    .ok_or("textured projected face produced no UVs")?;
    assert_eq!(projected, ReferenceFaceUvs::CANONICAL);
    Ok(())
}

fn mesh(explicit_selector: bool) -> Result<RenderMesh, Box<dyn std::error::Error>> {
    let (vertices, face, texture_face_selectors, texture_triangles) = if explicit_selector {
        (
            vec![
                ModelPoint::new(0, 0, 0),
                ModelPoint::new(4, 0, 0),
                ModelPoint::new(0, 4, 0),
                ModelPoint::new(0, 0, 4),
                ModelPoint::new(2, 0, 4),
                ModelPoint::new(0, 2, 4),
            ],
            Triangle::new(3, 4, 5),
            Some(vec![Some(TextureTriangleIndex::new(0))]),
            vec![TextureTriangle {
                render_type: 0,
                vertices: Triangle::new(0, 1, 2),
                mapping: TextureMappingParameters::default(),
            }],
        )
    } else {
        (
            vec![
                ModelPoint::new(0, 0, 0),
                ModelPoint::new(4, 0, 0),
                ModelPoint::new(0, 4, 0),
            ],
            Triangle::new(0, 1, 2),
            None,
            Vec::new(),
        )
    };

    let source = SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(900), provenance("target-a")?),
        format: ModelFormatIdentity {
            encoding: ModelEncoding::TrailerFfFd,
            version: Some(15),
        },
        vertices,
        faces: vec![face],
        face_colors: vec![500],
        default_priority: FacePriority::ZERO,
        face_render_types: None,
        face_priorities: None,
        face_alphas: None,
        face_textures: Some(vec![Some(TextureId::new(77))]),
        texture_face_selectors,
        face_biases: None,
        texture_triangles,
        vertex_skins: None,
        face_skins: None,
        skeletal_vertices: None,
    })?;
    let working = source.to_working_copy();
    let lit = light_model_data(&working, LightingParameters::for_loc(0, 0))?;
    Ok(RenderMesh::extract(
        &lit,
        RenderPlacement::new(
            LocalXZ::new(LocalCoord::from_units(0), LocalCoord::from_units(0)),
            LocalCoord::from_units(0),
        ),
        RenderOrigin::new(0, 0, 0),
    )?)
}
