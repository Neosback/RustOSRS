//! Normalize production normal output without duplicating its integer algorithms.

use super::{FixtureRunError, failure, source_model};
use crate::schema::{
    NormalizedExpectedCase, NormalizedModelTranslation, NormalizedNormalModel,
    NormalizedNormalVector, NormalizedVertexNormal,
};
use osrs_core::{
    definitions::ModelTranslation,
    model::{ModelNormalState, SourceModel, VertexNormal, WorkingModel},
    normals::{calculate_base_normals, merge_model_normals},
};

pub(super) fn execute_base(
    fixture_id: &str,
    model: NormalizedNormalModel,
) -> Result<NormalizedExpectedCase, FixtureRunError> {
    let source = normal_source(fixture_id, model)?;
    let normals = calculate_base_normals(&source.to_working_copy());
    Ok(NormalizedExpectedCase::BaseNormals {
        vertex_normals: normals
            .base_vertex_normals
            .into_iter()
            .map(normalize_vertex)
            .collect(),
        face_normals: normals.face_normals.map(|faces| {
            faces
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
    })
}

pub(super) fn execute_merge(
    fixture_id: &str,
    left: NormalizedNormalModel,
    right: NormalizedNormalModel,
    translation: NormalizedModelTranslation,
    hide_matched_faces: bool,
) -> Result<NormalizedExpectedCase, FixtureRunError> {
    let mut left = normal_source(fixture_id, left)?.to_working_copy();
    let mut right = normal_source(fixture_id, right)?.to_working_copy();
    merge_model_normals(
        &mut left,
        &mut right,
        ModelTranslation {
            x: translation.x,
            y: translation.y,
            z: translation.z,
        },
        hide_matched_faces,
    );
    Ok(NormalizedExpectedCase::NormalMerge {
        left_merged_vertex_normals: merged_normals(fixture_id, &left)?,
        right_merged_vertex_normals: merged_normals(fixture_id, &right)?,
        left_face_render_types: render_types(fixture_id, &left)?,
        right_face_render_types: render_types(fixture_id, &right)?,
    })
}

fn normal_source(
    fixture_id: &str,
    model: NormalizedNormalModel,
) -> Result<SourceModel, FixtureRunError> {
    let types = model
        .face_render_types
        .map(|types| {
            types
                .into_iter()
                .map(|value| {
                    i8::try_from(value)
                        .map_err(|_| failure(fixture_id, "fixture face render type exceeds i8"))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let colors = vec![0; model.faces.len()];
    source_model(model.vertices, model.faces, colors, None, types)
        .map_err(|detail| failure(fixture_id, detail))
}

fn merged_normals(
    fixture_id: &str,
    model: &WorkingModel,
) -> Result<Option<Vec<Option<NormalizedVertexNormal>>>, FixtureRunError> {
    let ModelNormalState::Computed(normals) = model.normal_state() else {
        return Err(failure(
            fixture_id,
            "normal merge left base normals uncomputed",
        ));
    };
    Ok(normals.merged_vertex_normals.as_ref().map(|values| {
        values
            .iter()
            .map(|normal| normal.map(normalize_vertex))
            .collect()
    }))
}

fn render_types(
    fixture_id: &str,
    model: &WorkingModel,
) -> Result<Option<Vec<u8>>, FixtureRunError> {
    model
        .face_render_types()
        .map(|types| {
            types
                .iter()
                .map(|&value| {
                    u8::try_from(value)
                        .map_err(|_| failure(fixture_id, "working face render type is negative"))
                })
                .collect()
        })
        .transpose()
}

const fn normalize_vertex(normal: VertexNormal) -> NormalizedVertexNormal {
    NormalizedVertexNormal {
        x: normal.x,
        y: normal.y,
        z: normal.z,
        magnitude: normal.magnitude,
    }
}
