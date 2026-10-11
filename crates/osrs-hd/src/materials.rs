//! Material resolution (`MaterialManager.loadMaterials` / `swapMaterials`): parent defaults,
//! conditional replacements (`HD_BRICK` replaces `BRICK` while "object textures" is on), the
//! vanilla-texture-to-material map and the material table order.
//!
//! Upstream: `rs117.hd.scene.MaterialManager` (BSD 2-Clause, see `THIRD_PARTY.md`).

use crate::{
    HdData, Material,
    expr::{Expression, Variables},
};
use std::collections::HashMap;

/// Name of the implicit material every table starts with.
pub const NONE: &str = "NONE";

/// Resolved materials.
///
/// Table index `0` is `NONE`; resolved definitions follow from index `1` in file order, with
/// replaced materials removed and the dummy `VANILLA_n` entries last.
#[derive(Debug, Clone, Default)]
pub struct MaterialSet {
    /// Definitions with parent defaults applied, without `NONE`; table index = position + 1.
    pub materials: Vec<Material>,
    /// Original name -> table index (after replacement; inactive replacements map to `0`).
    by_name: HashMap<String, usize>,
    /// Vanilla texture id -> table index.
    vanilla: HashMap<i32, usize>,
    /// Table indices of the dummy materials made for vanilla textures without a definition.
    fallback: std::collections::HashSet<usize>,
}

macro_rules! inherit {
    ($child:expr, $parent:expr, $($field:ident),+ $(,)?) => {
        $(if $child.$field.is_none() { $child.$field = $parent.$field.clone(); })+
    };
}

/// Apply `parent`'s values to every field `child` leaves unset (`GsonUtils.shallowInheritFrom`).
fn inherit(child: &mut Material, parent: &Material) {
    inherit!(
        child,
        parent,
        diffuse_map,
        normal_map,
        displacement_map,
        roughness_map,
        ambient_occlusion_map,
        flow_map,
        shadow_alpha_map,
        vanilla_texture_index,
        has_transparency,
        double_sided_faces,
        override_base_color,
        unlit,
        brightness,
        subsurface,
        displacement_scale,
        flow_map_strength,
        flow_map_duration,
        specular_strength,
        specular_gloss,
        scroll_speed,
        texture_scale,
        materials_to_replace,
        replacement_condition,
        description,
    );
    for (key, value) in &parent.extra {
        child
            .extra
            .entry(key.clone())
            .or_insert_with(|| value.clone());
    }
}

/// Resolve the parent chain of `index` (memoised; loops and unknown parents yield `None`).
fn resolve(
    index: usize,
    raw: &[Material],
    by_name: &HashMap<&str, usize>,
    resolved: &mut Vec<Option<Option<Material>>>,
    visiting: &mut Vec<usize>,
) -> Option<Material> {
    if let Some(done) = &resolved[index] {
        return done.clone();
    }
    if visiting.contains(&index) {
        return None;
    }
    visiting.push(index);
    let mut material = raw[index].clone();
    let outcome = match material.parent.clone().filter(|parent| parent != NONE) {
        None => Some(material),
        Some(parent_name) => match by_name.get(parent_name.as_str()) {
            Some(&parent) => resolve(parent, raw, by_name, resolved, visiting).map(|base| {
                inherit(&mut material, &base);
                material
            }),
            None => None,
        },
    };
    visiting.pop();
    resolved[index] = Some(outcome.clone());
    outcome
}

fn vanilla_index(material: &Material) -> Option<i32> {
    material
        .vanilla_texture_index
        .map(|index| index as i32)
        .filter(|index| *index >= 0)
}

impl MaterialSet {
    /// Resolve `data.materials` under `vars`. `vanilla_textures` lists the cache texture ids that
    /// exist; those without a definition get a transparent dummy material.
    pub fn build(
        data: &HdData,
        vars: &dyn Variables,
        vanilla_textures: impl IntoIterator<Item = i32>,
    ) -> Self {
        let by_name: HashMap<&str, usize> = data
            .materials
            .iter()
            .enumerate()
            .map(|(index, material)| (material.name.as_str(), index))
            .collect();
        let mut resolved = vec![None; data.materials.len()];
        let definitions: Vec<Material> = (0..data.materials.len())
            .filter_map(|index| resolve(index, &data.materials, &by_name, &mut resolved, &mut Vec::new()))
            .collect();

        let condition = |material: &Material| -> Option<Expression> {
            material
                .replacement_condition
                .as_ref()
                .and_then(|json| Expression::from_json(json).ok())
        };
        let conditions: Vec<Option<Expression>> = definitions.iter().map(condition).collect();
        let active = |index: usize| conditions[index].as_ref().is_none_or(|c| c.test(vars));
        let replaces = |index: usize, name: &str| {
            conditions[index].as_ref().is_some_and(|c| c.test(vars))
                && definitions[index]
                    .materials_to_replace
                    .as_ref()
                    .is_some_and(|names| names.iter().any(|n| n == name))
        };

        // Which definition each original name resolves to ("apply replacements top to bottom").
        let mut target: Vec<Option<usize>> = Vec::with_capacity(definitions.len());
        for (index, original) in definitions.iter().enumerate() {
            if !active(index) {
                target.push(None);
                continue;
            }
            let mut current = index;
            for other in 0..definitions.len() {
                if replaces(other, &definitions[current].name) {
                    current = other;
                }
            }
            let _ = original;
            target.push(Some(current));
        }

        let mut vanilla_target: HashMap<i32, usize> = HashMap::new();
        for (index, original) in definitions.iter().enumerate() {
            let Some(own) = vanilla_index(original) else {
                continue;
            };
            let parent_index = original
                .parent
                .as_deref()
                .and_then(|name| definitions.iter().find(|d| d.name == name))
                .and_then(vanilla_index);
            if parent_index == Some(own) {
                continue;
            }
            // Inactive replacements map to NONE and do not claim the texture.
            if let Some(replacement) = target[index] {
                vanilla_target.insert(own, replacement);
            }
        }

        // Distinct definitions that some name resolves to, in file order.
        let mut used = vec![false; definitions.len()];
        for replacement in target.iter().flatten() {
            used[*replacement] = true;
        }
        let mut table_of = vec![0usize; definitions.len()];
        let mut materials = Vec::new();
        for (index, definition) in definitions.iter().enumerate() {
            if used[index] {
                materials.push(definition.clone());
                table_of[index] = materials.len();
            }
        }
        let mut set = Self {
            materials,
            ..Self::default()
        };
        for (index, definition) in definitions.iter().enumerate() {
            let slot = target[index].map_or(0, |replacement| table_of[replacement]);
            set.by_name.insert(definition.name.clone(), slot);
        }
        for (texture, definition) in vanilla_target {
            set.vanilla.insert(texture, table_of[definition]);
        }
        for texture in vanilla_textures {
            if set.vanilla.contains_key(&texture) {
                continue;
            }
            set.materials.push(Material {
                name: format!("VANILLA_{texture}"),
                vanilla_texture_index: Some(texture as f32),
                has_transparency: Some(true),
                ..Material::default()
            });
            let slot = set.materials.len();
            set.by_name.insert(format!("VANILLA_{texture}"), slot);
            set.vanilla.insert(texture, slot);
            set.fallback.insert(slot);
        }
        set
    }

    /// Table size including `NONE`.
    pub fn len(&self) -> usize {
        self.materials.len() + 1
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// Table index of a material by its JSON name (`0` for `NONE`, unknown and inactive names).
    pub fn index_of(&self, name: &str) -> usize {
        self.by_name.get(name).copied().unwrap_or(0)
    }

    /// The material at a table index (`None` for `NONE`).
    pub fn get(&self, index: usize) -> Option<&Material> {
        index.checked_sub(1).and_then(|i| self.materials.get(i))
    }

    /// Table index replacing vanilla texture `id`, `0` when the texture is unknown.
    pub fn from_vanilla_texture(&self, id: i32) -> usize {
        self.vanilla.get(&id).copied().unwrap_or(0)
    }

    /// Dummy material standing in for a vanilla texture without an HD definition
    /// (`Material.isFallbackVanillaMaterial`).
    pub fn is_fallback_vanilla(&self, index: usize) -> bool {
        self.fallback.contains(&index)
    }

    /// Whether a material's faces are see-through (`Material.hasTransparency`).
    pub fn has_transparency(&self, index: usize) -> bool {
        self.get(index)
            .is_some_and(|m| m.has_transparency == Some(true))
    }

    pub fn double_sided(&self, index: usize) -> bool {
        self.get(index)
            .is_some_and(|m| m.double_sided_faces == Some(true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::HdSettings;

    fn material(name: &str, parent: Option<&str>) -> Material {
        Material {
            name: name.to_owned(),
            parent: parent.map(str::to_owned),
            ..Material::default()
        }
    }

    fn data(materials: Vec<Material>) -> HdData {
        HdData {
            materials,
            ..HdData::default()
        }
    }

    #[test]
    fn children_inherit_unset_fields() {
        let mut brick = material("BRICK", None);
        brick.specular_strength = Some(0.5);
        brick.vanilla_texture_index = Some(2.0);
        let mut light = material("LIGHT", Some("BRICK"));
        light.brightness = Some(3.0);
        let set = MaterialSet::build(&data(vec![brick, light]), &HdSettings::default(), []);
        let light = set.get(set.index_of("LIGHT")).unwrap();
        assert_eq!(light.specular_strength, Some(0.5));
        assert_eq!(light.brightness, Some(3.0));
        // An inherited vanilla index is not a replacement: BRICK still owns texture 2.
        assert_eq!(set.from_vanilla_texture(2), set.index_of("BRICK"));
    }

    #[test]
    fn model_textures_switches_the_replacement() {
        let mut vanilla = material("BRICK", None);
        vanilla.vanilla_texture_index = Some(2.0);
        let mut hd = material("HD_BRICK", None);
        hd.materials_to_replace = Some(vec!["BRICK".to_owned()]);
        hd.replacement_condition = Some(serde_json::json!("modelTextures"));
        let data = data(vec![vanilla, hd]);

        let on = MaterialSet::build(&data, &HdSettings::default(), [2]);
        assert_eq!(on.index_of("BRICK"), on.index_of("HD_BRICK"));
        assert_eq!(on.from_vanilla_texture(2), on.index_of("HD_BRICK"));

        let off = MaterialSet::build(
            &data,
            &HdSettings {
                model_textures: false,
                ..HdSettings::default()
            },
            [2],
        );
        assert_ne!(off.index_of("BRICK"), 0);
        assert_eq!(off.from_vanilla_texture(2), off.index_of("BRICK"));
        // The inactive replacement maps to NONE.
        assert_eq!(off.index_of("HD_BRICK"), 0);
    }

    #[test]
    fn unknown_vanilla_textures_get_a_fallback() {
        let set = MaterialSet::build(&data(vec![]), &HdSettings::default(), [7]);
        let index = set.from_vanilla_texture(7);
        assert!(index > 0 && set.is_fallback_vanilla(index));
        assert!(set.has_transparency(index));
        assert_eq!(set.from_vanilla_texture(8), 0);
    }

    #[test]
    fn parent_loops_drop_the_materials() {
        let set = MaterialSet::build(
            &data(vec![material("A", Some("B")), material("B", Some("A"))]),
            &HdSettings::default(),
            [],
        );
        assert_eq!(set.len(), 1);
    }
}
