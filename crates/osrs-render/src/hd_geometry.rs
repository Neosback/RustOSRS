//! HD vertex stream: the per-vertex data the 117 HD scene shader reads next to the Reference
//! vertex (position and `alpha | bias | hsl`).
//!
//! The HD profile draws two vertex buffers per draw: the Reference [`PackedVertex`] (position,
//! alpha, depth bias) and one [`HdVertex`] with the rest. Like the plugin's zone renderer, every
//! triangle carries the colour, material and water data of all three corners on each of its
//! vertices (the corner is `vertex_index % 3`), so the fragment shader can blend per-corner
//! materials. Adapted from `SceneUploader` / `scene_vert.glsl` of the 117 HD plugin. Copyright (c)
//! 2018, Adam <Adam@sigterm.info>; (c) 2021, 117 and Hooder (BSD 2-Clause, see THIRD_PARTY.md).

use crate::geometry::{AlphaModel, GeometryBuilder, PackedVertex, pack_alpha_faces};
use bytemuck::{Pod, Zeroable};

/// Bits of an [`HdFace::material`] entry: table index (low 12 bits) and the overlay flag.
pub const HD_MATERIAL_INDEX_MASK: u16 = 0x0FFF;
pub const HD_MATERIAL_OVERLAY: u16 = 1 << 12;

/// [`HdFace::flags`]: the face wants straight-up normals instead of lighting normals.
pub const HD_FLAG_UPWARDS_NORMALS: u16 = 1;
/// The face is shaded flat (face normal).
pub const HD_FLAG_FLAT_NORMALS: u16 = 1 << 1;
/// Do not receive shadows.
pub const HD_FLAG_NO_SHADOW_RECEIVE: u16 = 1 << 2;
/// Do not cast shadows.
pub const HD_FLAG_NO_SHADOW_CAST: u16 = 1 << 5;
/// Offset the shadow lookup along the normal (`shadowNormalBias`).
pub const HD_FLAG_SHADOW_NORMAL_BIAS: u16 = 1 << 6;
/// The face is terrain (so `water_type`/`water_depth` are meaningful).
pub const HD_FLAG_TERRAIN: u16 = 1 << 3;
/// Textures use vanilla UVs (clamp horizontally for transparent textures).
pub const HD_FLAG_VANILLA_UVS: u16 = 1 << 4;

/// One HD vertex (36 bytes), parallel to a [`PackedVertex`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct HdVertex {
    /// Octahedral-encoded unit normal, snorm16 (see [`oct_encode`]).
    pub normal: [i16; 2],
    /// Texture coordinates in texture repeats.
    pub uv: [f32; 2],
    /// Packed Jagex HSL of the triangle's three corners, then the face flags.
    pub hsl_flags: [u16; 4],
    /// Material of the three corners (index | overlay flag), then water type | plane << 8.
    pub material_water: [u16; 4],
    /// Water depth of the three corners (12 bits), then padding.
    pub depth: [u16; 4],
}

impl Eq for HdVertex {}

impl HdVertex {
    pub const SIZE: usize = 36;
}

const _: () = assert!(std::mem::size_of::<HdVertex>() == HdVertex::SIZE);

/// Octahedral encoding of a unit vector into two snorm16 values.
pub fn oct_encode(normal: [f32; 3]) -> [i16; 2] {
    let sum = normal[0].abs() + normal[1].abs() + normal[2].abs();
    if sum <= f32::EPSILON {
        // Degenerate: encode "up" (negative y).
        return oct_encode([0.0, -1.0, 0.0]);
    }
    let (mut x, mut y) = (normal[0] / sum, normal[1] / sum);
    if normal[2] < 0.0 {
        let (ox, oy) = (x, y);
        x = (1.0 - oy.abs()) * sign(ox);
        y = (1.0 - ox.abs()) * sign(oy);
    }
    [snorm(x), snorm(y)]
}

/// Inverse of [`oct_encode`] (the WGSL decode mirrors this).
pub fn oct_decode(encoded: [i16; 2]) -> [f32; 3] {
    let (x, y) = (unsnorm(encoded[0]), unsnorm(encoded[1]));
    let z = 1.0 - x.abs() - y.abs();
    let (mut nx, mut ny) = (x, y);
    if z < 0.0 {
        nx = (1.0 - y.abs()) * sign(x);
        ny = (1.0 - x.abs()) * sign(y);
    }
    let length = (nx * nx + ny * ny + z * z).sqrt();
    [nx / length, ny / length, z / length]
}

fn sign(value: f32) -> f32 {
    if value >= 0.0 { 1.0 } else { -1.0 }
}

fn snorm(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

fn unsnorm(value: i16) -> f32 {
    (f32::from(value) / 32767.0).max(-1.0)
}

/// One triangle for the HD stream, positions already zone-local and in front-face winding order.
#[derive(Debug, Clone, PartialEq)]
pub struct HdFace {
    pub positions: [[i32; 3]; 3],
    pub uvs: [[f32; 2]; 3],
    /// Unit normals per corner.
    pub normals: [[f32; 3]; 3],
    /// Packed Jagex HSL per corner.
    pub hsl: [u16; 3],
    /// Per corner material table index, with [`HD_MATERIAL_OVERLAY`].
    pub material: [u16; 3],
    /// Per corner water depth (0 = none).
    pub water_depth: [u16; 3],
    pub water_type: u8,
    pub plane: u8,
    pub flags: u16,
    /// Transparency, `0` = opaque (the Reference `alpha` byte).
    pub alpha: u8,
    /// Draw in the alpha pass even when `alpha` is `0`.
    pub alpha_pass: bool,
    /// The face uses a cutout material (leaves, fences): it stays opaque but is drawn with
    /// alpha-to-coverage, after the solid faces of its group.
    pub cutout: bool,
    /// Authored depth bias.
    pub bias: u8,
    /// Also emit the back side (reversed winding and normals).
    pub double_sided: bool,
}

impl HdFace {
    /// The same triangle seen from behind: corner order and per-corner data reversed, normals
    /// flipped.
    fn reversed(&self) -> Self {
        let flip = |normal: [f32; 3]| [-normal[0], -normal[1], -normal[2]];
        Self {
            positions: [self.positions[2], self.positions[1], self.positions[0]],
            uvs: [self.uvs[2], self.uvs[1], self.uvs[0]],
            normals: [
                flip(self.normals[2]),
                flip(self.normals[1]),
                flip(self.normals[0]),
            ],
            hsl: [self.hsl[2], self.hsl[1], self.hsl[0]],
            material: [self.material[2], self.material[1], self.material[0]],
            water_depth: [
                self.water_depth[2],
                self.water_depth[1],
                self.water_depth[0],
            ],
            double_sided: false,
            ..self.clone()
        }
    }

    fn vertices(&self) -> [(PackedVertex, HdVertex); 3] {
        let alpha_bias = (u32::from(self.alpha) << 24) | (u32::from(self.bias) << 16);
        let water = u16::from(self.water_type) | (u16::from(self.plane & 3) << 8);
        std::array::from_fn(|corner| {
            let [x, y, z] = self.positions[corner];
            let packed = PackedVertex::new(
                [x, y, z],
                alpha_bias | u32::from(self.hsl[corner]),
                0,
                [0, 0],
            );
            let hd = HdVertex {
                normal: oct_encode(self.normals[corner]),
                uv: self.uvs[corner],
                hsl_flags: [self.hsl[0], self.hsl[1], self.hsl[2], self.flags],
                material_water: [self.material[0], self.material[1], self.material[2], water],
                depth: [
                    self.water_depth[0] & 0x0FFF,
                    self.water_depth[1] & 0x0FFF,
                    self.water_depth[2] & 0x0FFF,
                    0,
                ],
            };
            (packed, hd)
        })
    }
}

impl GeometryBuilder {
    /// Place the cutout faces collected by [`Self::push_hd_faces`] after the solid ones. Call once
    /// when the group is complete.
    pub fn finish_hd(&mut self) {
        self.hd_cutout_opaque = self.hd_pending_cutout.len() as u32;
        for (packed, hd) in std::mem::take(&mut self.hd_pending_cutout) {
            self.opaque.push(packed);
            self.hd_opaque.push(hd);
        }
    }

    /// Emit faces of one model (or one terrain tile) into the HD streams. `origin` is the model
    /// origin in zone-local units, used to sort the transparent faces like the Reference path.
    ///
    /// Faces with `alpha != 0` go to the alpha streams and are recorded as one [`AlphaModel`];
    /// double-sided faces emit their back side right after the front.
    pub fn push_hd_faces(&mut self, faces: &[HdFace], origin: [i32; 3]) {
        let first_alpha_vertex = self.alpha.len();
        let mut alpha_face_sums: Vec<[i32; 3]> = Vec::new();
        for face in faces {
            let mut emit = |face: &HdFace, builder: &mut Self| {
                let alpha = face.alpha != 0 || face.alpha_pass;
                if alpha {
                    let sum = |axis: usize| {
                        face.positions
                            .iter()
                            .map(|p| p[axis])
                            .fold(0_i32, i32::wrapping_add)
                    };
                    alpha_face_sums.push([sum(0), sum(1), sum(2)]);
                }
                for (packed, hd) in face.vertices() {
                    if !alpha && face.cutout {
                        builder.hd_pending_cutout.push((packed, hd));
                    } else if alpha {
                        builder.alpha.push(packed);
                        builder.hd_alpha.push(hd);
                    } else {
                        builder.opaque.push(packed);
                        builder.hd_opaque.push(hd);
                    }
                }
            };
            emit(face, self);
            if face.double_sided {
                emit(&face.reversed(), self);
            }
        }
        if !alpha_face_sums.is_empty() {
            let (radius, packed_faces) = pack_alpha_faces(&alpha_face_sums);
            self.alpha_models.push(AlphaModel {
                first_vertex: first_alpha_vertex as u32,
                vertex_count: (self.alpha.len() - first_alpha_vertex) as u32,
                origin,
                radius,
                packed_faces,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(alpha: u8, double_sided: bool) -> HdFace {
        HdFace {
            positions: [[0, 0, 0], [128, 0, 0], [0, 0, 128]],
            uvs: [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            normals: [[0.0, -1.0, 0.0]; 3],
            hsl: [10, 20, 30],
            material: [1, 2 | HD_MATERIAL_OVERLAY, 3],
            water_depth: [0, 5, 0],
            water_type: 4,
            plane: 2,
            flags: HD_FLAG_TERRAIN,
            alpha,
            alpha_pass: false,
            cutout: false,
            bias: 7,
            double_sided,
        }
    }

    #[test]
    fn octahedral_normals_round_trip() {
        for normal in [
            [0.0_f32, -1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, -1.0],
            [0.3, -0.8, 0.2],
            [-0.6, 0.1, -0.5],
        ] {
            let length =
                (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
            let unit = normal.map(|c| c / length);
            let decoded = oct_decode(oct_encode(unit));
            let dot: f32 = unit.iter().zip(decoded).map(|(a, b)| a * b).sum();
            assert!(dot > 0.9999, "{unit:?} -> {decoded:?} (dot {dot})");
        }
        assert_eq!(
            oct_decode(oct_encode([0.0; 3])),
            oct_decode(oct_encode([0.0, -1.0, 0.0]))
        );
    }

    #[test]
    fn corners_carry_all_three_materials_and_streams_stay_parallel() {
        let mut builder = GeometryBuilder::default();
        builder.push_hd_faces(&[face(0, true)], [0, 0, 0]);
        // Front and back.
        assert_eq!(builder.opaque.len(), 6);
        assert_eq!(builder.opaque.len(), builder.hd_opaque.len());
        let front = &builder.hd_opaque[0];
        assert_eq!(front.material_water[..3], [1, 2 | HD_MATERIAL_OVERLAY, 3]);
        assert_eq!(front.material_water[3], 4 | (2 << 8));
        assert_eq!(front.depth[..3], [0, 5, 0]);
        // The back side lists the corners in reverse, so corner `i` still addresses its own data.
        let back = &builder.hd_opaque[3];
        assert_eq!(back.material_water[..3], [3, 2 | HD_MATERIAL_OVERLAY, 1]);
        assert_eq!(builder.opaque[3].position[..3], [0, 0, 128]);
        assert_eq!(oct_decode(back.normal)[1].round(), 1.0, "normal flipped");
    }

    #[test]
    fn transparent_faces_become_one_sorted_model() {
        let mut builder = GeometryBuilder::default();
        builder.push_hd_faces(
            &[face(0, false), face(128, false), face(128, true)],
            [64, 0, 64],
        );
        assert_eq!(builder.opaque.len(), 3);
        assert_eq!(builder.alpha.len(), 9);
        assert_eq!(builder.hd_alpha.len(), 9);
        assert_eq!(builder.alpha_models.len(), 1);
        let model = &builder.alpha_models[0];
        assert_eq!(model.vertex_count, 9);
        assert_eq!(model.packed_faces.len(), 3);
        // Alpha and bias live on the Reference vertex.
        assert_eq!(builder.alpha[0].abhsl >> 24, 128);
        assert_eq!((builder.alpha[0].abhsl >> 16) & 0xFF, 7);
    }
}
