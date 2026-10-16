//! Authored appearance, independent of map semantics and GPU resources.
mod number;
mod pack;
mod resolve;
pub mod shader;
pub use number::ThemeNumber;
use osrs_core::{
    coords::{MapTile, PlaneIndex},
    ids::{FloorOverlayId, FloorUnderlayId, ObjectId},
};
pub use pack::{ThemeAsset, ThemePack};
pub use resolve::{AppearanceQuery, ResolvedTheme};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const THEME_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ThemeOverrides {
    pub ambient_color: Option<[u8; 3]>,
    pub ambient_strength: Option<ThemeNumber>,
    pub sunlight_color: Option<[u8; 3]>,
    pub sunlight_strength: Option<ThemeNumber>,
    pub fog_color: Option<[u8; 3]>,
    pub fog_depth: Option<ThemeNumber>,
    pub water_color: Option<[u8; 3]>,
    pub exposure: Option<ThemeNumber>,
    pub brightness: Option<ThemeNumber>,
    pub normal_mapping: Option<bool>,
    pub parallax_mapping: Option<bool>,
    pub underwater_caustics: Option<bool>,
    /// Named material replacement, resolved by the existing material table.
    pub material: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeTarget {
    Global,
    Area {
        min: MapTile,
        max: MapTile,
        min_plane: PlaneIndex,
        max_plane: PlaneIndex,
    },
    Underlay(FloorUnderlayId),
    Overlay(FloorOverlayId),
    Object(ObjectId),
    Placement {
        tile: MapTile,
        plane: PlaneIndex,
        object: ObjectId,
        kind: u8,
        orientation: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub target: ThemeTarget,
    pub values: ThemeOverrides,
}

/// A material derived from an existing HD material, with optional image inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeMaterial {
    pub name: String,
    pub parent: String,
    pub diffuse: Option<String>,
    pub normal: Option<String>,
    pub height: Option<String>,
    pub roughness: Option<String>,
    pub ambient_occlusion: Option<String>,
    pub flow: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeDefinition {
    pub version: u32,
    pub id: String,
    pub revision: u32,
    pub name: String,
    /// Inherit stock appearance; custom overrides never modify imported upstream data.
    pub overrides: ThemeOverrides,
    pub rules: Vec<ThemeRule>,
    pub materials: Vec<ThemeMaterial>,
    pub shaders: Vec<shader::ShaderExtension>,
    pub provenance: Vec<String>,
    pub attribution: String,
}

impl Default for ThemeDefinition {
    fn default() -> Self {
        Self::stock()
    }
}
impl ThemeDefinition {
    pub fn stock() -> Self {
        Self {
            version: THEME_VERSION,
            id: "stock-117hd".into(),
            revision: 1,
            name: "Stock 117HD".into(),
            overrides: ThemeOverrides::default(),
            rules: Vec::new(),
            materials: Vec::new(),
            shaders: Vec::new(),
            provenance: Vec::new(),
            attribution: "Derived from 117HD/RLHD (BSD-2-Clause); see THIRD_PARTY.md".into(),
        }
    }
    pub fn duplicate(&self, id: String, name: String) -> Self {
        Self {
            id,
            name,
            revision: 1,
            ..self.clone()
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != THEME_VERSION {
            return Err("Unsupported theme version".into());
        }
        if self.id.trim().is_empty() || self.name.trim().is_empty() || self.revision == 0 {
            return Err("A theme needs an identity, name and nonzero revision".into());
        }
        self.overrides.validate()?;
        let mut ids = BTreeSet::new();
        for rule in &self.rules {
            if rule.id.is_empty() || !ids.insert(&rule.id) {
                return Err("Rule identities must be unique".into());
            }
            match rule.target {
                ThemeTarget::Area {
                    min,
                    max,
                    min_plane,
                    max_plane,
                } if min.x > max.x || min.y > max.y || min_plane > max_plane => {
                    return Err("Area bounds are reversed".into());
                }
                ThemeTarget::Placement {
                    kind, orientation, ..
                } if kind > 22 || orientation > 3 => return Err("Invalid object placement".into()),
                _ => {}
            }
            rule.values.validate()?;
        }
        let mut names = BTreeSet::new();
        for material in &self.materials {
            if material.name.trim().is_empty() || !names.insert(&material.name) {
                return Err("Material names must be unique".into());
            }
            for path in material.asset_paths() {
                pack::validate_path(path)?;
            }
        }
        let mut stages = BTreeSet::new();
        for shader in &self.shaders {
            shader.validate()?;
            if !stages.insert(shader.stage) {
                return Err("Only one shader per extension stage is supported".into());
            }
        }
        Ok(())
    }
}
impl ThemeMaterial {
    pub fn asset_paths(&self) -> impl Iterator<Item = &str> {
        [
            &self.diffuse,
            &self.normal,
            &self.height,
            &self.roughness,
            &self.ambient_occlusion,
            &self.flow,
        ]
        .into_iter()
        .filter_map(|p| p.as_deref())
    }
}
impl ThemeOverrides {
    pub fn validate(&self) -> Result<(), String> {
        for value in [
            self.ambient_strength,
            self.sunlight_strength,
            self.fog_depth,
            self.exposure,
            self.brightness,
        ]
        .into_iter()
        .flatten()
        {
            if !(0.0..=1000.0).contains(&value.get()) {
                return Err("Appearance value is outside 0–1000".into());
            }
        }
        if self.exposure.is_some_and(|x| x.get() <= 0.0)
            || self.brightness.is_some_and(|x| x.get() <= 0.0)
        {
            return Err("Exposure and brightness must be greater than zero".into());
        }
        Ok(())
    }
    pub fn overlay(&mut self, other: &Self) {
        macro_rules! merge { ($($field:ident),*) => { $(if other.$field.is_some() { self.$field.clone_from(&other.$field); })* }; }
        merge!(
            ambient_color,
            ambient_strength,
            sunlight_color,
            sunlight_strength,
            fog_color,
            fog_depth,
            water_color,
            exposure,
            brightness,
            normal_mapping,
            parallax_mapping,
            underwater_caustics,
            material
        );
    }
}

/// Before/after authoring state; the editor adapts this to its existing generic patch history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemePatch {
    before: ThemePack,
    after: ThemePack,
}
impl ThemePatch {
    pub fn plan(before: &ThemePack, after: ThemePack) -> Result<Self, String> {
        before.validate()?;
        after.validate()?;
        Ok(Self {
            before: before.clone(),
            after,
        })
    }
    pub fn inverse(&self) -> Self {
        Self {
            before: self.after.clone(),
            after: self.before.clone(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.before == self.after
    }
    pub fn apply(&self, document: &mut ThemePack) -> Result<(), &'static str> {
        if *document != self.before {
            return Err("The theme changed while the edit was being prepared");
        }
        document.clone_from(&self.after);
        Ok(())
    }
}
