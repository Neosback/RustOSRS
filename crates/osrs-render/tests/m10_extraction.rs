use osrs_core::{
    coords::{LocalCoord, LocalXZ, ModelPoint},
    definitions::DefinitionIdentity,
    ids::{ModelId, TextureId},
    lighting::{LightingParameters, light_model_data},
    model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts,
        TextureMappingParameters, TextureTriangle, TextureTriangleIndex, Triangle,
    },
    provenance::{CacheFingerprint, Digest256, ProfileDigest, TargetProvenance},
};
use osrs_render::{RenderMesh, RenderOrigin, RenderPlacement, RenderPoint};

fn provenance() -> Result<TargetProvenance, Box<dyn std::error::Error>> {
    Ok(TargetProvenance::new(
        "m10-test",
        ProfileDigest::from_digest(Digest256::from_bytes([0x11; 32])),
        CacheFingerprint::from_digest(Digest256::from_bytes([0x22; 32])),
        1,
    )?)
}

fn source_model(with_optional_metadata: bool) -> Result<SourceModel, Box<dyn std::error::Error>> {
    let default_priority =
        FacePriority::new(3).ok_or_else(|| std::io::Error::other("priority 3 must be valid"))?;
    let explicit_priority =
        FacePriority::new(9).ok_or_else(|| std::io::Error::other("priority 9 must be valid"))?;

    let (
        face_render_types,
        face_priorities,
        face_alphas,
        face_textures,
        texture_face_selectors,
        face_biases,
        texture_triangles,
    ) = if with_optional_metadata {
        (
            Some(vec![0]),
            Some(vec![explicit_priority]),
            Some(vec![-1]),
            Some(vec![Some(TextureId::new(42))]),
            Some(vec![Some(TextureTriangleIndex::new(0))]),
            Some(vec![7]),
            vec![TextureTriangle {
                render_type: 0,
                vertices: Triangle::new(0, 1, 2),
                mapping: TextureMappingParameters::default(),
            }],
        )
    } else {
        (None, None, None, None, None, None, Vec::new())
    };

    Ok(SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(77), provenance()?),
        format: ModelFormatIdentity {
            encoding: ModelEncoding::TrailerFfFd,
            version: Some(15),
        },
        vertices: vec![
            ModelPoint::new(0, 0, 0),
            ModelPoint::new(128, 0, 0),
            ModelPoint::new(0, 64, 128),
        ],
        faces: vec![Triangle::new(0, 1, 2)],
        face_colors: vec![500],
        default_priority,
        face_render_types,
        face_priorities,
        face_alphas,
        face_textures,
        texture_face_selectors,
        face_biases,
        texture_triangles,
        vertex_skins: None,
        face_skins: None,
        skeletal_vertices: None,
    })?)
}

#[test]
fn extraction_preserves_face_metadata_and_rebases_after_semantic_placement()
-> Result<(), Box<dyn std::error::Error>> {
    let source = source_model(true)?;
    let working = source.to_working_copy();
    let lit = light_model_data(&working, LightingParameters::for_loc(0, 0))?;

    assert_eq!(lit.face_render_types.as_deref(), Some(&[0][..]));
    assert_eq!(lit.face_colors[0].c, -2);

    let mesh = RenderMesh::extract(
        &lit,
        RenderPlacement::new(
            LocalXZ::new(LocalCoord::from_units(4096), LocalCoord::from_units(8192)),
            LocalCoord::from_units(-32),
        ),
        RenderOrigin::new(4000, -64, 8000),
    )?;

    assert_eq!(
        mesh.vertices(),
        &[
            RenderPoint {
                x: 96,
                y: 32,
                z: 192,
            },
            RenderPoint {
                x: 224,
                y: 32,
                z: 192,
            },
            RenderPoint {
                x: 96,
                y: 96,
                z: 320,
            },
        ]
    );
    assert_eq!(mesh.faces(), &[Triangle::new(0, 1, 2)]);
    assert_eq!(mesh.face_is_suppressed(0), Some(true));
    assert_eq!(mesh.face_is_suppressed(1), None);
    assert_eq!(mesh.default_priority().get(), 3);
    assert_eq!(mesh.face_render_types(), Some(&[0][..]));
    assert_eq!(
        mesh.face_priorities().map(|values| values[0].get()),
        Some(9)
    );
    assert_eq!(mesh.face_alphas(), Some(&[-1][..]));
    assert_eq!(mesh.face_textures(), Some(&[Some(TextureId::new(42))][..]));
    assert_eq!(mesh.texture_triangles(), &[Triangle::new(0, 1, 2)]);
    assert_eq!(mesh.texture_faces(), Some(&[Some(0)][..]));
    assert_eq!(mesh.face_biases(), Some(&[7][..]));
    Ok(())
}

#[test]
fn extraction_does_not_materialize_absent_optional_face_arrays()
-> Result<(), Box<dyn std::error::Error>> {
    let source = source_model(false)?;
    let working = source.to_working_copy();
    let lit = light_model_data(&working, LightingParameters::for_loc(0, 0))?;
    let mesh = RenderMesh::extract(
        &lit,
        RenderPlacement::new(
            LocalXZ::new(LocalCoord::from_units(0), LocalCoord::from_units(0)),
            LocalCoord::from_units(0),
        ),
        RenderOrigin::new(0, 0, 0),
    )?;

    assert_eq!(mesh.face_is_suppressed(0), Some(false));
    assert!(mesh.face_render_types().is_none());
    assert!(mesh.face_priorities().is_none());
    assert!(mesh.face_alphas().is_none());
    assert!(mesh.face_textures().is_none());
    assert!(mesh.texture_faces().is_none());
    assert!(mesh.face_biases().is_none());
    Ok(())
}
