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
    /// One record per model that contributed transparent faces to `alpha`, in emission order.
    pub alpha_models: Vec<AlphaModel>,
}

/// Sorting data of one model's transparent faces (`Zone.AlphaModel`).
///
/// RuneLite draws alpha models far to near by model origin and, for models near the camera, orders
/// each model's faces back to front in camera depth bins. This keeps what that sort needs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AlphaModel {
    /// First vertex of the model inside [`GeometryBuilder::alpha`].
    pub first_vertex: u32,
    /// Number of vertices (3 per face).
    pub vertex_count: u32,
    /// Model origin in zone-local units (`AlphaModel.x/y/z`).
    pub origin: [i32; 3],
    /// Face bin radius (`AlphaModel.radius`).
    pub radius: i32,
    /// Per face, in emission order: the centroid relative to the model centre packed as
    /// `x:11 | y:10 | z:11` bits (`AlphaModel.packedFaces`).
    pub packed_faces: Vec<i32>,
}

impl AlphaModel {
    /// Unpack face `index` into `(x, y, z)` in the shifted model-centre space.
    pub fn face(&self, index: usize) -> (i32, i32, i32) {
        let pack = self.packed_faces[index];
        (pack >> 21, (pack << 11) >> 22, (pack << 21) >> 21)
    }
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
    ) {
        match surface {
            TerrainSurface::Flat(flat) => self.push_flat_terrain(flat, tile, zone_origin),
            TerrainSurface::Shaped(shaped) => {
                self.push_shaped_terrain(shaped, tile, zone_origin);
            }
        }
    }

    fn push_flat_terrain(
        &mut self,
        flat: &FlatTerrainSurface,
        tile: (i32, i32),
        zone_origin: (i32, i32),
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
            PackedVertex::new([x, height, z], (hsl & 0xFFFF) as u32, texture, [u, v])
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
                let hsl = (face.colors[corner] & 0xFFFF) as u32;
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
    pub fn push_model(&mut self, model: &ReferenceLitModel, placement: ModelPlacement) {
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

        let first_alpha_vertex = self.alpha.len();
        // Centroid sums (3x the centroid) of the transparent faces, in the model's own space.
        let mut alpha_face_sums: Vec<[i32; 3]> = Vec::new();
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
            let face_uvs = if texture_id.is_some() {
                face_uvs(model, face_index, indices)
            } else {
                [[0.0; 2]; 3]
            };
            let target = if alpha != 0 {
                let sum = |axis: usize| -> i32 {
                    indices
                        .iter()
                        .map(|&i| {
                            let v = &model.vertices[i as usize];
                            [v.x, v.y, v.z][axis]
                        })
                        .fold(0_i32, i32::wrapping_add)
                };
                alpha_face_sums.push([sum(0), sum(1), sum(2)]);
                &mut self.alpha
            } else {
                &mut self.opaque
            };
            for (corner, index) in indices.into_iter().enumerate() {
                let color = [color_a, color_b, color_c][corner];
                let hsl = (color & 0xFFFF) as u32;
                let uv = [
                    (face_uvs[corner][0] * 256.0) as i32,
                    (face_uvs[corner][1] * 256.0) as i32,
                ];
                target.push(PackedVertex::new(
                    positions[index as usize],
                    alpha_bias | hsl,
                    texture,
                    uv,
                ));
            }
        }
        if !alpha_face_sums.is_empty() {
            let (radius, packed_faces) = pack_alpha_faces(&alpha_face_sums);
            self.alpha_models.push(AlphaModel {
                first_vertex: first_alpha_vertex as u32,
                vertex_count: (self.alpha.len() - first_alpha_vertex) as u32,
                origin: [placement.x, placement.y, placement.z],
                radius,
                packed_faces,
            });
        }
    }
}

/// `Zone.addAlphaModel`'s face packing: centre the transparent face centroids, scale them into
/// signed 11/10/11-bit fields, and return `(radius, packed faces)`.
fn pack_alpha_faces(sums: &[[i32; 3]]) -> (i32, Vec<i32>) {
    let mut min = [i32::MAX; 3];
    let mut max = [i32::MIN; 3];
    for sum in sums {
        for axis in 0..3 {
            min[axis] = min[axis].min(sum[axis]);
            max[axis] = max[axis].max(sum[axis]);
        }
    }
    // Java integer division truncates toward zero, like Rust's `/`.
    let center = [
        (min[0] + max[0]) / 6,
        (min[1] + max[1]) / 6,
        (min[2] + max[2]) / 6,
    ];
    let size = (max[0] / 3 - center[0])
        .max(min[0] / -3 - center[0])
        .max((max[1] / 3 - center[1]).max(min[1] / -3 - center[1]) * 2)
        .max((max[2] / 3 - center[2]).max(min[2] / -3 - center[2]));
    let mut shift = 0;
    let mut v = size >> 10;
    while v > 0 {
        shift += 1;
        v >>= 1;
    }
    let mut radius = 0_i32;
    let packed = sums
        .iter()
        .map(|sum| {
            let f = [
                ((sum[0] / 3) - center[0]) >> shift,
                ((sum[1] / 3) - center[1]) >> shift,
                ((sum[2] / 3) - center[2]) >> shift,
            ];
            radius = radius.max(f[0] * f[0] + f[1] * f[1] + f[2] * f[2]);
            ((f[0] & 0x7ff) << 21) | ((f[1] & 0x3ff) << 11) | (f[2] & 0x7ff)
        })
        .collect();
    (2 + f64::from(radius).sqrt() as i32, packed)
}

/// Reference UVs of one textured face (`computeFaceUvs`, static path): the explicit texture
/// triangle basis when the face has a texture-face selector, otherwise the canonical
/// `(0,0) (1,0) (0,1)` mapping.
fn face_uvs(model: &ReferenceLitModel, face_index: usize, indices: [u32; 3]) -> [[f32; 2]; 3] {
    let point = |index: u32| {
        model
            .vertices
            .get(index as usize)
            .map_or([0.0; 3], |v| [v.x as f32, v.y as f32, v.z as f32])
    };
    let selector = model
        .texture_faces
        .as_ref()
        .and_then(|selectors| selectors.get(face_index).copied().flatten());
    match selector.and_then(|index| model.texture_triangles.get(index as usize)) {
        Some(texture) => crate::uv::reference_uvs_from_points(
            indices.map(point),
            [texture.a.get(), texture.b.get(), texture.c.get()].map(point),
        ),
        None => [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
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
