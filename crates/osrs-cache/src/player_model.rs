//! Cache-backed immutable player base model assembly, following the client's
//! `PlayerComposition.getModel`: each slot's kit or worn-item models are recoloured by their own
//! definition and joined in slot order; the body colours then recolour the whole, and the model
//! is lit with the player constants. Posing is a per-instance step after this.
use crate::model_repository::{ModelRepositoryError, ModelSourceRepository};
use osrs_core::{
    definitions::{RecolorPair, RetexturePair},
    ids::{ItemId, KitId, ModelId, TextureId},
    lighting::{LightingParameters, ReferenceLitModel, light_model_data},
    model::WorkingModel,
    model::{SourceModel, SourceModelParts},
    model_construction::combine_source_models,
    player::{ItemDefinition, KitDefinition, PlayerAppearance, SlotContent, body_recolors},
};
use std::{collections::BTreeMap, sync::Arc};

/// Where raw models come from; the cache in production, fixtures in tests.
pub trait ModelLoader {
    fn load(&mut self, id: ModelId) -> Result<Arc<SourceModel>, String>;
}

impl ModelLoader for ModelSourceRepository {
    fn load(&mut self, id: ModelId) -> Result<Arc<SourceModel>, String> {
        self.load_unmirrored(id)
            .map_err(|e: ModelRepositoryError| e.to_string())
    }
}

/// Lighting constants of the client's player models.
const PLAYER_LIGHTING: LightingParameters = LightingParameters {
    ambient: 64,
    contrast: 850,
    light_x: -30,
    light_y: -50,
    light_z: -30,
};

/// A copy of `source` with face colours and textures replaced and every vertex moved down by
/// `y_offset` (the client's `recolor`, `retexture` and `changeOffset`). The original is untouched.
pub fn adjusted(
    source: &SourceModel,
    recolors: &[RecolorPair],
    retextures: &[RetexturePair],
    y_offset: i32,
) -> Result<SourceModel, String> {
    let mut colors = source.face_colors().to_vec();
    for pair in recolors {
        for color in &mut colors {
            if *color == pair.from {
                *color = pair.to;
            }
        }
    }
    let mut textures = source.face_textures().map(<[_]>::to_vec);
    if let Some(textures) = textures.as_mut() {
        for pair in retextures {
            for texture in textures.iter_mut() {
                if *texture == Some(TextureId::new(u32::from(pair.from))) {
                    *texture = Some(TextureId::new(u32::from(pair.to)));
                }
            }
        }
    }
    let mut vertices = source.vertices().to_vec();
    for vertex in &mut vertices {
        vertex.y += y_offset;
    }
    SourceModel::from_parts(SourceModelParts {
        identity: source.identity().clone(),
        format: source.format(),
        vertices,
        faces: source.faces().to_vec(),
        face_colors: colors,
        default_priority: source.default_priority(),
        face_render_types: source.face_render_types().map(<[_]>::to_vec),
        face_priorities: source.face_priorities().map(<[_]>::to_vec),
        face_alphas: source.face_alphas().map(<[_]>::to_vec),
        face_textures: textures,
        texture_face_selectors: source.texture_face_selectors().map(<[_]>::to_vec),
        face_biases: source.face_biases().map(<[_]>::to_vec),
        texture_triangles: source.texture_triangles().to_vec(),
        vertex_skins: source.vertex_skins().map(<[_]>::to_vec),
        face_skins: source.face_skins().map(<[i32]>::to_vec),
        skeletal_vertices: source.skeletal_vertices().map(<[_]>::to_vec),
    })
    .map_err(|e| e.to_string())
}

/// Every model reference the appearance needs, in assembly order, with its own recolours.
struct Part {
    models: Vec<ModelId>,
    recolors: Vec<RecolorPair>,
    retextures: Vec<RetexturePair>,
    offset: i32,
}

fn parts(
    appearance: &PlayerAppearance,
    kits: &BTreeMap<KitId, Arc<KitDefinition>>,
    items: &BTreeMap<ItemId, Arc<ItemDefinition>>,
) -> Result<Vec<Part>, String> {
    let mut out = Vec::new();
    for content in appearance.slots {
        match content {
            SlotContent::Empty => {}
            SlotContent::Kit(id) => {
                let kit = kits.get(&id).ok_or_else(|| format!("missing kit {id}"))?;
                if !kit.models.is_empty() {
                    out.push(Part {
                        models: kit.models.clone(),
                        recolors: kit.recolors.clone(),
                        retextures: kit.retextures.clone(),
                        offset: 0,
                    });
                }
            }
            SlotContent::Item(id) => {
                let item = items.get(&id).ok_or_else(|| format!("missing item {id}"))?;
                let worn = item.worn_models(appearance.gender);
                // The first model decides: an item without it shows nothing in this slot.
                let Some(first) = worn.models[0] else {
                    continue;
                };
                let mut models = vec![first];
                models.extend(worn.models[1..].iter().flatten().copied());
                out.push(Part {
                    models,
                    recolors: item.recolors.clone(),
                    retextures: item.retextures.clone(),
                    offset: i32::from(worn.offset),
                });
            }
        }
    }
    Ok(out)
}

/// Build the unposed, lit base model for `appearance`. `None` when nothing in it has a model.
/// Failure never substitutes another model or a partial assembly.
pub fn build_player_base(
    loader: &mut impl ModelLoader,
    appearance: &PlayerAppearance,
    kits: &BTreeMap<KitId, Arc<KitDefinition>>,
    items: &BTreeMap<ItemId, Arc<ItemDefinition>>,
) -> Result<Option<ReferenceLitModel>, String> {
    let mut sources = Vec::new();
    for part in parts(appearance, kits, items)? {
        for id in &part.models {
            let raw = loader.load(*id)?;
            sources.push(adjusted(
                &raw,
                &part.recolors,
                &part.retextures,
                part.offset,
            )?);
        }
    }
    if sources.is_empty() {
        return Ok(None);
    }
    let refs: Vec<&SourceModel> = sources.iter().collect();
    let assembled = combine_source_models(&refs).map_err(|e| e.to_string())?;
    let mut model = WorkingModel::from_assembled(&assembled).map_err(|e| e.to_string())?;
    for pair in body_recolors(appearance.colours) {
        for color in model.face_colors_mut() {
            if *color == pair.from {
                *color = pair.to;
            }
        }
    }
    light_model_data(&model, PLAYER_LIGHTING)
        .map(Some)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
