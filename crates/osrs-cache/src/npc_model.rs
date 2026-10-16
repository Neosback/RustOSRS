//! Cache-backed immutable NPC base model assembly. Posing and definition scale
//! are per-instance operations after lighting, following NPCComposition.getModel.
use crate::model_repository::ModelSourceRepository;
use osrs_core::{
    entity::NpcDefinition,
    ids::TextureId,
    lighting::{LightingParameters, ReferenceLitModel, light_model_data},
    model::WorkingModel,
    model_construction::combine_source_models,
};
use std::sync::Arc;

/// Build an unposed, unscaled base using the canonical model decoder and merge.
/// Failure never substitutes another NPC or partially assembled model.
pub fn build_npc_base(
    repository: &mut ModelSourceRepository,
    definition: &NpcDefinition,
) -> Result<Option<ReferenceLitModel>, String> {
    if definition.body_models.is_empty() {
        return Ok(None);
    }
    let mut sources = Vec::with_capacity(definition.body_models.len());
    for id in &definition.body_models {
        sources.push(repository.load_unmirrored(*id).map_err(|e| e.to_string())?);
    }
    if sources
        .iter()
        .any(|source| source.identity().provenance != definition.identity.provenance)
    {
        return Err("NPC/model target provenance mismatch".into());
    }
    let refs: Vec<_> = sources.iter().map(Arc::as_ref).collect();
    let assembled = combine_source_models(&refs).map_err(|e| e.to_string())?;
    let mut model = WorkingModel::from_assembled(&assembled).map_err(|e| e.to_string())?;
    for pair in &definition.recolors {
        for color in model.face_colors_mut() {
            if *color == pair.from {
                *color = pair.to;
            }
        }
    }
    if let Some(color) = definition.full_recolor {
        model.face_colors_mut().fill(color);
    }
    if let Some(textures) = model.face_textures_mut() {
        for pair in &definition.retextures {
            for texture in textures.iter_mut() {
                if *texture == Some(TextureId::new(u32::from(pair.from))) {
                    *texture = Some(TextureId::new(u32::from(pair.to)));
                }
            }
        }
    }
    light_model_data(
        &model,
        LightingParameters {
            ambient: i32::from(definition.ambient) + 64,
            contrast: i32::from(definition.contrast) + 850,
            light_x: -30,
            light_y: -50,
            light_z: -30,
        },
    )
    .map(Some)
    .map_err(|e| e.to_string())
}
