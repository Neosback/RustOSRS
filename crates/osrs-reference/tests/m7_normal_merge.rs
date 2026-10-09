use osrs_core::{
    coords::ModelPoint,
    definitions::{DefinitionIdentity, ModelTranslation},
    ids::ModelId,
    model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, ModelNormalState, SourceModel,
        SourceModelParts, Triangle, WorkingModel,
    },
    normals::merge_model_normals,
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};
use osrs_reference::schema::{
    NormalizedExpectedCase, NormalizedFixtureExpected, NormalizedFixtureInput, NormalizedInputCase,
    NormalizedNormalModel, NormalizedVertexNormal,
};
use std::{error::Error, io};

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str = "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

const CASES: [(&[u8], &[u8]); 3] = [
    (
        include_bytes!(
            "../../../reference-fixtures/normals/merge_coincident_triangle_hide_false.input.json"
        ),
        include_bytes!(
            "../../../reference-fixtures/normals/merge_coincident_triangle_hide_false.expected.json"
        ),
    ),
    (
        include_bytes!(
            "../../../reference-fixtures/normals/merge_coincident_triangle_hide_true.input.json"
        ),
        include_bytes!(
            "../../../reference-fixtures/normals/merge_coincident_triangle_hide_true.expected.json"
        ),
    ),
    (
        include_bytes!("../../../reference-fixtures/normals/merge_translated_negative.input.json"),
        include_bytes!(
            "../../../reference-fixtures/normals/merge_translated_negative.expected.json"
        ),
    ),
];

#[test]
fn source_pinned_normal_merge_fixtures_match_production_exactly() -> Result<(), Box<dyn Error>> {
    for (input_bytes, expected_bytes) in CASES {
        let input = NormalizedFixtureInput::parse(input_bytes)?;
        let expected = NormalizedFixtureExpected::parse(expected_bytes)?;
        let NormalizedInputCase::NormalMerge {
            left,
            right,
            translation,
            hide_matched_faces,
        } = input.case
        else {
            return Err(io::Error::other("M7 fixture input must be normal_merge").into());
        };

        let left_source = source_model(8_001, left)?;
        let right_source = source_model(8_002, right)?;
        let left_source_before = left_source.clone();
        let right_source_before = right_source.clone();
        let mut left_working = left_source.to_working_copy();
        let mut right_working = right_source.to_working_copy();

        let outcome = merge_model_normals(
            &mut left_working,
            &mut right_working,
            ModelTranslation {
                x: translation.x,
                y: translation.y,
                z: translation.z,
            },
            hide_matched_faces,
        );

        assert_eq!(left_source, left_source_before);
        assert_eq!(right_source, right_source_before);
        if merged_vertex_normals(&left_working).is_some() {
            assert!(outcome.matched_vertex_pairs() >= 3);
        } else {
            assert_eq!(outcome.matched_vertex_pairs(), 0);
        }

        let actual = NormalizedExpectedCase::NormalMerge {
            left_merged_vertex_normals: merged_vertex_normals(&left_working),
            right_merged_vertex_normals: merged_vertex_normals(&right_working),
            left_face_render_types: normalized_face_render_types(&left_working)?,
            right_face_render_types: normalized_face_render_types(&right_working)?,
        };
        assert_eq!(actual, expected.case);
    }
    Ok(())
}

fn source_model(
    model_id: u32,
    model: NormalizedNormalModel,
) -> Result<SourceModel, Box<dyn Error>> {
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
        identity: DefinitionIdentity::new(ModelId::new(model_id), fixture_provenance()?),
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

fn merged_vertex_normals(model: &WorkingModel) -> Option<Vec<Option<NormalizedVertexNormal>>> {
    let ModelNormalState::Computed(normals) = model.normal_state() else {
        return None;
    };
    normals.merged_vertex_normals.as_ref().map(|values| {
        values
            .iter()
            .map(|normal| {
                normal.map(|normal| NormalizedVertexNormal {
                    x: normal.x,
                    y: normal.y,
                    z: normal.z,
                    magnitude: normal.magnitude,
                })
            })
            .collect()
    })
}

fn normalized_face_render_types(model: &WorkingModel) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    model
        .face_render_types()
        .map(|types| {
            types
                .iter()
                .copied()
                .map(|value| {
                    u8::try_from(value)
                        .map_err(|_| io::Error::other("working face render type is negative"))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
        .map_err(Into::into)
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
