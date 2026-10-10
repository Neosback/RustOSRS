//! Packed scene geometry in the reference renderer's vertex layout.
//!
//! The layout mirrors the audited RuneLite static upload (`SceneUploader`): zone-local 16-bit
//! positions, one packed `alpha | bias | hsl` word per vertex, a texture id (`id + 1`, `0` for
//! untextured) and 8.8 fixed-point UVs. 20 bytes per vertex keeps zone buffers small.
//!
//! Terrain emission follows `SceneUploader.upload(SceneTilePaint/SceneTileModel)` and model
//! emission follows `uploadStaticModel`, including suppressed-face (`c == -2`) admission, flat
//! color promotion (`c == -1`), and the alpha/opaque split.

use bytemuck::{Pod, Zeroable};
use osrs_core::lighting::ReferenceLitModel;
use osrs_scene::terrain::{FlatTerrainSurface, ShapedTerrainSurface, TerrainSurface};

/// Terrain color sentinel meaning "do not draw" (`12345678`).
pub const TERRAIN_SKIP_COLOR: i32 = 12_345_678;

/// One vertex in the 20-byte reference layout.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct PackedVertex {
    /// Zone-local `x`, `y` (height; negative is up), `z`, and one pad value.
    pub position: [i16; 4],
    /// `alpha << 24 | bias << 16 | hsl` where `hsl` is the 16-bit packed color.
    pub abhsl: u32,
    /// Texture id plus one (`0` = untextured).
    pub texture: u16,
    /// UV in 8.8 fixed point (`256` = one texture repeat).
    pub uv: [i16; 2],
    pub pad: u16,
}

impl PackedVertex {
    pub const SIZE: usize = 20;

    fn new(position: [i32; 3], abhsl: u32, texture: u16, uv: [i32; 2]) -> Self {
        Self {
            position: [
                clamp_i16(position[0]),
                clamp_i16(position[1]),
                clamp_i16(position[2]),
                0,
            ],
            abhsl,
            texture,
            uv: [clamp_i16(uv[0]), clamp_i16(uv[1])],
            pad: 0,
        }
    }
}

fn clamp_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

/// Triangle lists for one draw group, split by transparency like the reference zone buffers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GeometryBuilder {
    pub opaque: Vec<PackedVertex>,
    pub alpha: Vec<PackedVertex>,
}

/// Placement of a model relative to the zone origin that owns its tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelPlacement {
    /// Position of the model origin in zone-local units.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Extra instance rotation in JAU (`256` for diagonal game objects), `0` otherwise.
    pub orientation: u16,
}

/// Stand-in color source for textured faces until texture sampling exists (M12).
pub trait TextureAverage {
    /// Packed 16-bit HSL average of the texture, if known.
    fn average_hsl(&self, texture_id: u32) -> Option<u16>;
}

impl TextureAverage for () {
    fn average_hsl(&self, _texture_id: u32) -> Option<u16> {
        None
    }
}

impl GeometryBuilder {
    pub fn is_empty(&self) -> bool {
        self.opaque.is_empty() && self.alpha.is_empty()
    }

    pub fn vertex_count(&self) -> usize {
        self.opaque.len() + self.alpha.len()
    }

    /// Emit one terrain tile (`SceneUploader.upload` for `SceneTilePaint` / `SceneTileModel`).
    ///
    /// `tile` is the scene tile; `zone_origin` is the scene-local origin (in 128-unit local
    /// units) of the zone that owns the tile. Output positions are zone-local.
    pub fn push_terrain(
        &mut self,
        surface: &TerrainSurface,
        tile: (i32, i32),
        zone_origin: (i32, i32),
        textures: &impl TextureAverage,
    ) {
        match surface {
            TerrainSurface::Flat(flat) => self.push_flat_terrain(flat, tile, zone_origin, textures),
            TerrainSurface::Shaped(shaped) => {
                self.push_shaped_terrain(shaped, tile, zone_origin, textures);
            }
        }
    }

    fn push_flat_terrain(
        &mut self,
        flat: &FlatTerrainSurface,
        tile: (i32, i32),
        zone_origin: (i32, i32),
        textures: &impl TextureAverage,
    ) {
        if flat.colors.northeast == TERRAIN_SKIP_COLOR {
            return;
        }
        let lx = tile.0 * 128 - zone_origin.0;
        let lz = tile.1 * 128 - zone_origin.1;
        let h = flat.heights;
        let c = flat.colors;
        let texture = flat.texture_id.map_or(0, |id| (id + 1) as u16);
        let corner = |x: i32, z: i32, height: i32, hsl: i32, u: i32, v: i32| {
            let hsl = terrain_color(hsl, flat.texture_id, textures);
            PackedVertex::new([x, height, z], hsl, texture, [u, v])
        };
        // Triangle order `NE NW SE / SW SE NW` with full-tile UVs, as in `SceneUploader`.
        let v_ne = corner(lx + 128, lz + 128, h.northeast, c.northeast, 256, 256);
        let v_nw = corner(lx, lz + 128, h.northwest, c.northwest, 0, 256);
        let v_se = corner(lx + 128, lz, h.southeast, c.southeast, 256, 0);
        let v_sw = corner(lx, lz, h.southwest, c.southwest, 0, 0);
        self.opaque
            .extend_from_slice(&[v_ne, v_nw, v_se, v_sw, v_se, v_nw]);
    }

    fn push_shaped_terrain(
        &mut self,
        shaped: &ShapedTerrainSurface,
        tile: (i32, i32),
        zone_origin: (i32, i32),
        textures: &impl TextureAverage,
    ) {
        let tile_x = tile.0 * 128;
        let tile_z = tile.1 * 128;
        for face in &shaped.faces {
            if face.colors[0] == TERRAIN_SKIP_COLOR {
                continue;
            }
            let texture = face.texture_id.map_or(0, |id| (id + 1) as u16);
            for (corner, &index) in face.indices.iter().enumerate() {
                let vertex = &shaped.vertices[index];
                let hsl = terrain_color(face.colors[corner], face.texture_id, textures);
                let x = vertex.position.x.units();
                let z = vertex.position.z.units();
                self.opaque.push(PackedVertex::new(
                    [
                        x - zone_origin.0,
                        vertex.position.y.units(),
                        z - zone_origin.1,
                    ],
                    hsl,
                    texture,
                    // 128 local units across the tile map to 256 UV units.
                    [(x - tile_x) * 2, (z - tile_z) * 2],
                ));
            }
        }
    }

    /// Emit one lit model (`SceneUploader.uploadStaticModel`).
    pub fn push_model(
        &mut self,
        model: &ReferenceLitModel,
        placement: ModelPlacement,
        textures: &impl TextureAverage,
    ) {
        let (sin, cos) = if placement.orientation == 0 {
            (0, 0)
        } else {
            let tables = osrs_core::trig::trig_tables();
            let index = usize::from(placement.orientation) & 2047;
            (tables.sine(index), tables.cosine(index))
        };
        let positions: Vec<[i32; 3]> = model
            .vertices
            .iter()
            .map(|vertex| {
                let (mut x, y, mut z) = (vertex.x, vertex.y, vertex.z);
                if placement.orientation != 0 {
                    let original_x = x;
                    x = (z
                        .wrapping_mul(sin)
                        .wrapping_add(original_x.wrapping_mul(cos)))
                        >> 16;
                    z = (z
                        .wrapping_mul(cos)
                        .wrapping_sub(original_x.wrapping_mul(sin)))
                        >> 16;
                }
                [x + placement.x, y + placement.y, z + placement.z]
            })
            .collect();

        for (face_index, face) in model.faces.iter().enumerate() {
            let colors = model.face_colors[face_index];
            let (color_a, color_b, color_c) = match colors.c {
                -2 => continue,
                -1 => (colors.a, colors.a, colors.a),
                _ => (colors.a, colors.b, colors.c),
            };
            let alpha = model
                .face_alphas
                .as_ref()
                .map_or(0, |alphas| alphas[face_index] as u8);
            let bias = model
                .face_biases
                .as_ref()
                .map_or(0, |biases| biases[face_index] as u8);
            let texture_id = model
                .face_textures
                .as_ref()
                .and_then(|textures| textures[face_index])
                .map(|texture| texture.get());
            let texture = texture_id.map_or(0, |id| (id + 1) as u16);
            let alpha_bias = (u32::from(alpha) << 24) | (u32::from(bias) << 16);

            let indices = [face.a.get(), face.b.get(), face.c.get()];
            let target = if alpha != 0 {
                &mut self.alpha
            } else {
                &mut self.opaque
            };
            for (corner, index) in indices.into_iter().enumerate() {
                let color = [color_a, color_b, color_c][corner];
                let hsl = match texture_id {
                    Some(id) => textured_standin(color, textures.average_hsl(id)),
                    None => (color & 0xFFFF) as u32,
                };
                // Reference UVs (M10 `prepare_reference_face_uvs`) are applied once textures are
                // sampled; the canonical triangle mapping is carried until then.
                let uv = [[0, 0], [256, 0], [0, 256]][corner];
                target.push(PackedVertex::new(
                    positions[index as usize],
                    alpha_bias | hsl,
                    texture,
                    uv,
                ));
            }
        }
    }
}

/// HSL for a terrain vertex. Textured terrain stores lightness only, so the stand-in swaps in the
/// texture's average hue/saturation while keeping the baked lightness.
fn terrain_color(color: i32, texture: Option<i32>, textures: &impl TextureAverage) -> u32 {
    match texture {
        Some(id) => textured_standin(color, textures.average_hsl(id as u32)),
        None => (color & 0xFFFF) as u32,
    }
}

/// Textured faces carry only a 7-bit lightness; until real texture sampling exists, color them
/// with the texture's average hue/saturation scaled by that lightness. Clearly a stand-in.
fn textured_standin(lightness: i32, average: Option<u16>) -> u32 {
    let light = (lightness & 127) as u32;
    match average {
        Some(average) => {
            let average = u32::from(average);
            let base_light = (average & 127).max(1);
            let scaled = (base_light * light / 64).clamp(2, 126);
            (average & 0xFF80) | scaled
        }
        // Unknown texture: neutral gray at the baked lightness.
        None => light.clamp(2, 126),
    }
}

/// Size of a renderer zone in scene-local units (8 tiles of 128).
pub const ZONE_LOCAL_UNITS: i32 = 1024;

/// Geometry of one `(level, minimum plane)` bucket inside a zone.
///
/// Splitting by level and minimum plane lets the frame choose visible content for the viewing
/// plane without rebuilding buffers (the reference draws level `<=` view plane and tiles whose
/// minimum plane is `<=` the view plane).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZoneGroup {
    pub level: u8,
    pub min_plane: u8,
    pub geometry: GeometryBuilder,
}

/// All geometry owned by one 8x8-tile zone (scene zone coordinates).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZoneGeometry {
    pub zone_x: i32,
    pub zone_z: i32,
    pub groups: Vec<ZoneGroup>,
}

impl ZoneGeometry {
    /// Scene-local origin of the zone in local units.
    pub const fn origin(&self) -> (i32, i32) {
        (
            self.zone_x * ZONE_LOCAL_UNITS,
            self.zone_z * ZONE_LOCAL_UNITS,
        )
    }

    pub fn group_mut(&mut self, level: u8, min_plane: u8) -> &mut GeometryBuilder {
        if let Some(index) = self
            .groups
            .iter()
            .position(|group| group.level == level && group.min_plane == min_plane)
        {
            return &mut self.groups[index].geometry;
        }
        self.groups.push(ZoneGroup {
            level,
            min_plane,
            geometry: GeometryBuilder::default(),
        });
        let last = self.groups.len() - 1;
        &mut self.groups[last].geometry
    }

    pub fn vertex_count(&self) -> usize {
        self.groups.iter().map(|g| g.geometry.vertex_count()).sum()
    }
}

/// Extracted static geometry for a whole scene window.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SceneGeometry {
    pub zones: Vec<ZoneGeometry>,
}

impl SceneGeometry {
    pub fn zone_mut(&mut self, zone_x: i32, zone_z: i32) -> &mut ZoneGeometry {
        if let Some(index) = self
            .zones
            .iter()
            .position(|zone| zone.zone_x == zone_x && zone.zone_z == zone_z)
        {
            return &mut self.zones[index];
        }
        self.zones.push(ZoneGeometry {
            zone_x,
            zone_z,
            groups: Vec::new(),
        });
        let last = self.zones.len() - 1;
        &mut self.zones[last]
    }

    pub fn vertex_count(&self) -> usize {
        self.zones.iter().map(ZoneGeometry::vertex_count).sum()
    }
}
