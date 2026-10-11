//! Tile-level data of the 117 HD port: areas as bounding boxes, ground materials, water types and
//! the tile-override table (`AreaManager`, `GroundMaterialManager`, `WaterTypeManager`,
//! `TileOverrideManager`, `TileOverride`).
//!
//! Upstream: `rs117.hd.scene.*` (BSD 2-Clause, see `THIRD_PARTY.md`).

use crate::{
    HdData, IdRef, TileOverride,
    expr::{Expression, Value, Variables},
    materials::MaterialSet,
};
use serde_json::Value as Json;
use std::collections::{HashMap, HashSet};

/// `TileOverride.OVERLAY_FLAG`: set on overlay ids in the override lookup.
pub const OVERLAY_FLAG: i32 = i32::MIN;

// ---------------------------------------------------------------------------------------------
// Areas
// ---------------------------------------------------------------------------------------------

/// An inclusive box in world tile coordinates and planes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aabb {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

impl Aabb {
    const ALL_PLANES: (i32, i32) = (i32::MIN, i32::MAX);

    fn flat(x1: i32, y1: i32, x2: i32, y2: i32, planes: (i32, i32)) -> Self {
        Self {
            min: [x1.min(x2), y1.min(y2), planes.0],
            max: [x1.max(x2), y1.max(y2), planes.1],
        }
    }

    /// `AABB.fromRegionId`: the 64x64 tiles of a map region.
    pub fn region(id: i32) -> Self {
        let (x, y) = ((id >> 8) << 6, (id & 0xFF) << 6);
        Self::flat(x, y, x + 63, y + 63, Self::ALL_PLANES)
    }

    /// The `XY`, `XYZ`, `XYXY`, `XYXYZ` and `XYZXYZ` forms of the area JSON.
    pub fn from_numbers(values: &[i32]) -> Option<Self> {
        Some(match *values {
            [x, y] => Self::flat(x, y, x, y, Self::ALL_PLANES),
            [x, y, z] => Self::flat(x, y, x, y, (z, z)),
            [x1, y1, x2, y2] => Self::flat(x1, y1, x2, y2, Self::ALL_PLANES),
            [x1, y1, x2, y2, z] => Self::flat(x1, y1, x2, y2, (z, z)),
            [x1, y1, z1, x2, y2, z2] => Self::flat(x1, y1, x2, y2, (z1.min(z2), z1.max(z2))),
            _ => return None,
        })
    }

    pub const fn contains(&self, x: i32, y: i32, plane: i32) -> bool {
        x >= self.min[0]
            && x <= self.max[0]
            && y >= self.min[1]
            && y <= self.max[1]
            && plane >= self.min[2]
            && plane <= self.max[2]
    }
}

/// Named areas flattened to boxes.
#[derive(Debug, Clone, Default)]
pub struct Areas {
    by_name: HashMap<String, Vec<Aabb>>,
}

impl Areas {
    pub fn new(data: &HdData) -> Self {
        let raw: HashMap<&str, &crate::Area> = data
            .areas
            .iter()
            .map(|area| (area.name.as_str(), area))
            .collect();
        let mut areas = Self::default();
        for area in &data.areas {
            let mut boxes = Vec::new();
            collect_boxes(area, &raw, &mut HashSet::new(), &mut boxes);
            areas.by_name.insert(area.name.clone(), boxes);
        }
        areas
    }

    /// Boxes of an area; `None` for names that are not defined.
    pub fn boxes(&self, name: &str) -> Option<&[Aabb]> {
        self.by_name.get(name).map(Vec::as_slice)
    }

    /// Does `name` contain the world tile? `ALL` contains everything, `NONE` and unknown names
    /// nothing.
    pub fn contains(&self, name: &str, x: i32, y: i32, plane: i32) -> bool {
        match name {
            "ALL" => true,
            _ => self
                .boxes(name)
                .is_some_and(|boxes| boxes.iter().any(|b| b.contains(x, y, plane))),
        }
    }

    /// Does an inline area reference (a name or an `[x0, y0, ..]` box) contain the tile?
    pub fn json_contains(&self, reference: &Json, x: i32, y: i32, plane: i32) -> bool {
        match reference {
            Json::String(name) => self.contains(name, x, y, plane),
            Json::Number(number) => number
                .as_i64()
                .is_some_and(|id| Aabb::region(id as i32).contains(x, y, plane)),
            Json::Array(parts) => {
                let numbers: Vec<i32> = parts
                    .iter()
                    .filter_map(|p| p.as_i64().map(|n| n as i32))
                    .collect();
                Aabb::from_numbers(&numbers).is_some_and(|b| b.contains(x, y, plane))
            }
            _ => false,
        }
    }
}

fn collect_boxes(
    area: &crate::Area,
    all: &HashMap<&str, &crate::Area>,
    visiting: &mut HashSet<String>,
    out: &mut Vec<Aabb>,
) {
    if !visiting.insert(area.name.clone()) {
        return;
    }
    for numbers in &area.aabbs {
        out.extend(Aabb::from_numbers(numbers));
    }
    out.extend(area.regions.iter().map(|id| Aabb::region(*id)));
    for region_box in &area.region_boxes {
        if let &[from, to, ref rest @ ..] = region_box.as_slice() {
            let planes = match *rest {
                [plane] => (plane, plane),
                [first, second] => (first.min(second), first.max(second)),
                _ => (0, 3),
            };
            let (x1, y1, x2, y2) = (from >> 8, from & 0xFF, to >> 8, to & 0xFF);
            out.push(Aabb::flat(
                x1.min(x2) << 6,
                y1.min(y2) << 6,
                (x1.max(x2) << 6) + 63,
                (y1.max(y2) << 6) + 63,
                planes,
            ));
        }
    }
    for child in &area.areas {
        match child {
            Json::String(name) => {
                if let Some(other) = all.get(name.as_str()) {
                    collect_boxes(other, all, visiting, out);
                }
            }
            Json::Array(parts) => {
                let numbers: Vec<i32> = parts
                    .iter()
                    .filter_map(|p| p.as_i64().map(|n| n as i32))
                    .collect();
                out.extend(Aabb::from_numbers(&numbers));
            }
            _ => {}
        }
    }
    visiting.remove(&area.name);
}

// ---------------------------------------------------------------------------------------------
// Ground materials
// ---------------------------------------------------------------------------------------------

/// A set of materials a ground tile may take, chosen per position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundMaterial {
    pub name: String,
    /// Indices into the [`MaterialSet`] table.
    pub materials: Vec<usize>,
}

impl GroundMaterial {
    /// `GroundMaterial.getRandomMaterial`: `java.util.Random`'s first output seeded with a hash of
    /// the world position.
    pub fn random_material(&self, x: i32, y: i32, z: i32) -> usize {
        if self.materials.len() <= 1 {
            return self.materials.first().copied().unwrap_or(0);
        }
        let mut hash = i64::from(x);
        hash = hash.wrapping_mul(31).wrapping_add(i64::from(y));
        hash = hash.wrapping_mul(31).wrapping_add(i64::from(z));
        const MASK: i64 = (1 << 48) - 1;
        const MULTIPLIER: i64 = 0x5DEE_CE66D;
        let mut seed = (hash ^ MULTIPLIER) & MASK;
        seed = (seed.wrapping_mul(MULTIPLIER).wrapping_add(0xB)) & MASK;
        let value = (seed as u64 >> 17) as i32;
        self.materials[(value % self.materials.len() as i32) as usize]
    }
}

/// `GroundMaterialManager.GROUND_MATERIALS`: the built-ins `NONE`, `DIRT` and `UNDERWATER_GENERIC`,
/// then the JSON entries.
#[derive(Debug, Clone, Default)]
pub struct GroundMaterials {
    list: Vec<GroundMaterial>,
    by_name: HashMap<String, usize>,
}

impl GroundMaterials {
    pub const NONE: usize = 0;
    pub const DIRT: usize = 1;
    pub const UNDERWATER_GENERIC: usize = 2;

    pub fn new(data: &HdData, materials: &MaterialSet) -> Self {
        let dirt = vec![materials.index_of("DIRT_1"), materials.index_of("DIRT_2")];
        let mut set = Self::default();
        for (name, list) in [
            ("NONE", vec![0]),
            ("DIRT", dirt.clone()),
            ("UNDERWATER_GENERIC", dirt),
        ] {
            set.push(name, list);
        }
        for ground in &data.ground_materials {
            let list = ground
                .materials
                .iter()
                .map(|name| materials.index_of(name))
                .collect();
            set.push(&ground.name, list);
        }
        set
    }

    fn push(&mut self, name: &str, materials: Vec<usize>) {
        let materials = if materials.is_empty() {
            vec![0]
        } else {
            materials
        };
        self.by_name.insert(name.to_owned(), self.list.len());
        self.list.push(GroundMaterial {
            name: name.to_owned(),
            materials,
        });
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    pub fn get(&self, index: usize) -> &GroundMaterial {
        &self.list[index]
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

// ---------------------------------------------------------------------------------------------
// Water types
// ---------------------------------------------------------------------------------------------

/// Water type indices: `0` is `NONE`, the JSON entries follow in file order.
#[derive(Debug, Clone, Default)]
pub struct WaterTypes {
    by_name: HashMap<String, u8>,
    fallback: HashMap<i32, u8>,
}

impl WaterTypes {
    pub const NONE: u8 = 0;

    pub fn new(data: &HdData) -> Self {
        let mut types = Self::default();
        for (position, water) in data.water_types.iter().enumerate() {
            let index = (position + 1).min(127) as u8;
            types.by_name.insert(water.name.clone(), index);
            if let Some(texture) = water.vanilla_texture_index.filter(|texture| *texture >= 0) {
                types.fallback.insert(texture, index);
            }
        }
        types
    }

    pub fn index_of(&self, name: &str) -> u8 {
        self.by_name.get(name).copied().unwrap_or(Self::NONE)
    }

    pub fn fallback(&self, texture: i32) -> u8 {
        self.fallback.get(&texture).copied().unwrap_or(Self::NONE)
    }

    /// `ProceduralGenerator.seasonalWaterType`: the override's water type, or, for vanilla water
    /// textures, the fallback water.
    pub fn seasonal(&self, override_water: u8, texture: i32) -> u8 {
        if override_water != Self::NONE {
            return override_water;
        }
        if (130..=189).contains(&texture) || texture == 208 {
            return self.fallback(texture);
        }
        match texture {
            1 | 24 => self.index_of("WATER"),
            25 => self.index_of("SWAMP_WATER_FLAT"),
            _ => Self::NONE,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Tile overrides
// ---------------------------------------------------------------------------------------------

/// One tile override with names resolved to indices and colour limits normalised
/// (`TileOverride.normalize`).
#[derive(Debug, Clone)]
pub struct ResolvedTileOverride {
    pub name: String,
    /// Index into [`GroundMaterials`].
    pub ground_material: usize,
    pub water_type: u8,
    pub blended: bool,
    pub blended_as_opposite: bool,
    pub forced: bool,
    pub depth_tested: bool,
    pub double_sided_faces: bool,
    pub uv_orientation: i32,
    /// Reciprocal of the JSON `uvScale`.
    pub uv_scale: f32,
    pub height_offset: i32,
    /// `(shift, min, max)` of each colour channel.
    hue: (i32, i32, i32),
    saturation: (i32, i32, i32),
    lightness: (i32, i32, i32),
}

impl ResolvedTileOverride {
    /// `TileOverride.modifyColor` on a packed Jagex HSL colour.
    pub fn modify_color(&self, hsl: i32) -> i32 {
        let apply = |value: i32, (shift, min, max): (i32, i32, i32)| (value + shift).clamp(min, max);
        let h = apply((hsl >> 10) & 0x3F, self.hue);
        let s = apply((hsl >> 7) & 7, self.saturation);
        let l = apply(hsl & 0x7F, self.lightness);
        (h << 10) | (s << 7) | l
    }

    /// Does `modify_color` leave every colour unchanged?
    pub fn is_color_identity(&self) -> bool {
        self.hue == (0, 0, 63) && self.saturation == (0, 0, 7) && self.lightness == (0, 0, 127)
    }
}

/// Normalise one channel: `set` pins min and max, `forced` tiles default to the lowest value.
fn limits(
    set: Option<i32>,
    shift: Option<i32>,
    min: Option<i32>,
    max: Option<i32>,
    top: i32,
    forced_value: Option<i32>,
) -> (i32, i32, i32) {
    let (mut lo, mut hi) = (min.unwrap_or(0), max.unwrap_or(top));
    if let Some(forced) = forced_value {
        (lo, hi) = (forced, forced);
    }
    if let Some(set) = set {
        (lo, hi) = (set, set);
    }
    (shift.unwrap_or(0), lo.clamp(0, top), hi.clamp(0, top))
}

#[derive(Debug, Clone)]
struct Replacement {
    /// `None`: always true.
    condition: Option<Expression>,
    /// Index into the override table (`0` = `NONE`).
    target: usize,
}

/// The outcome of a lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverrideMatch {
    /// Override table index; `0` is `NONE`.
    pub index: usize,
    /// The lookup matched through an overlay id (so the vertex counts as an overlay one).
    pub queried_as_overlay: bool,
}

/// All tile overrides, ready for lookups.
#[derive(Debug, Clone)]
pub struct TileOverrides {
    all: Vec<ResolvedTileOverride>,
    areas: Vec<Option<String>>,
    replacements: Vec<Vec<Replacement>>,
    /// Where each entry ends up once always-true replacements are followed.
    constant_target: Vec<usize>,
    any_match: Vec<usize>,
    id_match: HashMap<i32, Vec<usize>>,
}

impl TileOverrides {
    /// Index of the built-in `NONE` override.
    pub const NONE: usize = 0;

    /// Resolve `data.tile_overrides`. `resolve_id` turns an overlay/underlay id reference into a
    /// number (the JSON uses plain integers).
    pub fn new(
        data: &HdData,
        grounds: &GroundMaterials,
        water: &WaterTypes,
        resolve_id: &dyn Fn(&IdRef) -> Option<i64>,
    ) -> Self {
        let source = &data.tile_overrides;
        let by_name: HashMap<&str, usize> = source
            .iter()
            .enumerate()
            .filter(|(_, o)| !o.name.is_empty())
            .map(|(i, o)| (o.name.as_str(), i + 1))
            .collect();

        let none = ResolvedTileOverride {
            name: "NONE".to_owned(),
            ground_material: GroundMaterials::DIRT,
            water_type: WaterTypes::NONE,
            blended: true,
            blended_as_opposite: false,
            forced: false,
            depth_tested: false,
            double_sided_faces: false,
            uv_orientation: 0,
            uv_scale: 1.0,
            height_offset: 0,
            hue: (0, 0, 63),
            saturation: (0, 0, 7),
            lightness: (0, 0, 127),
        };
        let mut all = vec![none];
        let mut area_names: Vec<Option<String>> = vec![None];
        let mut replacements: Vec<Vec<Replacement>> = vec![Vec::new()];
        for o in source {
            all.push(resolve_override(o, grounds, water));
            area_names.push(o.area.clone());
            let mut list = Vec::new();
            for (name, condition) in &o.replacements {
                let target = if name == "NONE" {
                    Self::NONE
                } else {
                    by_name.get(name.as_str()).copied().unwrap_or(Self::NONE)
                };
                let Ok(expression) = Expression::from_json(condition) else {
                    continue;
                };
                match expression.constant() {
                    Some(false) => {}
                    Some(true) => {
                        list.push(Replacement {
                            condition: None,
                            target,
                        });
                        // Nothing after an always-true condition can match.
                        break;
                    }
                    None => list.push(Replacement {
                        condition: Some(expression),
                        target,
                    }),
                }
            }
            replacements.push(list);
        }

        let mut set = Self {
            all,
            areas: area_names,
            replacements,
            constant_target: Vec::new(),
            any_match: Vec::new(),
            id_match: HashMap::new(),
        };
        set.constant_target = (0..set.all.len())
            .map(|index| set.resolve_constant(index))
            .collect();
        for (position, o) in source.iter().enumerate() {
            let index = position + 1;
            // `Area.NONE` overrides are only reachable as replacements.
            if o.area.as_deref().is_none_or(|area| area == "NONE") {
                continue;
            }
            let overlays = o
                .overlay_ids
                .iter()
                .filter_map(resolve_id)
                .map(|id| OVERLAY_FLAG | id as i32);
            let underlays = o.underlay_ids.iter().filter_map(resolve_id).map(|id| id as i32);
            let ids: Vec<i32> = overlays.chain(underlays).collect();
            if ids.is_empty() {
                set.any_match.push(index);
            } else {
                for id in ids {
                    set.id_match.entry(id).or_default().push(index);
                }
            }
        }
        set
    }

    pub fn get(&self, index: usize) -> &ResolvedTileOverride {
        &self.all[index]
    }

    pub fn len(&self) -> usize {
        self.all.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    fn resolve_constant(&self, start: usize) -> usize {
        let mut current = start;
        for _ in 0..self.all.len() {
            match self.replacements[current].first() {
                Some(Replacement {
                    condition: None,
                    target,
                }) if *target != current => current = *target,
                _ => break,
            }
        }
        current
    }

    /// `TileOverrideManager.getOverrideBeforeReplacements` for the world tile `(x, y, plane)`.
    /// `ids` are tried in order: overlay ids carry [`OVERLAY_FLAG`].
    pub fn lookup(&self, areas: &Areas, world: (i32, i32, i32), ids: &[i32]) -> OverrideMatch {
        let in_area = |index: usize| {
            self.areas[index]
                .as_deref()
                .is_some_and(|area| areas.contains(area, world.0, world.1, world.2))
        };
        let mut matched = OverrideMatch {
            index: Self::NONE,
            queried_as_overlay: false,
        };
        // `NONE` sorts last upstream: any "any match" override may replace it.
        let mut order = usize::MAX;
        'ids: for &id in ids {
            let Some(entries) = self.id_match.get(&id) else {
                continue;
            };
            for &entry in entries {
                if in_area(entry) {
                    order = entry;
                    matched = OverrideMatch {
                        index: self.constant_target[entry],
                        queried_as_overlay: id & OVERLAY_FLAG != 0,
                    };
                    break 'ids;
                }
            }
        }
        for &entry in &self.any_match {
            if entry > order {
                break;
            }
            if in_area(entry) {
                matched.index = self.constant_target[entry];
            }
        }
        matched
    }

    /// `TileOverride.resolveReplacements`: follow the first true replacement until none applies.
    pub fn resolve(&self, start: usize, vars: &dyn Variables) -> usize {
        let mut current = start;
        for _ in 0..self.all.len() {
            let next = self.replacements[current]
                .iter()
                .find(|r| r.condition.as_ref().is_none_or(|c| c.test(vars)))
                .map_or(current, |r| r.target);
            if next == current {
                break;
            }
            current = next;
        }
        current
    }

    /// Does the override resolve to itself regardless of the tile's colour?
    pub fn is_constant(&self, index: usize) -> bool {
        self.replacements[index].is_empty()
    }
}

fn resolve_override(
    o: &TileOverride,
    grounds: &GroundMaterials,
    water: &WaterTypes,
) -> ResolvedTileOverride {
    // Forced tiles replace hidden ones with white by default.
    let forced = o.forced == Some(true);
    let forced_value = |white: i32| forced.then_some(white);
    ResolvedTileOverride {
        name: o.name.clone(),
        ground_material: o
            .ground_material
            .as_deref()
            .and_then(|name| grounds.index_of(name))
            .unwrap_or(GroundMaterials::NONE),
        water_type: o
            .water_type
            .as_deref()
            .map_or(WaterTypes::NONE, |name| water.index_of(name)),
        blended: o.blended.unwrap_or(true),
        blended_as_opposite: o.blended_as_opposite == Some(true),
        forced,
        depth_tested: o.depth_tested == Some(true),
        double_sided_faces: o.double_sided_faces == Some(true),
        uv_orientation: o.uv_orientation.unwrap_or(0),
        uv_scale: 1.0 / o.uv_scale.filter(|scale| *scale != 0.0).unwrap_or(1.0),
        height_offset: o.height_offset.unwrap_or(0),
        hue: limits(o.set_hue, o.shift_hue, o.min_hue, o.max_hue, 63, forced_value(0)),
        saturation: limits(
            o.set_saturation,
            o.shift_saturation,
            o.min_saturation,
            o.max_saturation,
            7,
            forced_value(0),
        ),
        lightness: limits(
            o.set_lightness,
            o.shift_lightness,
            o.min_lightness,
            o.max_lightness,
            127,
            forced_value(127),
        ),
    }
}

/// Variables of a tile override replacement: the settings plus the tile's colour as `h`, `s`, `l`
/// (`TileOverrideVariables`: the south-western-most corner colour).
pub struct TileVariables<'a> {
    pub settings: &'a dyn Variables,
    pub hsl: [i32; 3],
}

impl Variables for TileVariables<'_> {
    fn get(&self, name: &str) -> Option<Value> {
        match name {
            "h" => Some(Value::Int(i64::from(self.hsl[0]))),
            "s" => Some(Value::Int(i64::from(self.hsl[1]))),
            "l" => Some(Value::Int(i64::from(self.hsl[2]))),
            _ => self.settings.get(name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_forms() {
        assert!(Aabb::from_numbers(&[10, 20]).unwrap().contains(10, 20, 3));
        assert!(!Aabb::from_numbers(&[10, 20]).unwrap().contains(11, 20, 0));
        let on_plane = Aabb::from_numbers(&[0, 0, 5, 5, 2]).unwrap();
        assert!(on_plane.contains(3, 3, 2) && !on_plane.contains(3, 3, 1));
        // Corners may come in either order.
        let flipped = Aabb::from_numbers(&[5, 5, 0, 0]).unwrap();
        assert!(flipped.contains(2, 2, 0));
        // Region 12850 is x=50, y=50: tiles 3200..3263.
        let region = Aabb::region(12850);
        assert!(region.contains(3200, 3200, 0) && region.contains(3263, 3263, 3));
        assert!(!region.contains(3264, 3200, 0));
    }

    #[test]
    fn random_material_is_stable_and_spread() {
        let ground = GroundMaterial {
            name: "T".into(),
            materials: vec![1, 2, 3],
        };
        assert_eq!(
            ground.random_material(3222, 3218, 0),
            ground.random_material(3222, 3218, 0)
        );
        let mut seen = HashSet::new();
        for x in 0..40 {
            seen.insert(ground.random_material(3200 + x, 3200, 0));
        }
        assert_eq!(seen.len(), 3, "all three materials occur");
        let single = GroundMaterial {
            name: "S".into(),
            materials: vec![9],
        };
        assert_eq!(single.random_material(1, 2, 3), 9);
    }

    #[test]
    fn modify_color_clamps_to_the_limits() {
        let mut o = ResolvedTileOverride {
            name: String::new(),
            ground_material: 0,
            water_type: 0,
            blended: true,
            blended_as_opposite: false,
            forced: false,
            depth_tested: false,
            double_sided_faces: false,
            uv_orientation: 0,
            uv_scale: 1.0,
            height_offset: 0,
            hue: (0, 0, 63),
            saturation: (0, 0, 7),
            lightness: (0, 0, 127),
        };
        assert!(o.is_color_identity());
        let color = (10 << 10) | (3 << 7) | 50;
        assert_eq!(o.modify_color(color), color);
        o.lightness = (40, 0, 70);
        assert_eq!(o.modify_color(color), (10 << 10) | (3 << 7) | 70);
        assert!(!o.is_color_identity());
    }
}
