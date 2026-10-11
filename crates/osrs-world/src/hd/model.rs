//! Model emission for the HD profile: model overrides (`ModelOverrideManager`) and a port of the
//! face loop of `SceneUploader.uploadStaticModel`.
//!
//! Supported override fields: base and texture materials (and per-vanilla-material overrides),
//! `uvType` (geometry, vanilla, model/world planes, box), `uvScale` and orientation, flat and
//! upward normals, `undoVanillaShading`, `doubleSidedFaces`, `receiveShadows`, `depthBias`, `hide`
//! and colour/alpha modifiers. Not yet: colour-conditional overrides, terrain vertex snapping,
//! wind displacement, tile-colour inheritance, TzHaar recolouring and fake-shadow hiding.
//!
//! Adapted from `rs117.hd.scene.ModelOverrideManager`, `ModelOverride` and
//! `rs117.hd.renderer.zone.SceneUploader`. Copyright (c) 2018, Adam <Adam@sigterm.info>; (c) 2021,
//! 117 and Hooder (BSD 2-Clause, see `THIRD_PARTY.md`).

use super::HdContext;
use osrs_core::{lighting::ReferenceLitModel, trig::trig_tables};
use osrs_hd::serde_json::{self, Value};
use osrs_hd::{
    Gamevals, HdData, ModelOverride,
    materials::MaterialSet,
    settings::{HdSettings, SeasonalTheme, VanillaShadowMode},
    tiles::Areas,
};
use osrs_render::{
    HD_FLAG_FLAT_NORMALS, HD_FLAG_NO_SHADOW_CAST, HD_FLAG_NO_SHADOW_RECEIVE,
    HD_FLAG_SHADOW_NORMAL_BIAS, HD_FLAG_UPWARDS_NORMALS, HD_FLAG_VANILLA_UVS, HdFace,
    ModelPlacement, vanilla_face_uvs,
};
use std::collections::HashMap;

const TILE: f32 = 128.0;
const JAU_TO_RAD: f32 = std::f32::consts::TAU / 2048.0;

/// `UvType`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UvType {
    Vanilla,
    Geometry,
    /// Planar projection along a model axis pair: `(u axis, v axis)` of x=0, y=1, z=2.
    Model {
        u: usize,
        v: usize,
        mirror: Mirror,
    },
    /// Planar projection of the world position along a fixed normal.
    World([f32; 3]),
    Box,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirror {
    None,
    A,
    B,
}

impl UvType {
    fn parse(name: &str) -> Self {
        let (base, mirror) = if let Some(base) = name.strip_suffix("_MIRROR_A") {
            (base, Mirror::A)
        } else if let Some(base) = name.strip_suffix("_MIRROR_B") {
            (base, Mirror::B)
        } else {
            (name, Mirror::None)
        };
        match base {
            "VANILLA" => Self::Vanilla,
            "GEOMETRY" => Self::Geometry,
            "MODEL_XY" => Self::Model { u: 0, v: 1, mirror },
            "MODEL_XZ" => Self::Model { u: 0, v: 2, mirror },
            "MODEL_YZ" => Self::Model { u: 1, v: 2, mirror },
            "WORLD_XY" => Self::World([0.0, 0.0, -1.0]),
            "WORLD_XZ" => Self::World([0.0, -1.0, 0.0]),
            "WORLD_YZ" => Self::World([-1.0, 0.0, 0.0]),
            "BOX" => Self::Box,
            _ => Self::Vanilla,
        }
    }
}

/// Hue/saturation/lightness/alpha modifiers (`ModelOverride.modifyColor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ColorMod {
    hue: (i32, i32, i32),
    saturation: (i32, i32, i32),
    lightness: (i32, i32, i32),
    alpha: (i32, i32, i32),
    modifies_color: bool,
    modifies_alpha: bool,
}

impl ColorMod {
    fn new(o: &ModelOverride) -> Self {
        let channel =
            |set: Option<i32>, shift: Option<i32>, min: Option<i32>, max: Option<i32>, top: i32| {
                let (mut lo, mut hi) = (min.unwrap_or(0), max.unwrap_or(top));
                if let Some(set) = set.filter(|set| *set != -1) {
                    (lo, hi) = (set, set);
                }
                (shift.unwrap_or(0), lo.clamp(0, top), hi.clamp(0, top))
            };
        let hue = channel(o.set_hue, o.shift_hue, o.min_hue, o.max_hue, 63);
        let saturation = channel(
            o.set_saturation,
            o.shift_saturation,
            o.min_saturation,
            o.max_saturation,
            7,
        );
        let lightness = channel(
            o.set_lightness,
            o.shift_lightness,
            o.min_lightness,
            o.max_lightness,
            127,
        );
        let alpha = channel(o.set_alpha, o.shift_alpha, o.min_alpha, o.max_alpha, 255);
        let modifies_alpha = alpha != (0, 0, 255);
        Self {
            hue,
            saturation,
            lightness,
            alpha,
            modifies_color: hue != (0, 0, 63)
                || saturation != (0, 0, 7)
                || lightness != (0, 0, 127)
                || modifies_alpha,
            modifies_alpha,
        }
    }

    fn color(&self, hsl: i32) -> i32 {
        let apply =
            |value: i32, (shift, min, max): (i32, i32, i32)| (value + shift).clamp(min, max);
        (apply((hsl >> 10) & 0x3F, self.hue) << 10)
            | (apply((hsl >> 7) & 7, self.saturation) << 7)
            | apply(hsl & 0x7F, self.lightness)
    }

    fn alpha(&self, alpha: i32) -> i32 {
        (alpha + self.alpha.0).clamp(self.alpha.1, self.alpha.2)
    }
}

/// A model override with names resolved (`ModelOverride.normalize`).
#[derive(Debug, Clone)]
pub struct ResolvedModelOverride {
    pub base_material: usize,
    pub texture_material: usize,
    pub uv_type: UvType,
    pub uv_scale: f32,
    pub uv_orientation: [i32; 3],
    pub retain_vanilla_uvs: bool,
    pub hide: bool,
    pub flat_normals: bool,
    pub upwards_normals: bool,
    pub double_sided_faces: bool,
    pub undo_vanilla_shading: bool,
    pub receive_shadows: bool,
    pub cast_shadows: bool,
    pub shadow_normal_bias: bool,
    /// Hide faces that are Jagex's baked ground shadow (`hideVanillaShadows`, after the PvM rule).
    pub hide_vanilla_shadows: bool,
    pub depth_bias: Option<u8>,
    color: ColorMod,
    /// Overrides applied to faces that use a given vanilla material.
    pub material_overrides: Vec<(usize, ResolvedModelOverride)>,
}

impl ResolvedModelOverride {
    /// `ModelOverride.NONE`.
    pub fn none() -> Self {
        Self::resolve(
            &ModelOverride::default(),
            &MaterialSet::default(),
            &HdSettings::default(),
        )
    }

    fn resolve(o: &ModelOverride, materials: &MaterialSet, settings: &HdSettings) -> Self {
        let named =
            |name: &Option<String>| name.as_deref().map_or(0, |name| materials.index_of(name));
        let (mut base, mut texture) = (named(&o.base_material), named(&o.texture_material));
        // Without object textures only materials that do not replace vanilla textures remain.
        if !settings.model_textures && o.force_material_changes != Some(true) {
            if modifies_vanilla_texture(materials, base) {
                base = 0;
            }
            if modifies_vanilla_texture(materials, texture) {
                texture = 0;
            }
        }
        let orientation = o.uv_orientation.unwrap_or(0);
        let pick = |axis: Option<i32>| axis.filter(|value| *value != 0).unwrap_or(orientation);
        let mut material_overrides = Vec::new();
        for (name, value) in o.material_overrides.iter().flatten() {
            let Ok(inner) = serde_json::from_value::<ModelOverride>(value.clone()) else {
                continue;
            };
            let resolved = Self::resolve(&inner, materials, settings);
            if !settings.model_textures
                && modifies_vanilla_texture(materials, resolved.texture_material)
            {
                continue;
            }
            material_overrides.push((materials.index_of(name), resolved));
        }
        // Models that keep their vanilla shadows for PvM show them and, in `PreferInPvm`, drop the
        // dynamic ones (`ModelOverride.normalize`).
        let mut hide_vanilla_shadows = o.hide_vanilla_shadows == Some(true);
        let mut cast_shadows = o.cast_shadows != Some(false);
        if o.retain_vanilla_shadows_in_pvm == Some(true) {
            hide_vanilla_shadows = false;
            if settings.vanilla_shadows == VanillaShadowMode::PreferInPvm
                && o.hide_hd_shadows_in_pvm == Some(true)
            {
                cast_shadows = false;
            }
        }
        Self {
            base_material: base,
            texture_material: texture,
            uv_type: o.uv_type.as_deref().map_or(UvType::Vanilla, UvType::parse),
            uv_scale: o.uv_scale.filter(|scale| *scale != 0.0).unwrap_or(1.0),
            uv_orientation: [
                pick(o.uv_orientation_x),
                pick(o.uv_orientation_y),
                pick(o.uv_orientation_z),
            ],
            retain_vanilla_uvs: o.retain_vanilla_uvs.unwrap_or(true),
            hide: o.hide == Some(true),
            flat_normals: o.flat_normals == Some(true),
            upwards_normals: o.upwards_normals == Some(true),
            double_sided_faces: o.double_sided_faces == Some(true),
            undo_vanilla_shading: o.undo_vanilla_shading.unwrap_or(true),
            receive_shadows: o.receive_shadows.unwrap_or(true),
            cast_shadows,
            shadow_normal_bias: o.shadow_normal_bias == Some(true),
            hide_vanilla_shadows,
            depth_bias: o
                .depth_bias
                .filter(|bias| *bias != -1)
                .map(|bias| bias.clamp(0, 255) as u8),
            color: ColorMod::new(o),
            material_overrides,
        }
    }
}

/// `Material.modifiesVanillaTexture`: the material brings its own texture or maps.
fn modifies_vanilla_texture(materials: &MaterialSet, index: usize) -> bool {
    let Some(material) = materials.get(index) else {
        return false;
    };
    let mut root = material;
    for _ in 0..16 {
        match root.parent.as_deref() {
            Some(name) if name != "NONE" => {
                let Some(parent) = materials.get(materials.index_of(name)) else {
                    break;
                };
                root = parent;
            }
            Some(_) => {
                return material.normal_map.is_some()
                    || material.displacement_map.is_some()
                    || material.roughness_map.is_some()
                    || material.ambient_occlusion_map.is_some()
                    || material.flow_map.is_some()
                    || material.shadow_alpha_map.is_some();
            }
            None => break,
        }
    }
    true
}

/// Model overrides by object id, with area-restricted variants.
#[derive(Debug, Clone)]
pub struct ModelOverrides {
    none: ResolvedModelOverride,
    list: Vec<ResolvedModelOverride>,
    /// object id -> (index of the unrestricted override, area-restricted ones in file order).
    by_object: HashMap<u32, ObjectEntry>,
    areas: Vec<Vec<Value>>,
}

#[derive(Debug, Clone, Default)]
struct ObjectEntry {
    default: Option<usize>,
    restricted: Vec<usize>,
}

impl ModelOverrides {
    /// Index the object overrides of `data` (NPC, projectile and graphics-object ones are for
    /// models the editor does not draw yet).
    pub fn with_settings(data: &HdData, materials: &MaterialSet, settings: &HdSettings) -> Self {
        let mut overrides = Self {
            none: ResolvedModelOverride::resolve(&ModelOverride::default(), materials, settings),
            list: Vec::new(),
            by_object: HashMap::new(),
            areas: Vec::new(),
        };
        for source in &data.model_overrides {
            if let Some(theme) = source.seasonal_theme.as_deref()
                && !season_matches(theme, settings.season)
            {
                continue;
            }
            let ids: Vec<u32> = source
                .object_ids
                .iter()
                .filter_map(|id| Gamevals::resolve(&data.gamevals.objects, id))
                .filter_map(|id| u32::try_from(id).ok())
                .collect();
            if ids.is_empty() {
                continue;
            }
            let index = overrides.list.len();
            overrides
                .list
                .push(ResolvedModelOverride::resolve(source, materials, settings));
            overrides.areas.push(source.areas.clone());
            for id in ids {
                let entry = overrides.by_object.entry(id).or_default();
                if source.areas.is_empty() {
                    entry.default = Some(index);
                } else {
                    entry.restricted.push(index);
                }
            }
        }
        overrides
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// The override for an object at a world tile (`ModelOverrideManager.getOverride`).
    pub fn get(
        &self,
        object_id: u32,
        world: (i32, i32, i32),
        areas: &Areas,
    ) -> &ResolvedModelOverride {
        let Some(entry) = self.by_object.get(&object_id) else {
            return &self.none;
        };
        for &index in &entry.restricted {
            if self.areas[index]
                .iter()
                .any(|area| areas.json_contains(area, world.0, world.1, world.2))
            {
                return &self.list[index];
            }
        }
        entry.default.map_or(&self.none, |index| &self.list[index])
    }
}

fn season_matches(name: &str, season: SeasonalTheme) -> bool {
    matches!(
        (name, season),
        ("SUMMER", SeasonalTheme::Summer)
            | ("AUTUMN", SeasonalTheme::Autumn)
            | ("WINTER", SeasonalTheme::Winter)
            | ("AUTOMATIC", SeasonalTheme::Automatic)
    )
}

// ---------------------------------------------------------------------------------------------
// Face loop
// ---------------------------------------------------------------------------------------------

const IGNORE_LOW_LIGHTNESS: i32 = 3;
const LIGHTNESS_MULTIPLIER: f32 = 3.0;
const BASE_LIGHTEN: i32 = 10;

/// `SceneUploader.getMaxBrightness`.
fn max_brightness(saturation: i32) -> i32 {
    match saturation {
        0 => 127,
        1 => 61,
        2 => 59,
        3 => 57,
        4 | 5 => 56,
        _ => 55,
    }
}

/// `SceneUploader.undoVanillaShading`: brighten colours the client darkened with its light.
pub fn undo_vanilla_shading(color: i32, legacy_grey: bool, normal: [f32; 3]) -> i32 {
    let s = (color >> 7) & 7;
    let mut l = color & 0x7F;
    let adjust = BASE_LIGHTEN as f32 - l as f32
        + if l < IGNORE_LOW_LIGHTNESS {
            0.0
        } else {
            (l - IGNORE_LOW_LIGHTNESS) as f32 * LIGHTNESS_MULTIPLIER
        };
    let length_squared = normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2];
    if length_squared > 0.0 {
        let light_dot_normal =
            (normal[0] + normal[1] + normal[2]) * 0.577_350_26 / length_squared.sqrt();
        if light_dot_normal > 0.0 {
            l += (light_dot_normal * adjust) as i32;
        }
    }
    l = l.min(if legacy_grey { 55 } else { max_brightness(s) });
    (color & 0xFC00) | (s << 7) | l
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn normalize(vector: [f32; 3]) -> [f32; 3] {
    let length = (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt();
    if length <= f32::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        vector.map(|c| c / length)
    }
}

fn sign(value: f32) -> f32 {
    if value < 0.0 { -1.0 } else { 1.0 }
}

/// `ModelOverride.computeBoxUvw`: project the face onto the dominant axis plane of its normal.
fn box_uvs(
    corners: [[f32; 3]; 3],
    model_orientation: i32,
    scale: f32,
    orientation: [i32; 3],
) -> [[f32; 2]; 3] {
    let mut v = corners;
    if model_orientation % 2048 != 0 {
        let radians = model_orientation as f32 * JAU_TO_RAD;
        let (sin, cos) = radians.sin_cos();
        for p in &mut v {
            let temp = p[0] * sin + p[2] * cos;
            p[0] = p[0] * cos - p[2] * sin;
            p[2] = temp;
        }
    }
    for p in &mut v {
        for c in p.iter_mut() {
            *c = (*c / TILE + 0.5) / scale;
        }
    }
    let n = cross(sub(v[1], v[0]), sub(v[2], v[0]));
    let (ax, ay, az) = (n[0].abs(), n[1].abs(), n[2].abs());
    let mut out = [[0.0_f32; 2]; 3];
    let rotation = if ax > ay && ax > az {
        let flip = sign(n[0]);
        for (uv, p) in out.iter_mut().zip(&v) {
            *uv = [flip * -p[2], p[1]];
        }
        orientation[0]
    } else if ay > ax && ay > az {
        let flip = sign(n[1]);
        for (uv, p) in out.iter_mut().zip(&v) {
            *uv = [flip * -p[0], p[2]];
        }
        orientation[1]
    } else {
        let flip = sign(n[2]);
        for (uv, p) in out.iter_mut().zip(&v) {
            *uv = [flip * p[0], p[1]];
        }
        orientation[2]
    };
    if rotation % 2048 != 0 {
        let (sin, cos) = (rotation as f32 * JAU_TO_RAD).sin_cos();
        for uv in &mut out {
            let (x, y) = (uv[0] - 0.5, uv[1] - 0.5);
            *uv = [(x * cos - y * sin) + 0.5, (x * sin + y * cos) + 0.5];
        }
    }
    out
}

/// `UvType.MODEL_*` UVs: two model axes (rotation-compensated), optionally mirrored.
fn model_uvs(
    corners: [[f32; 3]; 3],
    model_orientation: i32,
    axes: (usize, usize),
    mirror: Mirror,
) -> [[f32; 2]; 3] {
    let mut out = [[0.0_f32; 2]; 3];
    for (uv, corner) in out.iter_mut().zip(corners) {
        let mut p = corner;
        if model_orientation % 2048 != 0 {
            let (sin, cos) = (model_orientation as f32 * JAU_TO_RAD).sin_cos();
            let temp = p[0] * sin + p[2] * cos;
            p[0] = p[0] * cos - p[2] * sin;
            p[2] = temp;
        }
        let mut value = [p[axes.0] / TILE, p[axes.1] / TILE];
        match mirror {
            Mirror::None => {}
            Mirror::A => {
                if value[0] < value[1] {
                    value.swap(0, 1);
                }
            }
            Mirror::B => {
                value[1] = 1.0 - value[1];
                if value[0] < value[1] {
                    value.swap(0, 1);
                }
            }
        }
        *uv = value;
    }
    out
}

/// World-plane UVs, as the plugin's vertex shader derives them from the plane normal.
fn world_uvs(normal: [f32; 3], scale: f32, world: [[f32; 3]; 3]) -> [[f32; 2]; 3] {
    let n = normal;
    let c1 = cross([0.0, 0.0, 1.0], n);
    let c2 = cross([0.0, 1.0, 0.0], n);
    let len = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let t = normalize(if len(c1) > len(c2) { c1 } else { c2 });
    let b = cross(n, t);
    world.map(|p| {
        let dot = |a: [f32; 3]| a[0] * p[0] + a[1] * p[1] + a[2] * p[2];
        [dot(t) / TILE / scale, dot(b) / TILE / scale]
    })
}

/// Placement of a model in the scene, for UVs that depend on world position.
#[derive(Debug, Clone, Copy)]
pub struct ModelSite {
    pub object_id: u32,
    /// World tile and plane of the anchor tile.
    pub world: (i32, i32, i32),
    /// Zone-local placement of the model origin.
    pub placement: ModelPlacement,
    /// Scene-local origin of the owning zone, to turn zone-local positions into world ones.
    pub zone_origin_world: (i32, i32),
}

/// The HD faces of one lit model.
pub fn model_faces(hd: &HdContext, model: &ReferenceLitModel, site: ModelSite) -> Vec<HdFace> {
    let override_ = hd.models.get(site.object_id, site.world, &hd.areas);
    model_faces_resolved(
        &hd.materials,
        hd.settings.hide_fake_shadows(),
        override_,
        model,
        site,
    )
}

/// Emit one model after its object/location override has already been resolved. Dynamic objects
/// use this after posing so their material replacement, UV, alpha and shadow semantics are the
/// same as static locs.
pub(crate) fn model_faces_resolved(
    materials: &MaterialSet,
    hide_fake_shadows: bool,
    override_: &ResolvedModelOverride,
    model: &ReferenceLitModel,
    site: ModelSite,
) -> Vec<HdFace> {
    let placement = site.placement;

    // Placed vertex positions, rotated like the Reference path.
    let (sin, cos) = if placement.orientation == 0 {
        (0, 0)
    } else {
        let tables = trig_tables();
        let index = usize::from(placement.orientation) & 2047;
        (tables.sine(index), tables.cosine(index))
    };
    let rotate = |x: i32, z: i32| -> (i32, i32) {
        if placement.orientation == 0 {
            (x, z)
        } else {
            (
                (z.wrapping_mul(sin).wrapping_add(x.wrapping_mul(cos))) >> 16,
                (z.wrapping_mul(cos).wrapping_sub(x.wrapping_mul(sin))) >> 16,
            )
        }
    };
    let positions: Vec<[i32; 3]> = model
        .vertices
        .iter()
        .map(|v| {
            let (x, z) = rotate(v.x, v.z);
            [x + placement.x, v.y + placement.y, z + placement.z]
        })
        .collect();
    let local: Vec<[f32; 3]> = model
        .vertices
        .iter()
        .map(|v| [v.x as f32, v.y as f32, v.z as f32])
        .collect();

    // Vertex normals: area-weighted sum of the adjacent face normals (the client's base normals).
    let mut vertex_normals = vec![[0.0_f32; 3]; model.vertices.len()];
    for face in &model.faces {
        let idx = [
            face.a.get() as usize,
            face.b.get() as usize,
            face.c.get() as usize,
        ];
        if idx.iter().any(|i| *i >= local.len()) {
            continue;
        }
        let n = cross(
            sub(local[idx[1]], local[idx[0]]),
            sub(local[idx[2]], local[idx[0]]),
        );
        for i in idx {
            for axis in 0..3 {
                vertex_normals[i][axis] += n[axis];
            }
        }
    }
    let rotate_normal = |n: [f32; 3]| -> [f32; 3] {
        if placement.orientation == 0 {
            return n;
        }
        let radians = f32::from(placement.orientation) * JAU_TO_RAD;
        let (s, c) = radians.sin_cos();
        [n[2] * s + n[0] * c, n[1], n[2] * c - n[0] * s]
    };

    let mut faces = Vec::with_capacity(model.faces.len());
    for (face_index, face) in model.faces.iter().enumerate() {
        let colors = model.face_colors[face_index];
        let flat_lit = colors.c == -1;
        let (mut c1, mut c2, mut c3) = match colors.c {
            -2 => continue,
            -1 => (colors.a, colors.a, colors.a),
            _ => (colors.a, colors.b, colors.c),
        };
        let mut transparency = i32::from(
            model
                .face_alphas
                .as_ref()
                .map_or(0, |alphas| alphas[face_index] as u8),
        );
        if transparency >= 253 {
            continue;
        }
        let texture_id = model
            .face_textures
            .as_ref()
            .and_then(|textures| textures[face_index])
            .map(|texture| texture.get() as i32);
        let texture_face = model
            .texture_faces
            .as_ref()
            .and_then(|faces| faces.get(face_index).copied().flatten());

        let mut uv_type = UvType::Geometry;
        let mut material = override_.base_material;
        let mut face_override = override_;
        if let Some(id) = texture_id {
            uv_type = UvType::Vanilla;
            if override_.texture_material != 0 {
                material = override_.texture_material;
            } else {
                material = materials.from_vanilla_texture(id);
                if let Some((_, special)) = override_
                    .material_overrides
                    .iter()
                    .find(|(index, _)| *index == material)
                {
                    face_override = special;
                    material = special.texture_material;
                }
            }
        }
        if face_override.hide {
            continue;
        }

        let idx = [
            face.a.get() as usize,
            face.b.get() as usize,
            face.c.get() as usize,
        ];
        if idx.iter().any(|i| *i >= positions.len()) {
            continue;
        }
        // Fake shadows or lighting baked into models are made transparent: the HD shadows replace
        // them.
        if hide_fake_shadows
            && face_override.hide_vanilla_shadows
            && is_baked_ground_shading(model, face_index, transparency, texture_id.is_some())
        {
            continue;
        }
        let mut keep_shading = false;
        if texture_id.is_some() {
            // Vanilla shading on textures is too visible even after reversing it; use a fixed
            // colour and cap the brightness afterwards.
            (c1, c2, c3) = (90, 90, 90);
            keep_shading = true;
        }
        if face_override.color.modifies_color {
            (c1, c2, c3) = (
                face_override.color.color(c1),
                face_override.color.color(c2),
                face_override.color.color(c3),
            );
        }
        if material != 0 {
            uv_type = face_override.uv_type;
            if uv_type == UvType::Vanilla
                || (texture_id.is_some() && face_override.retain_vanilla_uvs)
            {
                uv_type = if model.texture_faces.is_some() && texture_face.is_some() {
                    UvType::Vanilla
                } else {
                    UvType::Geometry
                };
            }
        }

        // Normals: vertex normals unless the face is flat-lit or has none.
        let corner_points = idx.map(|i| local[i]);
        let face_normal = normalize(cross(
            sub(corner_points[1], corner_points[0]),
            sub(corner_points[2], corner_points[0]),
        ));
        let vertex_n = idx.map(|i| vertex_normals[i]);
        let use_face_normal =
            face_override.flat_normals || flat_lit || vertex_n.iter().all(|n| *n == [0.0; 3]);
        let normals_unrotated = if use_face_normal {
            [face_normal; 3]
        } else {
            vertex_n
        };

        if hd_undo(face_override) && !keep_shading {
            let hsl = [c1, c2, c3];
            let undone: Vec<i32> = hsl
                .iter()
                .zip(normals_unrotated)
                .map(|(color, normal)| undo_vanilla_shading(*color, false, normal))
                .collect();
            (c1, c2, c3) = (undone[0], undone[1], undone[2]);
        }
        let normals = normals_unrotated.map(|n| normalize(rotate_normal(n)));

        if face_override.color.modifies_alpha {
            transparency = 255 - face_override.color.alpha(255 - transparency);
        }
        let bias = face_override.depth_bias.unwrap_or_else(|| {
            model
                .face_biases
                .as_ref()
                .map_or(0, |biases| biases[face_index] as u8)
        });

        let uvs = match uv_type {
            UvType::Geometry => [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            UvType::Vanilla => {
                if texture_id.is_some() {
                    vanilla_face_uvs(
                        model,
                        face_index,
                        [idx[0] as u32, idx[1] as u32, idx[2] as u32],
                    )
                } else {
                    [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]
                }
            }
            UvType::Box => box_uvs(
                corner_points,
                0,
                face_override.uv_scale,
                face_override.uv_orientation,
            ),
            UvType::Model { u, v, mirror } => model_uvs(corner_points, 0, (u, v), mirror),
            UvType::World(normal) => {
                let world = idx.map(|i| {
                    [
                        (positions[i][0] + site.zone_origin_world.0) as f32,
                        positions[i][1] as f32,
                        (positions[i][2] + site.zone_origin_world.1) as f32,
                    ]
                });
                world_uvs(normal, face_override.uv_scale, world)
            }
        };

        let mut flags = 0;
        if face_override.upwards_normals {
            flags |= HD_FLAG_UPWARDS_NORMALS;
        }
        if face_override.flat_normals {
            flags |= HD_FLAG_FLAT_NORMALS;
        }
        if !face_override.receive_shadows {
            flags |= HD_FLAG_NO_SHADOW_RECEIVE;
        }
        if !face_override.cast_shadows {
            flags |= HD_FLAG_NO_SHADOW_CAST;
        }
        if face_override.shadow_normal_bias {
            flags |= HD_FLAG_SHADOW_NORMAL_BIAS;
        }
        if uv_type == UvType::Vanilla && texture_id.is_some() {
            flags |= HD_FLAG_VANILLA_UVS;
        }
        let material_flags = materials.has_transparency(material);
        let double_sided = face_override.double_sided_faces || materials.double_sided(material);
        faces.push(HdFace {
            positions: idx.map(|i| positions[i]),
            uvs,
            normals,
            hsl: [c1, c2, c3].map(|c| (c & 0xFFFF) as u16),
            material: [material.min(0x0FFF) as u16; 3],
            water_depth: [0; 3],
            water_type: 0,
            plane: 0,
            flags,
            alpha: transparency as u8,
            // Materials with transparency draw in the alpha pass even when the face is opaque.
            alpha_pass: material_flags,
            bias,
            double_sided,
        });
    }
    faces
}

/// `HDUtils.isBakedGroundShading`: a mostly transparent, untextured face lying flat near the model's
/// origin is the shadow Jagex baked under the object.
fn is_baked_ground_shading(
    model: &ReferenceLitModel,
    face_index: usize,
    transparency: i32,
    textured: bool,
) -> bool {
    if transparency <= 100 || textured {
        return false;
    }
    let face = &model.faces[face_index];
    let height = |vertex: usize| model.vertices.get(vertex).map(|v| v.y);
    let (Some(a), Some(b), Some(c)) = (
        height(face.a.get() as usize),
        height(face.b.get() as usize),
        height(face.c.get() as usize),
    ) else {
        return false;
    };
    a >= -8 && a == b && a == c
}

fn hd_undo(face_override: &ResolvedModelOverride) -> bool {
    face_override.undo_vanilla_shading
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_shading_lightens_lit_faces_and_caps_brightness() {
        let color = (10 << 10) | (2 << 7) | 20;
        // The vanilla light shines along (1,1,1): faces toward it were darkened least... the
        // client darkens faces facing away, and the undo brightens those facing the light axis.
        let toward = undo_vanilla_shading(color, false, [1.0, 1.0, 1.0]);
        let away = undo_vanilla_shading(color, false, [-1.0, -1.0, -1.0]);
        assert!(toward & 0x7F > away & 0x7F, "{toward} {away}");
        assert_eq!(away, color, "faces turned from the light stay as they are");
        // Hue and saturation survive; lightness is capped by the saturation limit (59 at s=2).
        assert_eq!(toward & 0xFF80, color & 0xFF80);
        assert!(toward & 0x7F <= 59);
        assert_eq!(
            undo_vanilla_shading((3 << 7) | 120, false, [1.0, 1.0, 1.0]) & 0x7F,
            57
        );
    }

    #[test]
    fn box_uvs_follow_the_dominant_axis() {
        // A face in the XZ plane (a floor) maps x to u and z to v.
        let floor = [[0.0, 0.0, 0.0], [128.0, 0.0, 0.0], [0.0, 0.0, 128.0]];
        let uv = box_uvs(floor, 0, 1.0, [0; 3]);
        assert!((uv[1][0] - uv[0][0]).abs() > 0.5 || (uv[1][1] - uv[0][1]).abs() > 0.5);
        let spread =
            |uvs: [[f32; 2]; 3]| (uvs[1][0] - uvs[0][0]).abs() + (uvs[2][1] - uvs[0][1]).abs();
        assert!(spread(uv) > 1.0);
        // A wall face in the XY plane maps x to u and y to v.
        let wall = [[0.0, 0.0, 0.0], [128.0, 0.0, 0.0], [0.0, -128.0, 0.0]];
        let uv = box_uvs(wall, 0, 1.0, [0; 3]);
        assert!((uv[1][0] - uv[0][0]).abs() > 0.9 && (uv[2][1] - uv[0][1]).abs() > 0.9);
        // A larger uvScale stretches the texture over more geometry.
        let wide = box_uvs(wall, 0, 2.0, [0; 3]);
        assert!(((wide[1][0] - wide[0][0]).abs() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn uv_type_names_parse() {
        assert_eq!(UvType::parse("BOX"), UvType::Box);
        assert_eq!(
            UvType::parse("MODEL_XZ_MIRROR_A"),
            UvType::Model {
                u: 0,
                v: 2,
                mirror: Mirror::A
            }
        );
        assert_eq!(UvType::parse("WORLD_XZ"), UvType::World([0.0, -1.0, 0.0]));
        assert_eq!(UvType::parse("GEOMETRY"), UvType::Geometry);
    }

    #[test]
    fn colour_modifiers_clamp() {
        let o = ModelOverride {
            shift_lightness: Some(10),
            max_lightness: Some(40),
            ..ModelOverride::default()
        };
        let modifier = ColorMod::new(&o);
        assert!(modifier.modifies_color && !modifier.modifies_alpha);
        assert_eq!(modifier.color((5 << 10) | (2 << 7) | 35) & 0x7F, 40);
        assert_eq!(modifier.color((5 << 10) | (2 << 7) | 10) & 0x7F, 20);
    }
}
