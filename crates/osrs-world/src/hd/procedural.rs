//! Port of the 117 HD plugin's `ProceduralGenerator`: per-scene terrain data for the HD profile.
//!
//! Four passes over the terrain, in the plugin's order:
//!
//! 1. **main tile overrides**: resolve the tile override of every tile (underlay, overlay and
//!    both) from its ids, world position and colour;
//! 2. **underwater terrain**: find water tiles and sink the terrain below them by distance from the
//!    shore (`DEPTH_LEVEL_SLOPE`), recording water depth per vertex;
//! 3. **terrain normals**: area-weighted vertex normals over the (sunk) terrain;
//! 4. **terrain data**: per-vertex blended colour and ground material, and which vertices belong to
//!    overlays or underlays.
//!
//! Vertices are identified by their scene-local `(x, height, z)`, so tiles of one level share data
//! along their edges while different levels (a bridge over ground) stay apart.
//!
//! Adapted from `rs117.hd.scene.ProceduralGenerator`. Copyright (c) 2021, 117 (BSD 2-Clause, see
//! `THIRD_PARTY.md`).

use super::HdContext;
use osrs_hd::tiles::{OVERLAY_FLAG, OverrideMatch, TileOverrides, TileVariables, WaterTypes};
use osrs_scene::terrain::{TerrainColorSource, TerrainSurface};
use std::collections::HashMap;

/// `HDUtils.HIDDEN_HSL`: the colour of terrain the client does not draw.
pub const HIDDEN_HSL: i32 = 12_345_678;
/// `HDUtils.UNDERWATER_HSL`.
pub const UNDERWATER_HSL: i32 = 6_676;
/// `ProceduralGenerator.DEPTH_LEVEL_SLOPE`: depth of the terrain by distance level from the shore.
pub const DEPTH_LEVEL_SLOPE: [i32; 12] = [
    150, 300, 470, 610, 700, 750, 820, 920, 1080, 1300, 1350, 1380,
];
const TILE: i32 = 128;
const PLANES: usize = 4;

pub const VERTEX_LAND: u8 = 1;
pub const VERTEX_WATER: u8 = 1 << 1;
pub const VERTEX_HIGH_PRIORITY_COLOR: u8 = 1 << 2;
pub const VERTEX_OVERLAY: u8 = 1 << 3;
pub const VERTEX_UNDERLAY: u8 = 1 << 4;

/// Tile flag: the tile contains water.
pub const TILE_WATER: u8 = 1;
/// Tile flag: the tile is skipped because water lies below it on another plane.
pub const TILE_SKIP: u8 = 1 << 1;

/// Scene-local `(x, height, z)` of a terrain vertex.
pub type VertexKey = (i32, i32, i32);

/// Which of a tile's three overrides to read (`SceneContext.TILE_OVERRIDE_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrideSlot {
    Main = 0,
    Underlay = 1,
    Overlay = 2,
}

/// One terrain tile of the scene as the generator sees it.
#[derive(Debug, Clone, Copy)]
pub struct TileInput<'a> {
    pub x: i32,
    pub y: i32,
    /// Original plane (`Tile.getRenderLevel`), unchanged by bridge relinking.
    pub level: u8,
    pub surface: &'a TerrainSurface,
    /// Overlay id as stored in the scene (`0` = none).
    pub overlay_id: i32,
    /// Underlay id as stored in the scene (`0` = none).
    pub underlay_id: i32,
}

/// A scene tile slot: the tile, and the tile below it when it is a bridge.
#[derive(Debug, Clone, Copy)]
pub struct SceneTileInput<'a> {
    /// Storage plane of the slot.
    pub plane: u8,
    pub tile: Option<TileInput<'a>>,
    pub bridge: Option<TileInput<'a>>,
}

/// The terrain of a scene window.
pub struct ProceduralInput<'a> {
    /// Scene size in tiles.
    pub size: (i32, i32),
    /// World tile of scene tile `(0, 0)`.
    pub base: (i32, i32),
    /// In generation order: plane, then x, then y.
    pub tiles: Vec<SceneTileInput<'a>>,
}

/// One triangle of a terrain tile in scene-local vertex keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileFace {
    pub keys: [VertexKey; 3],
    pub colors: [i32; 3],
    pub overlay: bool,
    /// Vanilla texture id, `-1` when untextured.
    pub texture: i32,
}

/// Triangles of a tile in upload order, plus the paint corners when it is a flat paint tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileGeometry {
    pub faces: Vec<TileFace>,
    /// `[sw, se, nw, ne]` (`ProceduralGenerator.tileVertexKeys`) for paint tiles.
    pub corners: Option<[VertexKey; 4]>,
    /// Flat paint colours `[sw, se, nw, ne]`.
    pub corner_colors: [i32; 4],
    pub texture: i32,
}

/// Triangles and vertex keys of a tile (`tileVertexKeys` / `faceVertexKeys`).
///
/// Paint tiles upload `ne nw se` then `sw se nw`; tile models follow the shape tables of the
/// scene builder.
pub fn tile_geometry(surface: &TerrainSurface, x: i32, y: i32) -> TileGeometry {
    match surface {
        TerrainSurface::Flat(flat) => {
            let (x0, z0) = (x * TILE, y * TILE);
            let key = |dx: i32, dz: i32, height: i32| (x0 + dx, height, z0 + dz);
            let sw = key(0, 0, flat.heights.southwest);
            let se = key(TILE, 0, flat.heights.southeast);
            let nw = key(0, TILE, flat.heights.northwest);
            let ne = key(TILE, TILE, flat.heights.northeast);
            let c = flat.colors;
            let texture = flat.texture_id.unwrap_or(-1);
            TileGeometry {
                faces: vec![
                    TileFace {
                        keys: [ne, nw, se],
                        colors: [c.northeast, c.northwest, c.southeast],
                        overlay: false,
                        texture,
                    },
                    TileFace {
                        keys: [sw, se, nw],
                        colors: [c.southwest, c.southeast, c.northwest],
                        overlay: false,
                        texture,
                    },
                ],
                corners: Some([sw, se, nw, ne]),
                corner_colors: [c.southwest, c.southeast, c.northwest, c.northeast],
                texture,
            }
        }
        TerrainSurface::Shaped(shaped) => {
            let key = |index: usize| {
                let p = shaped.vertices[index].position;
                (p.x.units(), p.y.units(), p.z.units())
            };
            let faces = shaped
                .faces
                .iter()
                .map(|face| TileFace {
                    keys: face.indices.map(key),
                    colors: face.colors,
                    overlay: face.color_source == TerrainColorSource::Overlay,
                    texture: face.texture_id.unwrap_or(-1),
                })
                .collect();
            TileGeometry {
                faces,
                corners: None,
                corner_colors: [0; 4],
                texture: -1,
            }
        }
    }
}

/// Does the tile carry a vanilla texture (`ProceduralGenerator.useDefaultColor`'s first test)?
pub fn has_vanilla_texture(surface: &TerrainSurface) -> bool {
    match surface {
        TerrainSurface::Flat(flat) => flat.texture_id.is_some_and(|id| id >= 0),
        TerrainSurface::Shaped(shaped) => shaped.faces.iter().any(|f| f.texture_id.is_some()),
    }
}

/// `HDUtils.getSouthWesternMostTileColor`: the colour that decides colour-conditional overrides.
pub fn south_western_color(surface: &TerrainSurface, x: i32, y: i32) -> i32 {
    match surface {
        TerrainSurface::Flat(flat) => flat.colors.southwest,
        TerrainSurface::Shaped(_) => {
            let (base_x, base_z) = (x * TILE, y * TILE);
            let mut hsl = 0;
            'faces: for face in tile_geometry(surface, x, y).faces {
                if face.overlay {
                    continue;
                }
                for corner in 0..3 {
                    hsl = face.colors[corner];
                    let key = face.keys[corner];
                    if key.0 - base_x != TILE && key.2 - base_z != TILE {
                        break 'faces;
                    }
                }
            }
            hsl
        }
    }
}

/// Everything the generator derives from the scene.
#[derive(Debug, Default)]
pub struct ProceduralData {
    size: (i32, i32),
    /// `[level][x][y]` -> `[main, underlay, overlay]` override matches.
    overrides: Vec<[OverrideMatch; 3]>,
    tile_flags: Vec<u8>,
    vertices: HashMap<VertexKey, VertexData>,
}

/// Per-vertex terrain data.
#[derive(Debug, Clone, Default)]
pub struct VertexData {
    pub flags: u8,
    pub underwater_depth: i32,
    normal_sum: [i64; 3],
    pub normal: [f32; 3],
    /// Blended terrain colour (Jagex HSL), when a tile wrote one.
    pub color: Option<i32>,
    /// Blended ground material (table index).
    pub material: Option<u16>,
}

impl VertexData {
    pub const fn is(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }
}

const NO_MATCH: OverrideMatch = OverrideMatch {
    index: TileOverrides::NONE,
    queried_as_overlay: false,
};

impl ProceduralData {
    fn tile_index(&self, plane: usize, x: i32, y: i32) -> usize {
        (plane * self.size.0 as usize + x as usize) * self.size.1 as usize + y as usize
    }

    /// The override match of a tile (`NONE` where nothing was generated).
    pub fn tile_override(&self, level: u8, x: i32, y: i32, slot: OverrideSlot) -> OverrideMatch {
        if x < 0 || y < 0 || x >= self.size.0 || y >= self.size.1 {
            return NO_MATCH;
        }
        self.overrides
            .get(self.tile_index(usize::from(level), x, y))
            .map_or(NO_MATCH, |matches| matches[slot as usize])
    }

    pub fn tile_flags(&self, plane: u8, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.size.0 || y >= self.size.1 {
            return 0;
        }
        self.tile_flags
            .get(self.tile_index(usize::from(plane), x, y))
            .copied()
            .unwrap_or(0)
    }

    pub fn vertex(&self, key: VertexKey) -> Option<&VertexData> {
        self.vertices.get(&key)
    }

    /// Unit vertex normal, `(0, -1, 0)` (up) where none was computed.
    pub fn normal(&self, key: VertexKey) -> [f32; 3] {
        self.vertices
            .get(&key)
            .filter(|v| v.normal != [0.0; 3])
            .map_or([0.0, -1.0, 0.0], |v| v.normal)
    }

    pub fn underwater_depth(&self, key: VertexKey) -> i32 {
        self.vertices.get(&key).map_or(0, |v| v.underwater_depth)
    }

    pub fn has(&self, key: VertexKey, flag: u8) -> bool {
        self.vertices.get(&key).is_some_and(|v| v.is(flag))
    }

    /// Run the four passes over `input`.
    pub fn generate(input: &ProceduralInput<'_>, ctx: &HdContext) -> Self {
        let (width, height) = input.size;
        let mut data = Self {
            size: input.size,
            overrides: vec![[NO_MATCH; 3]; PLANES * width as usize * height as usize],
            tile_flags: vec![0; PLANES * width as usize * height as usize],
            vertices: HashMap::new(),
        };
        data.main_tile_overrides(input, ctx);
        data.underwater_terrain(input, ctx);
        data.terrain_normals(input);
        data.terrain_data(input, ctx);
        data
    }

    // -- pass 1 ---------------------------------------------------------------------------------

    fn main_tile_overrides(&mut self, input: &ProceduralInput<'_>, ctx: &HdContext) {
        for slot in &input.tiles {
            for tile in [slot.tile, slot.bridge].into_iter().flatten() {
                let world = (
                    input.base.0 + tile.x,
                    input.base.1 + tile.y,
                    i32::from(tile.level),
                );
                let overlay = OVERLAY_FLAG | tile.overlay_id;
                let hsl = unpack_hsl(south_western_color(tile.surface, tile.x, tile.y));
                let vars = TileVariables {
                    settings: &ctx.settings,
                    hsl,
                };
                let resolve = |ids: &[i32]| -> OverrideMatch {
                    let mut found = ctx.tile_overrides.lookup(&ctx.areas, world, ids);
                    if !ctx.tile_overrides.is_constant(found.index) {
                        found.index = ctx.tile_overrides.resolve(found.index, &vars);
                    }
                    found
                };
                let matches = [
                    resolve(&[overlay, tile.underlay_id]),
                    resolve(&[tile.underlay_id]),
                    resolve(&[overlay]),
                ];
                let index = self.tile_index(usize::from(tile.level), tile.x, tile.y);
                self.overrides[index] = matches;
            }
        }
    }

    // -- pass 2 ---------------------------------------------------------------------------------

    fn seasonal_water(&self, ctx: &HdContext, override_index: usize, texture: i32) -> u8 {
        ctx.water
            .seasonal(ctx.tile_overrides.get(override_index).water_type, texture)
    }

    #[allow(clippy::too_many_lines)]
    fn underwater_terrain(&mut self, input: &ProceduralInput<'_>, ctx: &HdContext) {
        let (width, height) = input.size;
        let (w, h) = (width as usize, height as usize);
        // Depth levels per plane over the (w+1) x (h+1) vertex grid; `1` assumes water until a
        // land vertex resets it.
        let mut levels = vec![vec![vec![1_u8; h + 1]; w + 1]; PLANES];
        let mut bounds = [(w as i32, h as i32, 0_i32, 0_i32); PLANES];
        let (mut min_plane, mut max_plane) = (PLANES, 0);

        for slot in &input.tiles {
            let z = usize::from(slot.plane);
            let Some(main) = slot.tile else {
                continue;
            };
            let (x, y) = (main.x as usize, main.y as usize);
            let zero_corners = |levels: &mut Vec<Vec<Vec<u8>>>| {
                levels[z][x][y] = 0;
                levels[z][x + 1][y] = 0;
                levels[z][x][y + 1] = 0;
                levels[z][x + 1][y + 1] = 0;
            };
            // A bridge's water decision comes from the ground below it.
            let tile = slot.bridge.unwrap_or(main);
            let level = tile.level;
            let geometry = tile_geometry(tile.surface, tile.x, tile.y);

            if let Some(corners) = geometry.corners {
                let override_match = self.tile_override(level, tile.x, tile.y, OverrideSlot::Main);
                let water = self.seasonal_water(ctx, override_match.index, geometry.texture);
                if water == WaterTypes::NONE {
                    let visible = geometry.corner_colors[3] != HIDDEN_HSL
                        || ctx.tile_overrides.get(override_match.index).forced;
                    if visible {
                        for key in corners {
                            self.vertex_mut(key).flags |= VERTEX_LAND;
                        }
                    }
                    zero_corners(&mut levels);
                } else {
                    if z > 0 && self.water_below(z, x as i32, y as i32) {
                        zero_corners(&mut levels);
                        let index = self.tile_index(z, x as i32, y as i32);
                        self.tile_flags[index] |= TILE_SKIP;
                        continue;
                    }
                    self.mark_water_tile(
                        z,
                        x as i32,
                        y as i32,
                        &mut bounds,
                        &mut min_plane,
                        &mut max_plane,
                    );
                    for key in corners {
                        self.vertex_mut(key).flags |= VERTEX_WATER;
                    }
                }
            } else {
                let overlay_match =
                    self.tile_override(level, tile.x, tile.y, OverrideSlot::Overlay);
                let underlay_match =
                    self.tile_override(level, tile.x, tile.y, OverrideSlot::Underlay);
                let face_water = |this: &Self, face: &TileFace| {
                    let index = if face.overlay {
                        overlay_match.index
                    } else {
                        underlay_match.index
                    };
                    (
                        this.seasonal_water(ctx, index, face.texture),
                        ctx.tile_overrides.get(index).forced,
                    )
                };
                if z > 0 {
                    let includes_water = geometry
                        .faces
                        .iter()
                        .any(|face| face_water(self, face).0 != WaterTypes::NONE);
                    if includes_water && self.water_below(z, x as i32, y as i32) {
                        zero_corners(&mut levels);
                        let index = self.tile_index(z, x as i32, y as i32);
                        self.tile_flags[index] |= TILE_SKIP;
                        continue;
                    }
                }
                for face in &geometry.faces {
                    let (water, forced) = face_water(self, face);
                    if water == WaterTypes::NONE {
                        for key in face.keys {
                            if face.colors[0] != HIDDEN_HSL || forced {
                                self.vertex_mut(key).flags |= VERTEX_LAND;
                            }
                            if key.0 % TILE == 0 && key.2 % TILE == 0 {
                                let (vx, vy) = ((key.0 / TILE) as usize, (key.2 / TILE) as usize);
                                if let Some(cell) =
                                    levels[z].get_mut(vx).and_then(|c| c.get_mut(vy))
                                {
                                    *cell = 0;
                                }
                            }
                        }
                    } else {
                        self.mark_water_tile(
                            z,
                            x as i32,
                            y as i32,
                            &mut bounds,
                            &mut min_plane,
                            &mut max_plane,
                        );
                        for key in face.keys {
                            self.vertex_mut(key).flags |= VERTEX_WATER;
                        }
                    }
                }
            }
        }
        // Tiles that do not exist cannot be water.
        let mut present = vec![vec![false; h]; w * PLANES];
        for slot in &input.tiles {
            if let Some(tile) = slot.tile {
                present[usize::from(slot.plane) * w + tile.x as usize][tile.y as usize] = true;
            }
        }
        for z in 0..PLANES {
            for x in 0..w {
                for y in 0..h {
                    if !present[z * w + x][y] {
                        levels[z][x][y] = 0;
                        levels[z][x + 1][y] = 0;
                        levels[z][x][y + 1] = 0;
                        levels[z][x + 1][y + 1] = 0;
                    }
                }
            }
        }

        if min_plane > max_plane {
            return;
        }
        // Sink the terrain further from the shore, one level per pass.
        for _ in 0..DEPTH_LEVEL_SLOPE.len() - 1 {
            for z in min_plane..=max_plane {
                let (min_x, min_y, max_x, max_y) = bounds[z];
                for x in min_x..=max_x {
                    for y in min_y..=max_y {
                        let (xu, yu) = (x as usize, y as usize);
                        let level = levels[z][xu][yu];
                        if level == 0 {
                            continue;
                        }
                        // The scene edge becomes a wall so fog cannot pass through.
                        if xu == 0 || yu == 0 || xu == w || yu == h {
                            levels[z][xu][yu] = 0;
                            continue;
                        }
                        if levels[z][xu - 1][yu] < level
                            || levels[z][xu][yu - 1] < level
                            || (xu < w && levels[z][xu + 1][yu] < level)
                            || (yu < h && levels[z][xu][yu + 1] < level)
                        {
                            continue;
                        }
                        levels[z][xu][yu] += 1;
                    }
                }
            }
        }

        let offset = |z: usize, x: i32, y: i32| -> i32 {
            let level = levels[z][x as usize][y as usize];
            if level == 0 {
                return 0;
            }
            let depth = DEPTH_LEVEL_SLOPE[usize::from(level) - 1];
            (depth as f32 * 0.55) as i32
        };

        for slot in &input.tiles {
            let z = usize::from(slot.plane);
            let Some(main) = slot.tile else { continue };
            if z < min_plane || z > max_plane {
                continue;
            }
            let (x, y) = (main.x, main.y);
            if self.tile_flags[self.tile_index(z, x, y)] & TILE_WATER == 0 {
                continue;
            }
            let tile = slot.bridge.unwrap_or(main);
            let geometry = tile_geometry(tile.surface, tile.x, tile.y);
            if let Some(corners) = geometry.corners {
                let depths = [
                    offset(z, x, y),
                    offset(z, x + 1, y),
                    offset(z, x, y + 1),
                    offset(z, x + 1, y + 1),
                ];
                for (key, depth) in corners.into_iter().zip(depths) {
                    self.vertex_mut(key).underwater_depth = depth;
                }
            } else {
                for face in &geometry.faces {
                    for key in face.keys {
                        if key.0 % TILE == 0 && key.2 % TILE == 0 {
                            // A tile corner: the grid value.
                            let (vx, vy) = (key.0 / TILE, key.2 / TILE);
                            let depth = offset(z, vx, vy);
                            self.vertex_mut(key).underwater_depth = depth;
                        } else {
                            // Interpolate between the tile's corners.
                            let lerp_x = (key.0 as f32 / TILE as f32).fract();
                            let lerp_y = (key.2 as f32 / TILE as f32).fract();
                            let mix = |a: i32, b: i32, t: f32| a as f32 + (b - a) as f32 * t;
                            let north = mix(offset(z, x, y + 1), offset(z, x + 1, y + 1), lerp_x);
                            let south = mix(offset(z, x, y), offset(z, x + 1, y), lerp_x);
                            let depth = (south + (north - south) * lerp_y) as i32;
                            if !self.has(key, VERTEX_LAND) {
                                self.vertex_mut(key).underwater_depth = depth;
                            }
                        }
                    }
                }
            }
        }
    }

    fn water_below(&self, z: usize, x: i32, y: i32) -> bool {
        (0..z).any(|below| self.tile_flags[self.tile_index(below, x, y)] & TILE_WATER != 0)
    }

    fn mark_water_tile(
        &mut self,
        z: usize,
        x: i32,
        y: i32,
        bounds: &mut [(i32, i32, i32, i32); PLANES],
        min_plane: &mut usize,
        max_plane: &mut usize,
    ) {
        let index = self.tile_index(z, x, y);
        self.tile_flags[index] |= TILE_WATER;
        *min_plane = (*min_plane).min(z);
        *max_plane = (*max_plane).max(z);
        let b = &mut bounds[z];
        b.0 = b.0.min(x);
        b.1 = b.1.min(y);
        b.2 = b.2.max(x);
        b.3 = b.3.max(y);
    }

    fn vertex_mut(&mut self, key: VertexKey) -> &mut VertexData {
        self.vertices.entry(key).or_default()
    }

    // -- pass 3 ---------------------------------------------------------------------------------

    fn terrain_normals(&mut self, input: &ProceduralInput<'_>) {
        for slot in &input.tiles {
            // The bridge's ground comes first, then the deck above it.
            if let Some(bridge) = slot.bridge {
                self.tile_normals(bridge, false);
            }
            if let Some(tile) = slot.tile {
                self.tile_normals(tile, slot.bridge.is_some());
            }
        }
        for vertex in self.vertices.values_mut() {
            let [x, y, z] = vertex.normal_sum.map(|c| c as f64);
            let length = (x * x + y * y + z * z).sqrt();
            if length > 0.0 {
                vertex.normal = [
                    (x / length) as f32,
                    (y / length) as f32,
                    (z / length) as f32,
                ];
            }
        }
    }

    fn tile_normals(&mut self, tile: TileInput<'_>, is_bridge: bool) {
        for face in tile_geometry(tile.surface, tile.x, tile.y).faces {
            let mut points = face.keys;
            if !is_bridge {
                for point in &mut points {
                    point.1 += self.underwater_depth(*point);
                }
            }
            let normal = surface_normal(points);
            for key in face.keys {
                let sum = &mut self.vertex_mut(key).normal_sum;
                for axis in 0..3 {
                    sum[axis] += normal[axis];
                }
            }
        }
    }

    // -- pass 4 ---------------------------------------------------------------------------------

    fn terrain_data(&mut self, input: &ProceduralInput<'_>, ctx: &HdContext) {
        for slot in &input.tiles {
            for tile in [slot.tile, slot.bridge].into_iter().flatten() {
                self.tile_terrain_data(input, tile, ctx);
            }
        }
    }

    fn tile_terrain_data(
        &mut self,
        input: &ProceduralInput<'_>,
        tile: TileInput<'_>,
        ctx: &HdContext,
    ) {
        let geometry = tile_geometry(tile.surface, tile.x, tile.y);
        let world = (
            input.base.0 + tile.x,
            input.base.1 + tile.y,
            i32::from(tile.level),
        );
        // (key, colour, override match, is overlay vertex, keeps the default colour)
        struct Entry {
            key: VertexKey,
            color: i32,
            index: usize,
            is_overlay: bool,
            default_color: bool,
        }
        let mut entries: Vec<Entry> = Vec::new();
        let textured = has_vanilla_texture(tile.surface);
        let default_color = |index: usize| {
            textured || (index != TileOverrides::NONE && !ctx.tile_overrides.get(index).blended)
        };

        if let Some(corners) = geometry.corners {
            let main = self.tile_override(tile.level, tile.x, tile.y, OverrideSlot::Main);
            if ctx.tile_overrides.get(main.index).water_type != WaterTypes::NONE {
                return;
            }
            let colors = geometry.corner_colors;
            let skip = default_color(main.index);
            for (key, color) in corners.into_iter().zip(colors) {
                entries.push(Entry {
                    key,
                    color,
                    index: main.index,
                    is_overlay: main.queried_as_overlay,
                    default_color: skip,
                });
            }
        } else {
            let overlay_match =
                self.tile_override(tile.level, tile.x, tile.y, OverrideSlot::Overlay);
            let underlay_match =
                self.tile_override(tile.level, tile.x, tile.y, OverrideSlot::Underlay);
            for face in &geometry.faces {
                let found = if face.overlay {
                    overlay_match
                } else {
                    underlay_match
                };
                if ctx.tile_overrides.get(found.index).water_type != WaterTypes::NONE {
                    continue;
                }
                for corner in 0..3 {
                    entries.push(Entry {
                        key: face.keys[corner],
                        color: face.colors[corner],
                        index: found.index,
                        is_overlay: face.overlay,
                        default_color: face.overlay && default_color(found.index),
                    });
                }
            }
        }

        for entry in entries {
            let override_data = ctx.tile_overrides.get(entry.index);
            let mut color = entry.color;
            if color < 0 || (color == HIDDEN_HSL && !override_data.forced) {
                continue;
            }
            // Near-solid-black tiles under wall objects must not win over real colours.
            let low_priority = entry.color <= 2;
            let normal = self.vertices.get(&entry.key).map_or([0.0; 3], |v| v.normal);
            color = relight(color, normal);
            color = override_data.modify_color(color);

            let ground = ctx.grounds.get(override_data.ground_material);
            let material = ground.random_material(world.0, world.1, world.2) as u16;
            let is_overlay = entry.is_overlay != override_data.blended_as_opposite;

            let vertex = self.vertex_mut(entry.key);
            vertex.flags |= if is_overlay {
                VERTEX_OVERLAY
            } else {
                VERTEX_UNDERLAY
            };
            if (!low_priority || !vertex.is(VERTEX_HIGH_PRIORITY_COLOR)) && !entry.default_color {
                if is_overlay || vertex.color.is_none() {
                    vertex.color = Some(color);
                    vertex.material = Some(material);
                } else {
                    vertex.color.get_or_insert(color);
                    vertex.material.get_or_insert(material);
                }
                if !low_priority {
                    vertex.flags |= VERTEX_HIGH_PRIORITY_COLOR;
                }
            }
        }
    }
}

/// Approximately reverse the vanilla tile lighting on a terrain colour using the vertex normal,
/// then cap the lightness (`TerrainDataGenerator`).
pub fn relight(color: i32, normal: [f32; 3]) -> i32 {
    const LIGHTEN_MULTIPLIER: f32 = 1.5;
    const LIGHTEN_BASE: i32 = 15;
    const LIGHTEN_ADD: i32 = 3;
    const DARKEN_MULTIPLIER: f32 = 0.5;

    let length_squared = normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2];
    let dot = if length_squared >= f32::EPSILON {
        (normal[0] + normal[2]) / (2.0 * length_squared).sqrt()
    } else {
        0.0
    };
    let mut lightness = color & 0x7F;
    let light_factor = dot.max(0.0);
    let adjusted =
        ((lightness - LIGHTEN_ADD).max(0) as f32 * LIGHTEN_MULTIPLIER) as i32 + LIGHTEN_BASE;
    lightness = mix(lightness as f32, adjusted as f32, light_factor) as i32;
    let dark_factor = -dot.min(0.0);
    let darkened = (lightness as f32 * DARKEN_MULTIPLIER) as i32;
    lightness = (1.25 * mix(lightness as f32, darkened as f32, dark_factor)) as i32;
    lightness = lightness.min(55);
    (color & !0x7F) | lightness
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn unpack_hsl(color: i32) -> [i32; 3] {
    [(color >> 10) & 63, (color >> 7) & 7, color & 127]
}

/// `HDUtils.calculateSurfaceNormals`: the (unnormalised, area-weighted) face normal.
pub fn surface_normal(points: [VertexKey; 3]) -> [i64; 3] {
    let [a, b, c] = points.map(|p| [i64::from(p.0), i64::from(p.1), i64::from(p.2)]);
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use osrs_scene::terrain::{FlatTerrainSurface, TerrainCorners};

    fn flat(heights: [i32; 4], color: i32) -> TerrainSurface {
        let corners =
            |values: [i32; 4]| TerrainCorners::new(values[0], values[1], values[2], values[3]);
        TerrainSurface::Flat(FlatTerrainSurface::new(
            corners(heights),
            corners([color; 4]),
            None,
        ))
    }

    #[test]
    fn flat_geometry_follows_the_upload_order() {
        let geometry = tile_geometry(&flat([0, 10, 20, 30], 500), 2, 3);
        let corners = geometry.corners.expect("paint tile");
        assert_eq!(corners[0], (256, 0, 384), "sw");
        assert_eq!(corners[1], (384, 10, 384), "se");
        assert_eq!(corners[2], (256, 30, 512), "nw");
        assert_eq!(corners[3], (384, 20, 512), "ne");
        assert_eq!(geometry.faces[0].keys, [corners[3], corners[2], corners[1]]);
        assert_eq!(geometry.faces[1].keys, [corners[0], corners[1], corners[2]]);
    }

    #[test]
    fn level_ground_faces_up() {
        // Winding ne, nw, se over flat ground points toward negative y (up in game space).
        let geometry = tile_geometry(&flat([0; 4], 500), 0, 0);
        let normal = surface_normal(geometry.faces[0].keys);
        assert_eq!(normal[0], 0);
        assert_eq!(normal[2], 0);
        assert!(normal[1] < 0);
        let normal = surface_normal(geometry.faces[1].keys);
        assert!(normal[1] < 0, "both triangles face the same way");
    }

    // ---- synthetic scenes -----------------------------------------------------------------

    use crate::hd::HdContext;
    use osrs_hd::{
        GroundMaterial as GroundJson, HdData, IdRef, Material, TileOverride, WaterType,
        settings::HdSettings,
    };

    fn material(name: &str) -> Material {
        Material {
            name: name.to_owned(),
            ..Material::default()
        }
    }

    fn override_for(name: &str, underlay: i64, ground: &str, water: Option<&str>) -> TileOverride {
        TileOverride {
            name: name.to_owned(),
            area: Some("ALL".to_owned()),
            underlay_ids: vec![IdRef::Number(underlay)],
            ground_material: (!ground.is_empty()).then(|| ground.to_owned()),
            water_type: water.map(str::to_owned),
            ..TileOverride::default()
        }
    }

    /// Underlay 1 = grass, 2 = dirt, 3 = water.
    fn context() -> HdContext {
        let data = HdData {
            materials: vec![material("GRASS_1"), material("DIRT_1"), material("DIRT_2")],
            ground_materials: vec![
                GroundJson {
                    name: "GRASS".into(),
                    materials: vec!["GRASS_1".into()],
                    ..GroundJson::default()
                },
                GroundJson {
                    name: "PATH".into(),
                    materials: vec!["DIRT_1".into()],
                    ..GroundJson::default()
                },
            ],
            tile_overrides: vec![
                override_for("GRASS_T", 1, "GRASS", None),
                override_for("PATH_T", 2, "PATH", None),
                override_for("WATER_T", 3, "", Some("WATER")),
            ],
            water_types: vec![WaterType {
                name: "WATER".into(),
                ..WaterType::default()
            }],
            ..HdData::default()
        };
        HdContext::new(data, HdSettings::default(), [])
    }

    /// A `size` x `size` flat scene whose tile `(x, y)` has underlay `underlay(x, y)`.
    fn scene(
        size: i32,
        underlay: impl Fn(i32, i32) -> i32,
    ) -> (Vec<TerrainSurface>, Vec<(i32, i32, i32)>) {
        let mut surfaces = Vec::new();
        let mut ids = Vec::new();
        for x in 0..size {
            for y in 0..size {
                surfaces.push(flat([0; 4], (8 << 10) | (3 << 7) | 40));
                ids.push((x, y, underlay(x, y)));
            }
        }
        (surfaces, ids)
    }

    fn generate(
        surfaces: &[TerrainSurface],
        ids: &[(i32, i32, i32)],
        size: i32,
        ctx: &HdContext,
    ) -> ProceduralData {
        let tiles = surfaces
            .iter()
            .zip(ids)
            .map(|(surface, &(x, y, underlay))| SceneTileInput {
                plane: 0,
                tile: Some(TileInput {
                    x,
                    y,
                    level: 0,
                    surface,
                    overlay_id: 0,
                    underlay_id: underlay,
                }),
                bridge: None,
            })
            .collect();
        ProceduralData::generate(
            &ProceduralInput {
                size: (size, size),
                base: (3200, 3200),
                tiles,
            },
            ctx,
        )
    }

    #[test]
    fn terrain_normals_tilt_towards_the_slope() {
        let ctx = context();
        // A ramp rising to the east: height 0 at x = 0 falling to -128 (up) at x = 2.
        let ramp = |x: i32| -64 * x;
        let surfaces: Vec<TerrainSurface> = (0..2)
            .map(|x| {
                let (west, east) = (ramp(x), ramp(x + 1));
                let corners = |sw, se, ne, nw| TerrainCorners::new(sw, se, ne, nw);
                TerrainSurface::Flat(FlatTerrainSurface::new(
                    corners(west, east, east, west),
                    corners(500, 500, 500, 500),
                    None,
                ))
            })
            .collect();
        let tiles = surfaces
            .iter()
            .enumerate()
            .map(|(x, surface)| SceneTileInput {
                plane: 0,
                tile: Some(TileInput {
                    x: x as i32,
                    y: 0,
                    level: 0,
                    surface,
                    overlay_id: 0,
                    underlay_id: 0,
                }),
                bridge: None,
            })
            .collect();
        let data = ProceduralData::generate(
            &ProceduralInput {
                size: (2, 1),
                base: (3200, 3200),
                tiles,
            },
            &ctx,
        );
        let normal = data.normal((128, ramp(1), 0));
        // Rising to the east means the surface faces up and towards the west (-x).
        assert!(normal[1] < -0.85, "{normal:?}");
        assert!(normal[0] < -0.4, "{normal:?}");
        assert!((normal.iter().map(|c| c * c).sum::<f32>() - 1.0).abs() < 1e-3);
        // A vertex nobody touches faces up.
        assert_eq!(data.normal((9999, 0, 9999)), [0.0, -1.0, 0.0]);
    }

    #[test]
    fn override_resolution_picks_the_ground_by_underlay_id() {
        let ctx = context();
        let (surfaces, ids) = scene(4, |x, _| if x < 2 { 1 } else { 2 });
        let data = generate(&surfaces, &ids, 4, &ctx);
        let grass = ctx.grounds.index_of("GRASS").expect("grass ground");
        let path = ctx.grounds.index_of("PATH").expect("path ground");
        let ground_at = |x: i32, y: i32| {
            let found = data.tile_override(0, x, y, OverrideSlot::Main);
            ctx.tile_overrides.get(found.index).ground_material
        };
        assert_eq!(ground_at(0, 0), grass);
        assert_eq!(ground_at(1, 3), grass);
        assert_eq!(ground_at(2, 0), path);
        assert_eq!(ground_at(3, 3), path);
    }

    #[test]
    fn ground_blending_shares_materials_along_shared_vertices() {
        let ctx = context();
        let (surfaces, ids) = scene(4, |x, _| if x < 2 { 1 } else { 2 });
        let data = generate(&surfaces, &ids, 4, &ctx);
        let grass = ctx.materials.index_of("GRASS_1") as u16;
        let dirt = ctx.materials.index_of("DIRT_1") as u16;
        // Vertices strictly inside one ground keep its material.
        assert_eq!(data.vertex((0, 0, 0)).and_then(|v| v.material), Some(grass));
        assert_eq!(
            data.vertex((512, 0, 256)).and_then(|v| v.material),
            Some(dirt)
        );
        // The vertex line x = 2 is shared by a grass and a dirt tile: it must carry one of the two
        // materials (the tile processed last wins for underlays), and every vertex on it the same.
        let border: Vec<Option<u16>> = (0..=4)
            .map(|y| data.vertex((256, 0, y * 128)).and_then(|v| v.material))
            .collect();
        assert!(
            border.iter().all(|m| *m == Some(grass) || *m == Some(dirt)),
            "{border:?}"
        );
        assert!(
            border[1..4].windows(2).all(|pair| pair[0] == pair[1]),
            "{border:?}"
        );
        // Every vertex got a blended colour with the lightness cap applied.
        for key in [(0, 0, 0), (256, 0, 256), (512, 0, 512)] {
            let color = data.vertex(key).and_then(|v| v.color).expect("colour");
            assert!(color & 0x7F <= 55, "{color}");
        }
    }

    #[test]
    fn water_sinks_the_terrain_by_distance_from_the_shore() {
        let ctx = context();
        let size = 9;
        // A 7x7 lake with a one-tile land border.
        let (surfaces, ids) = scene(size, |x, y| {
            if (1..8).contains(&x) && (1..8).contains(&y) {
                3
            } else {
                1
            }
        });
        let data = generate(&surfaces, &ids, size, &ctx);
        let depth = |x: i32, y: i32| data.underwater_depth((x * 128, 0, y * 128));
        // Shore vertices stay at the surface, depth grows towards the middle and is symmetric.
        assert_eq!(depth(1, 1), 0, "shore");
        assert_eq!(depth(0, 0), 0, "land");
        assert!(depth(2, 4) > depth(1, 4) || depth(1, 4) == 0);
        assert!(depth(4, 4) > depth(3, 4), "{} {}", depth(4, 4), depth(3, 4));
        assert_eq!(depth(3, 5), depth(5, 3), "symmetric");
        assert!(depth(4, 4) <= (DEPTH_LEVEL_SLOPE[11] as f32 * 0.55) as i32);
        // Water tiles are flagged, land tiles are not, and water vertices carry the flag.
        assert_ne!(data.tile_flags(0, 4, 4) & TILE_WATER, 0);
        assert_eq!(data.tile_flags(0, 0, 0) & TILE_WATER, 0);
        assert!(data.has((4 * 128, 0, 4 * 128), VERTEX_WATER));
        // The terrain normal under water follows the sunk surface, so it tilts towards the lake
        // centre: a vertex west of centre faces east-ish.
        let n = data.normal((3 * 128, 0, 4 * 128));
        assert!(n[0] > 0.0, "{n:?}");
    }

    #[test]
    fn relight_brightens_lit_slopes_and_caps_lightness() {
        // Facing the vanilla light (+x, +z): lighter than facing away.
        let colour = (8 << 10) | (3 << 7) | 30;
        let lit = relight(colour, [0.7, -0.1, 0.7]) & 0x7F;
        let shaded = relight(colour, [-0.7, -0.1, -0.7]) & 0x7F;
        assert!(lit > shaded, "{lit} vs {shaded}");
        // Hue and saturation are untouched and lightness never exceeds 55.
        assert_eq!(
            relight(colour | 0x7F, [0.7, 0.0, 0.7]) & !0x7F,
            colour & !0x7F
        );
        assert!(relight(127 | (1 << 10), [0.7, 0.0, 0.7]) & 0x7F <= 55);
        // Flat ground (no xz component) keeps the colour apart from the x1.25 re-expansion.
        assert_eq!(
            relight(colour, [0.0, -1.0, 0.0]) & 0x7F,
            (30.0_f32 * 1.25) as i32
        );
    }
}
