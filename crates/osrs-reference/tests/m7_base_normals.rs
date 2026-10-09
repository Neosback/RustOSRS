use osrs_core::{
    coords::ModelPoint,
    definitions::DefinitionIdentity,
    ids::ModelId,
    model::{FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts, Triangle},
    normals::calculate_base_normals,
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};
use osrs_reference::schema::{
    NormalizedExpectedCase, NormalizedFixtureExpected, NormalizedFixtureInput, NormalizedInputCase,
    NormalizedNormalModel, NormalizedNormalVector, NormalizedVertexNormal,
};
use std::{error::Error, io};

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str = "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

const CASES: [(&[u8], &[u8]); 2] = [
    (
        include_bytes!("../../../reference-fixtures/normals/base_smooth_triangle.input.json"),
        include_bytes!("../../../reference-fixtures/normals/base_smooth_triangle.expected.json"),
    ),
    (
        include_bytes!("../../../reference-fixtures/normals/base_flat_triangle.input.json"),
        include_bytes!("../../../reference-fixtures/normals/base_flat_triangle.expected.json"),
    ),
];

#[test]
fn source_pinned_base_normal_fixtures_match_production_exactly() -> Result<(), Box<dyn Error>> {
    for (input_bytes, expected_bytes) in CASES {
        let input = NormalizedFixtureInput::parse(input_bytes)?;
        let expected = NormalizedFixtureExpected::parse(expected_bytes)?;
        let NormalizedInputCase::BaseNormals { model } = input.case else {
            return Err(io::Error::other("M7 fixture input must be base_normals").into());
        };

        let source = source_model(model)?;
        let source_before = source.clone();
        let working = source.to_working_copy();
        let normals = calculate_base_normals(&working);
        assert_eq!(source, source_before, "base normal generation mutated source");

        let actual = NormalizedExpectedCase::BaseNormals {
            vertex_normals: normals
                .base_vertex_normals
                .into_iter()
                .map(|normal| NormalizedVertexNormal {
                    x: normal.x,
                    y: normal.y,
                    z: normal.z,
                    magnitude: normal.magnitude,
                })
                .collect(),
            face_normals: normals.face_normals.map(|face_normals| {
                face_normals
                    .into_iter()
                    .map(|normal| {
                        normal.map(|normal| NormalizedNormalVector {
                            x: normal.x,
                            y: normal.y,
                            z: normal.z,
                        })
                    })
                    .collect()
            }),
        };
        assert_eq!(actual, expected.case);
    }
    Ok(())
}

fn source_model(model: NormalizedNormalModel) -> Result<SourceModel, Box<dyn Error>> {
    let vertices = model
        .vertices
        .into_iter()
        .map(|point| ModelPoint::new(point.x, point.y, point.z))
        .collect();
    let faces: Vec<Triangle> = model
        .faces
        .into_iter()
        .map(|face| Triangle::new(face.a, face.b, face.c))
        .collect();
    let face_render_types = model
        .face_render_types
        .map(|types| {
            types
                .into_iter()
                .map(|value| {
                    i8::try_from(value)
                        .map_err(|_| io::Error::other("fixture face render type exceeds i8"))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    Ok(SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(7_001), fixture_provenance()?),
        format: ModelFormatIdentity {
            encoding: ModelEncoding::TrailerFfFd,
            version: None,
        },
        vertices,
        face_colors: vec![0; faces.len()],
        faces,
        default_priority: FacePriority::ZERO,
        face_render_types,
        face_priorities: None,
        face_alphas: None,
        face_textures: None,
        texture_face_selectors: None,
        face_biases: None,
        texture_triangles: Vec::new(),
        vertex_skins: None,
        face_skins: None,
        skeletal_vertices: None,
    })?)
}

fn fixture_provenance() -> Result<TargetProvenance, Box<dyn Error>> {
    let profile_digest = ProfileDigest::from_lower_hex(PROFILE_DIGEST)?;
    let cache_fingerprint = CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?;
    Ok(TargetProvenance::new(
        "osrs-live-build-241",
        profile_digest,
        cache_fingerprint,
        1,
    )?)
}
