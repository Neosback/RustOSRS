//! Data layer of the 117 HD port: the plugin's JSON scene data (materials, ground materials, tile
//! and model overrides, areas, environments, water types, sky presets, lights and the gameval name
//! tables), loaded verbatim from `assets/117hd/scene` and resolved against each other.
//!
//! The rendering side (shaders, passes, material arrays) builds on these tables; nothing here
//! touches the GPU. Upstream: <https://github.com/117HD/RLHD> (BSD 2-Clause, see `THIRD_PARTY.md`).
//! Field names follow the plugin's JSON schemas (`assets/117hd/schemas`); fields this port does not
//! use yet are kept in `extra` so no information is lost.

pub mod expr;
pub mod materials;
pub mod settings;
pub mod tiles;

pub use serde_json;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt, fs, io,
    path::{Path, PathBuf},
};

/// Anything that can go wrong while loading the data set.
#[derive(Debug)]
pub enum HdError {
    Io(PathBuf, io::Error),
    Json(PathBuf, serde_json::Error),
}

impl fmt::Display for HdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(path, error) => write!(f, "{}: {error}", path.display()),
            Self::Json(path, error) => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for HdError {}

type Extra = BTreeMap<String, Value>;

/// A colour as the JSON writes it: `"#RRGGBB"` or an `[r, g, b]` array (0-255 or 0-1).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Color {
    Hex(String),
    Rgb(Vec<f32>),
}

impl Color {
    /// Linear-looking sRGB components in `0.0..=1.0` (hex and 0-255 arrays are scaled).
    pub fn rgb(&self) -> Option<[f32; 3]> {
        match self {
            Self::Hex(text) => {
                let digits = text.strip_prefix('#')?;
                if digits.len() != 6 {
                    return None;
                }
                let value = u32::from_str_radix(digits, 16).ok()?;
                Some([
                    ((value >> 16) & 255) as f32 / 255.0,
                    ((value >> 8) & 255) as f32 / 255.0,
                    (value & 255) as f32 / 255.0,
                ])
            }
            Self::Rgb(parts) if parts.len() >= 3 => {
                let scale = if parts.iter().any(|part| *part > 1.0) {
                    255.0
                } else {
                    1.0
                };
                Some([parts[0] / scale, parts[1] / scale, parts[2] / scale])
            }
            Self::Rgb(_) => None,
        }
    }
}

/// A reference to a game id: a gameval name (`"BANK_BOOTH"`) or a number.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum IdRef {
    /// A raw id (`-1` means "none").
    Number(i64),
    Name(String),
}

/// `materials.json`: one material (texture set and shading parameters).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Material {
    pub name: String,
    pub description: Option<String>,
    pub parent: Option<String>,
    pub diffuse_map: Option<String>,
    pub normal_map: Option<String>,
    pub displacement_map: Option<String>,
    pub roughness_map: Option<String>,
    pub ambient_occlusion_map: Option<String>,
    pub flow_map: Option<String>,
    pub shadow_alpha_map: Option<String>,
    pub vanilla_texture_index: Option<f32>,
    pub has_transparency: Option<bool>,
    pub double_sided_faces: Option<bool>,
    pub override_base_color: Option<bool>,
    pub unlit: Option<bool>,
    pub brightness: Option<f32>,
    pub subsurface: Option<f32>,
    pub displacement_scale: Option<f32>,
    pub flow_map_strength: Option<f32>,
    pub flow_map_duration: Option<Vec<f32>>,
    pub specular_strength: Option<f32>,
    pub specular_gloss: Option<f32>,
    pub scroll_speed: Option<Vec<f32>>,
    pub texture_scale: Option<Vec<f32>>,
    pub materials_to_replace: Option<Vec<String>>,
    /// Condition under which this material is active/replaces others (see [`expr`]).
    pub replacement_condition: Option<Value>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `ground_materials.json`: a named set of materials a terrain tile can take.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GroundMaterial {
    pub name: String,
    pub description: Option<String>,
    pub materials: Vec<String>,
}

/// `tile_overrides.json`: maps underlay/overlay ids (optionally inside an area) to materials.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TileOverride {
    pub name: String,
    pub description: Option<String>,
    pub area: Option<String>,
    pub overlay_ids: Vec<IdRef>,
    pub underlay_ids: Vec<IdRef>,
    pub ground_material: Option<String>,
    pub water_type: Option<String>,
    pub blended: Option<bool>,
    pub blended_as_opposite: Option<bool>,
    pub double_sided_faces: Option<bool>,
    pub forced: Option<bool>,
    pub depth_tested: Option<bool>,
    pub uv_orientation: Option<i32>,
    pub uv_scale: Option<f32>,
    pub height_offset: Option<i32>,
    pub set_hue: Option<i32>,
    pub shift_hue: Option<i32>,
    pub min_hue: Option<i32>,
    pub max_hue: Option<i32>,
    pub set_saturation: Option<i32>,
    pub shift_saturation: Option<i32>,
    pub min_saturation: Option<i32>,
    pub max_saturation: Option<i32>,
    pub set_lightness: Option<i32>,
    pub shift_lightness: Option<i32>,
    pub min_lightness: Option<i32>,
    pub max_lightness: Option<i32>,
    /// Replacement override name -> condition, in file order (the first match wins).
    #[serde(deserialize_with = "ordered_map")]
    pub replacements: Vec<(String, Value)>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Deserialize a JSON object keeping the order of its keys.
fn ordered_map<'de, D>(deserializer: D) -> Result<Vec<(String, Value)>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = Vec<(String, Value)>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an object")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut entries = Vec::new();
            while let Some(entry) = map.next_entry::<String, Value>()? {
                entries.push(entry);
            }
            Ok(entries)
        }
        fn visit_unit<E>(self) -> Result<Self::Value, E> {
            Ok(Vec::new())
        }
    }
    deserializer.deserialize_any(Visitor)
}

/// `model_overrides.json`: per npc/object/projectile/graphics-object shading rules.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModelOverride {
    pub description: Option<String>,
    pub seasonal_theme: Option<String>,
    /// Area names or inline `[x0, y0, x1, y1]` boxes.
    pub areas: Vec<Value>,
    pub npc_ids: Vec<IdRef>,
    pub object_ids: Vec<IdRef>,
    pub projectile_ids: Vec<IdRef>,
    pub graphics_object_ids: Vec<IdRef>,
    pub base_material: Option<String>,
    pub texture_material: Option<String>,
    pub uv_type: Option<String>,
    pub uv_scale: Option<f32>,
    pub hide: Option<bool>,
    pub flat_normals: Option<bool>,
    pub upwards_normals: Option<bool>,
    pub hide_vanilla_shadows: Option<bool>,
    pub cast_shadows: Option<bool>,
    pub receive_shadows: Option<bool>,
    pub double_sided_faces: Option<bool>,
    pub undo_vanilla_shading: Option<bool>,
    pub hide_as_water_effect: Option<bool>,
    pub wind_displacement_mode: Option<String>,
    pub depth_bias: Option<i32>,
    pub height_offset: Option<i32>,
    pub uv_orientation: Option<i32>,
    pub uv_orientation_x: Option<i32>,
    pub uv_orientation_y: Option<i32>,
    pub uv_orientation_z: Option<i32>,
    pub retain_vanilla_uvs: Option<bool>,
    pub force_material_changes: Option<bool>,
    pub set_hue: Option<i32>,
    pub shift_hue: Option<i32>,
    pub min_hue: Option<i32>,
    pub max_hue: Option<i32>,
    pub set_saturation: Option<i32>,
    pub shift_saturation: Option<i32>,
    pub min_saturation: Option<i32>,
    pub max_saturation: Option<i32>,
    pub set_lightness: Option<i32>,
    pub shift_lightness: Option<i32>,
    pub min_lightness: Option<i32>,
    pub max_lightness: Option<i32>,
    pub set_alpha: Option<i32>,
    pub shift_alpha: Option<i32>,
    pub min_alpha: Option<i32>,
    pub max_alpha: Option<i32>,
    pub material_overrides: Option<BTreeMap<String, Value>>,
    pub color_overrides: Option<Vec<Value>>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `areas.json`: a named region of the world made of boxes, regions and other areas.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Area {
    pub name: String,
    pub description: Option<String>,
    pub hide_other_areas: Option<bool>,
    pub fill_gaps: Option<bool>,
    /// Areas this one includes: names, or inline `[x0, y0, x1, y1]` boxes.
    pub areas: Vec<Value>,
    /// Region ids.
    pub regions: Vec<i32>,
    /// `[x0, y0, x1, y1]` region boxes (inclusive, region coordinates or ids).
    pub region_boxes: Vec<Vec<i32>>,
    /// `[x0, y0, x1, y1]` or `[x0, y0, plane0, x1, y1, plane1]` tile boxes (inclusive).
    pub aabbs: Vec<Vec<i32>>,
    pub unhide_areas: Vec<Value>,
}

/// `environments.json`: lighting and fog of an area.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Environment {
    pub key: Option<String>,
    pub area: Option<String>,
    pub description: Option<String>,
    pub is_poh_theme: Option<bool>,
    pub is_underwater: Option<bool>,
    pub force: Option<bool>,
    pub allow_sky_override: Option<bool>,
    pub allow_roof_shadows: Option<bool>,
    pub instant_transition: Option<bool>,
    pub ambient_color: Option<Color>,
    pub ambient_strength: Option<f32>,
    pub directional_color: Option<Color>,
    pub directional_strength: Option<f32>,
    pub water_color: Option<Color>,
    pub water_caustics_color: Option<Color>,
    pub water_caustics_strength: Option<f32>,
    pub underglow_color: Option<Color>,
    pub underglow_strength: Option<f32>,
    pub shadow_angles: Option<Vec<f32>>,
    pub fog_color: Option<Color>,
    pub fog_depth: Option<f32>,
    pub ground_fog_start: Option<f32>,
    pub ground_fog_end: Option<f32>,
    pub ground_fog_opacity: Option<f32>,
    pub wind_angle: Option<f32>,
    pub wind_speed: Option<f32>,
    pub wind_strength: Option<f32>,
    pub wind_ceiling: Option<f32>,
    pub hide_vanilla_skyboxes: Option<bool>,
    pub night_exposure: Option<f32>,
    pub sky: Option<Value>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `water_types.json`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WaterType {
    pub name: String,
    pub description: Option<String>,
    pub vanilla_texture_index: Option<i32>,
    pub flat: Option<bool>,
    pub specular_strength: Option<f32>,
    pub specular_gloss: Option<f32>,
    pub normal_strength: Option<f32>,
    pub base_opacity: Option<f32>,
    pub fresnel_amount: Option<f32>,
    pub normal_map: Option<String>,
    pub surface_color: Option<Color>,
    pub foam_color: Option<Color>,
    pub depth_color: Option<Color>,
    pub has_foam: Option<bool>,
    pub duration: Option<f32>,
    pub fishing_spot_recolor: Option<i32>,
}

/// `sky_presets.json`: kept loosely typed (gradients and celestial settings are consumed by the
/// sky pass when it is ported).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkyPreset {
    pub name: String,
    pub parent: Option<String>,
    #[serde(flatten)]
    pub settings: Extra,
}

/// `lights.json`: one placed point light.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Light {
    pub description: Option<String>,
    pub world_x: Option<i32>,
    pub world_y: Option<i32>,
    pub plane: Option<i32>,
    pub alignment: Option<String>,
    pub offset: Option<Vec<f32>>,
    pub height: Option<i32>,
    pub radius: Option<i32>,
    pub strength: Option<f32>,
    pub color: Option<Color>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub duration: Option<f32>,
    pub range: Option<f32>,
    /// Area names or inline boxes.
    pub areas: Vec<Value>,
    pub npc_ids: Vec<IdRef>,
    pub object_ids: Vec<IdRef>,
    pub projectile_ids: Vec<IdRef>,
    pub graphics_object_ids: Vec<IdRef>,
    pub animation_ids: Vec<IdRef>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `gamevals.json`: the plugin's name -> id tables for the current cache.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Gamevals {
    pub npcs: HashMap<String, u32>,
    pub objects: HashMap<String, u32>,
    pub anims: HashMap<String, u32>,
    pub spotanims: HashMap<String, u32>,
    pub varbits: HashMap<String, u32>,
    pub varps: HashMap<String, u32>,
    pub varcs: HashMap<String, u32>,
}

impl Gamevals {
    /// Resolve a reference against one table.
    pub fn resolve(table: &HashMap<String, u32>, id: &IdRef) -> Option<i64> {
        match id {
            IdRef::Number(number) => Some(*number),
            IdRef::Name(name) => table.get(name).map(|id| i64::from(*id)),
        }
    }
}

/// The whole 117 HD data set.
#[derive(Debug, Default)]
pub struct HdData {
    pub materials: Vec<Material>,
    pub ground_materials: Vec<GroundMaterial>,
    pub tile_overrides: Vec<TileOverride>,
    pub model_overrides: Vec<ModelOverride>,
    pub areas: Vec<Area>,
    pub environments: Vec<Environment>,
    pub water_types: Vec<WaterType>,
    pub sky_presets: Vec<SkyPreset>,
    pub lights: Vec<Light>,
    pub gamevals: Gamevals,
}

fn read<T: for<'de> Deserialize<'de>>(directory: &Path, file: &str) -> Result<T, HdError> {
    let path = directory.join(file);
    let text = fs::read_to_string(&path).map_err(|error| HdError::Io(path.clone(), error))?;
    // The gameval file starts with `//` comment lines.
    let json: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    serde_json::from_str(&json).map_err(|error| HdError::Json(path, error))
}

impl HdData {
    /// Load every table from `<assets>/scene` (`assets/117hd` in this repository).
    pub fn load(assets: impl AsRef<Path>) -> Result<Self, HdError> {
        let scene = assets.as_ref().join("scene");
        Ok(Self {
            materials: read(&scene, "materials.json")?,
            ground_materials: read(&scene, "ground_materials.json")?,
            tile_overrides: read(&scene, "tile_overrides.json")?,
            model_overrides: read(&scene, "model_overrides.json")?,
            areas: read(&scene, "areas.json")?,
            environments: read(&scene, "environments.json")?,
            water_types: read(&scene, "water_types.json")?,
            sky_presets: read(&scene, "sky_presets.json")?,
            lights: read(&scene, "lights.json")?,
            gamevals: read(&scene, "gamevals.json")?,
        })
    }

    pub fn material(&self, name: &str) -> Option<&Material> {
        self.materials.iter().find(|material| material.name == name)
    }

    pub fn area(&self, name: &str) -> Option<&Area> {
        self.areas.iter().find(|area| area.name == name)
    }

    /// Does `area` (including the areas it includes) contain world tile `(x, y)` on `plane`?
    pub fn area_contains(&self, area: &str, x: i32, y: i32, plane: i32) -> bool {
        self.area_contains_inner(area, x, y, plane, &mut HashSet::new())
    }

    fn area_contains_inner(
        &self,
        name: &str,
        x: i32,
        y: i32,
        plane: i32,
        visiting: &mut HashSet<String>,
    ) -> bool {
        let Some(area) = self.area(name) else {
            return false;
        };
        if !visiting.insert(name.to_owned()) {
            return false;
        }
        let (region_x, region_y) = (x >> 6, y >> 6);
        let inside = area.aabbs.iter().any(|aabb| match aabb.as_slice() {
            [x0, y0, x1, y1] => (*x0..=*x1).contains(&x) && (*y0..=*y1).contains(&y),
            [x0, y0, z0, x1, y1, z1] => {
                (*x0..=*x1).contains(&x) && (*y0..=*y1).contains(&y) && (*z0..=*z1).contains(&plane)
            }
            _ => false,
        }) || area.regions.contains(&((region_x << 8) | region_y))
            || area
                .region_boxes
                .iter()
                .any(|boxed| match boxed.as_slice() {
                    [x0, y0, x1, y1] => {
                        (*x0..=*x1).contains(&region_x) && (*y0..=*y1).contains(&region_y)
                    }
                    _ => false,
                })
            || area.areas.iter().any(|child| match child {
                Value::String(name) => self.area_contains_inner(name, x, y, plane, visiting),
                Value::Array(parts) => {
                    let numbers: Vec<i32> = parts
                        .iter()
                        .filter_map(|part| part.as_i64().map(|n| n as i32))
                        .collect();
                    matches!(numbers.as_slice(),
                        [x0, y0, x1, y1] if (*x0..=*x1).contains(&x) && (*y0..=*y1).contains(&y))
                }
                _ => false,
            });
        visiting.remove(name);
        inside
    }

    /// The first environment whose area contains the tile (the plugin lists specific areas first).
    pub fn environment_at(&self, x: i32, y: i32, plane: i32) -> Option<&Environment> {
        self.environments.iter().find(|environment| {
            environment
                .area
                .as_deref()
                .is_some_and(|area| self.area_contains(area, x, y, plane))
        })
    }
}
