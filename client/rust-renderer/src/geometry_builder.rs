use std::collections::{HashMap, HashSet};

use crate::packed_vertex::{PackedVertexDeduper, VertexInput};

const INTEGER_FIELD_STRIDE: usize = 7;
const UV_FIELD_STRIDE: usize = 2;
const MODEL_FACE_FIELD_STRIDE: usize = 5;
const FLAG_REUSE_VERTEX: u8 = 1;
const FLAG_PRIORITY_IS_PACKED: u8 = 2;

/// Batched packed-vertex builder for the Stage 5 geometry-preparation migration.
///
/// The typed-array contract is intentionally flat so JavaScript can hand large
/// model batches to WASM without one call per vertex:
///
/// integer_fields, stride 7:
/// [x, y, z, hsl, alpha, texture_id, priority]
///
/// uv_fields, stride 2:
/// [u, v]
///
/// flags, stride 1:
/// bit 0 = reuse/deduplicate vertex
/// bit 1 = priority is already packed to 0..7
#[derive(Default)]
pub struct VertexBatchBuilder {
    deduper: PackedVertexDeduper,
    texture_id_map: HashMap<i32, i32>,
    used_texture_ids: HashSet<i32>,
}

impl VertexBatchBuilder {
    pub fn clear(&mut self) {
        self.deduper = PackedVertexDeduper::default();
        self.used_texture_ids.clear();
    }

    pub fn set_texture_id_map(
        &mut self,
        texture_ids: &[i32],
        texture_indices: &[i32],
    ) -> Result<(), String> {
        if texture_ids.len() != texture_indices.len() {
            return Err(format!(
                "texture ID map has {} ids but {} indices",
                texture_ids.len(),
                texture_indices.len()
            ));
        }

        self.texture_id_map.clear();
        self.texture_id_map.reserve(texture_ids.len());
        for (&texture_id, &texture_index) in texture_ids.iter().zip(texture_indices) {
            if texture_id < 0 {
                return Err(format!(
                    "texture ID map contains negative texture id {texture_id}"
                ));
            }
            if texture_index < 0 {
                return Err(format!(
                    "texture ID map contains negative texture index {texture_index}"
                ));
            }
            self.texture_id_map.insert(texture_id, texture_index);
        }
        self.used_texture_ids.clear();
        Ok(())
    }

    pub fn used_texture_ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self.used_texture_ids.iter().copied().collect();
        ids.sort_unstable();
        ids
    }

    fn map_texture_id(&mut self, texture_id: i32) -> i32 {
        if texture_id < 0 {
            return -1;
        }
        match self.texture_id_map.get(&texture_id).copied() {
            Some(texture_index) => {
                self.used_texture_ids.insert(texture_id);
                texture_index
            }
            None => -1,
        }
    }

    pub fn vertex_count(&self) -> usize {
        self.deduper.vertices().len()
    }

    pub fn push_batch(
        &mut self,
        integer_fields: &[i32],
        uv_fields: &[f32],
        flags: &[u8],
    ) -> Result<Vec<u32>, String> {
        if !integer_fields.len().is_multiple_of(INTEGER_FIELD_STRIDE) {
            return Err(format!(
                "vertex integer field packet must contain stride-{INTEGER_FIELD_STRIDE} records"
            ));
        }

        let vertex_count = integer_fields.len() / INTEGER_FIELD_STRIDE;
        let expected_uv_fields = vertex_count
            .checked_mul(UV_FIELD_STRIDE)
            .ok_or_else(|| "vertex UV field count overflow".to_string())?;

        if uv_fields.len() != expected_uv_fields {
            return Err(format!(
                "vertex UV field packet has {} values, expected {expected_uv_fields}",
                uv_fields.len()
            ));
        }

        if flags.len() != vertex_count {
            return Err(format!(
                "vertex flag packet has {} values, expected {vertex_count}",
                flags.len()
            ));
        }

        let mut indices = Vec::with_capacity(vertex_count);

        for (index, &record_flags) in flags.iter().enumerate() {
            let integer_offset = index * INTEGER_FIELD_STRIDE;
            let uv_offset = index * UV_FIELD_STRIDE;

            if record_flags & !(FLAG_REUSE_VERTEX | FLAG_PRIORITY_IS_PACKED) != 0 {
                return Err(format!(
                    "vertex flags at record {index} contain unsupported bits 0x{record_flags:02x}"
                ));
            }

            let input = VertexInput {
                x: integer_fields[integer_offset],
                y: integer_fields[integer_offset + 1],
                z: integer_fields[integer_offset + 2],
                hsl: integer_fields[integer_offset + 3],
                alpha: integer_fields[integer_offset + 4],
                texture_id: integer_fields[integer_offset + 5],
                priority: integer_fields[integer_offset + 6],
                priority_is_packed: (record_flags & FLAG_PRIORITY_IS_PACKED) != 0,
                u: uv_fields[uv_offset],
                v: uv_fields[uv_offset + 1],
            };

            indices.push(
                self.deduper
                    .push(input, (record_flags & FLAG_REUSE_VERTEX) != 0),
            );
        }

        Ok(indices)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn push_terrain_tile(
        &mut self,
        vertices_x: &[i32],
        vertices_y: &[i32],
        vertices_z: &[i32],
        faces_a: &[i32],
        faces_b: &[i32],
        faces_c: &[i32],
        colors_a: &[i32],
        colors_b: &[i32],
        colors_c: &[i32],
        texture_ids: &[i32],
        tile_x: i32,
        tile_z: i32,
        offset_x: i32,
        offset_z: i32,
    ) -> Result<Vec<u32>, String> {
        const INVALID_HSL_COLOR: i32 = 12_345_678;
        const TILE_SIZE: f32 = 128.0;

        if vertices_y.len() != vertices_x.len() || vertices_z.len() != vertices_x.len() {
            return Err("terrain vertex coordinate arrays must have equal lengths".to_string());
        }

        let face_count = faces_a.len();
        for (label, len) in [
            ("facesB", faces_b.len()),
            ("facesC", faces_c.len()),
            ("faceColorsA", colors_a.len()),
            ("faceColorsB", colors_b.len()),
            ("faceColorsC", colors_c.len()),
        ] {
            if len != face_count {
                return Err(format!(
                    "terrain {label} has {len} entries, expected {face_count}"
                ));
            }
        }
        if !texture_ids.is_empty() && texture_ids.len() != face_count {
            return Err(format!(
                "terrain textureIds has {} entries, expected {face_count}",
                texture_ids.len()
            ));
        }

        let mut indices = Vec::with_capacity(face_count.saturating_mul(3));

        for face_index in 0..face_count {
            let source_texture_id = texture_ids.get(face_index).copied().unwrap_or(-1);
            if colors_a[face_index] == INVALID_HSL_COLOR && source_texture_id == -1 {
                continue;
            }

            let corners = [
                faces_a[face_index],
                faces_b[face_index],
                faces_c[face_index],
            ];
            let hsls = [
                colors_a[face_index],
                colors_b[face_index],
                colors_c[face_index],
            ];
            let texture_index = self.map_texture_id(source_texture_id);

            for corner in 0..3 {
                let vertex_index = usize::try_from(corners[corner]).map_err(|_| {
                    format!(
                        "terrain face {face_index} corner {corner} has negative vertex index {}",
                        corners[corner]
                    )
                })?;
                if vertex_index >= vertices_x.len() {
                    return Err(format!(
                        "terrain face {face_index} corner {corner} references vertex {vertex_index}, but tile has {} vertices",
                        vertices_x.len()
                    ));
                }

                let u = (vertices_x[vertex_index] - tile_x) as f32 / TILE_SIZE;
                let v = (vertices_z[vertex_index] - tile_z) as f32 / TILE_SIZE;
                let input = VertexInput {
                    x: vertices_x[vertex_index].wrapping_add(offset_x),
                    y: vertices_y[vertex_index],
                    z: vertices_z[vertex_index].wrapping_add(offset_z),
                    hsl: hsls[corner],
                    alpha: 0xff,
                    texture_id: texture_index,
                    priority: 0,
                    priority_is_packed: false,
                    u,
                    v,
                };
                indices.push(self.deduper.push(input, true));
            }
        }

        Ok(indices)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn push_terrain_batch(
        &mut self,
        tile_vertex_offsets: &[u32],
        tile_face_offsets: &[u32],
        vertices_x: &[i32],
        vertices_y: &[i32],
        vertices_z: &[i32],
        faces_a: &[i32],
        faces_b: &[i32],
        faces_c: &[i32],
        colors_a: &[i32],
        colors_b: &[i32],
        colors_c: &[i32],
        texture_ids: &[i32],
        tile_x: &[i32],
        tile_z: &[i32],
        offset_x: i32,
        offset_z: i32,
    ) -> Result<Vec<u32>, String> {
        let tile_count = tile_x.len();
        if tile_z.len() != tile_count {
            return Err(format!(
                "terrain batch tileZ has {} entries, expected {tile_count}",
                tile_z.len()
            ));
        }
        if tile_vertex_offsets.len() != tile_count + 1 {
            return Err(format!(
                "terrain batch vertex offsets has {} entries, expected {}",
                tile_vertex_offsets.len(),
                tile_count + 1
            ));
        }
        if tile_face_offsets.len() != tile_count + 1 {
            return Err(format!(
                "terrain batch face offsets has {} entries, expected {}",
                tile_face_offsets.len(),
                tile_count + 1
            ));
        }
        if vertices_y.len() != vertices_x.len() || vertices_z.len() != vertices_x.len() {
            return Err("terrain batch vertex coordinate arrays must have equal lengths".to_string());
        }

        let face_count = faces_a.len();
        for (label, len) in [
            ("facesB", faces_b.len()),
            ("facesC", faces_c.len()),
            ("faceColorsA", colors_a.len()),
            ("faceColorsB", colors_b.len()),
            ("faceColorsC", colors_c.len()),
        ] {
            if len != face_count {
                return Err(format!(
                    "terrain batch {label} has {len} entries, expected {face_count}"
                ));
            }
        }
        if !texture_ids.is_empty() && texture_ids.len() != face_count {
            return Err(format!(
                "terrain batch textureIds has {} entries, expected {face_count}",
                texture_ids.len()
            ));
        }

        if tile_vertex_offsets.last().copied().unwrap_or(0) as usize != vertices_x.len() {
            return Err(format!(
                "terrain batch final vertex offset {} does not match {} vertices",
                tile_vertex_offsets.last().copied().unwrap_or(0),
                vertices_x.len()
            ));
        }
        if tile_face_offsets.last().copied().unwrap_or(0) as usize != face_count {
            return Err(format!(
                "terrain batch final face offset {} does not match {face_count} faces",
                tile_face_offsets.last().copied().unwrap_or(0)
            ));
        }

        let mut indices = Vec::with_capacity(face_count.saturating_mul(3));
        for tile_index in 0..tile_count {
            let vertex_start = tile_vertex_offsets[tile_index] as usize;
            let vertex_end = tile_vertex_offsets[tile_index + 1] as usize;
            let face_start = tile_face_offsets[tile_index] as usize;
            let face_end = tile_face_offsets[tile_index + 1] as usize;

            if vertex_start > vertex_end || vertex_end > vertices_x.len() {
                return Err(format!(
                    "terrain batch tile {tile_index} has invalid vertex range {vertex_start}..{vertex_end}"
                ));
            }
            if face_start > face_end || face_end > face_count {
                return Err(format!(
                    "terrain batch tile {tile_index} has invalid face range {face_start}..{face_end}"
                ));
            }

            let tile_texture_ids = if texture_ids.is_empty() {
                &[][..]
            } else {
                &texture_ids[face_start..face_end]
            };

            let mut tile_indices = self.push_terrain_tile(
                &vertices_x[vertex_start..vertex_end],
                &vertices_y[vertex_start..vertex_end],
                &vertices_z[vertex_start..vertex_end],
                &faces_a[face_start..face_end],
                &faces_b[face_start..face_end],
                &faces_c[face_start..face_end],
                &colors_a[face_start..face_end],
                &colors_b[face_start..face_end],
                &colors_c[face_start..face_end],
                tile_texture_ids,
                tile_x[tile_index],
                tile_z[tile_index],
                offset_x,
                offset_z,
            )?;
            indices.append(&mut tile_indices);
        }

        Ok(indices)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn push_model_faces(
        &mut self,
        vertices_x: &[i32],
        vertices_y: &[i32],
        vertices_z: &[i32],
        faces_a: &[i32],
        faces_b: &[i32],
        faces_c: &[i32],
        colors_a: &[i32],
        colors_b: &[i32],
        colors_c: &[i32],
        uvs: &[f32],
        face_fields: &[i32],
        scene_x: i32,
        scene_height: i32,
        scene_z: i32,
        override_hue: i32,
        override_saturation: i32,
        override_luminance: i32,
        override_amount: i32,
        reuse_vertices: bool,
    ) -> Result<Vec<u32>, String> {
        if vertices_y.len() != vertices_x.len() || vertices_z.len() != vertices_x.len() {
            return Err("model vertex coordinate arrays must have equal lengths".to_string());
        }

        let face_count = faces_a.len();
        for (label, len) in [
            ("indices2", faces_b.len()),
            ("indices3", faces_c.len()),
            ("faceColors1", colors_a.len()),
            ("faceColors2", colors_b.len()),
            ("faceColors3", colors_c.len()),
        ] {
            if len != face_count {
                return Err(format!(
                    "model {label} has {len} entries, expected {face_count}"
                ));
            }
        }

        if !face_fields.len().is_multiple_of(MODEL_FACE_FIELD_STRIDE) {
            return Err(format!(
                "model face packet must contain stride-{MODEL_FACE_FIELD_STRIDE} records"
            ));
        }

        if !uvs.is_empty() {
            let expected_uvs = face_count
                .checked_mul(6)
                .ok_or_else(|| "model UV field count overflow".to_string())?;
            if uvs.len() != expected_uvs {
                return Err(format!(
                    "model UV packet has {} values, expected {expected_uvs}",
                    uvs.len()
                ));
            }
        }

        let selected_face_count = face_fields.len() / MODEL_FACE_FIELD_STRIDE;
        let mut indices = Vec::with_capacity(selected_face_count.saturating_mul(3));

        for (packet_index, face) in face_fields
            .as_chunks::<MODEL_FACE_FIELD_STRIDE>()
            .0
            .iter()
            .enumerate()
        {
            let face_index = usize::try_from(face[0]).map_err(|_| {
                format!(
                    "model face packet record {packet_index} has negative face index {}",
                    face[0]
                )
            })?;
            if face_index >= face_count {
                return Err(format!(
                    "model face packet record {packet_index} references face {face_index}, but model has {face_count} faces"
                ));
            }

            let alpha = face[1];
            let priority = face[2];
            let render_layer = face[3];
            let texture_index = self.map_texture_id(face[4]);

            let mut hsl_a = colors_a[face_index];
            let mut hsl_b = colors_b[face_index];
            let mut hsl_c = colors_c[face_index];
            if hsl_c == -1 {
                hsl_b = hsl_a;
                hsl_c = hsl_a;
            }

            if override_amount != 0 {
                hsl_a = apply_color_override(
                    hsl_a,
                    override_hue,
                    override_saturation,
                    override_luminance,
                    override_amount,
                );
                hsl_b = apply_color_override(
                    hsl_b,
                    override_hue,
                    override_saturation,
                    override_luminance,
                    override_amount,
                );
                hsl_c = apply_color_override(
                    hsl_c,
                    override_hue,
                    override_saturation,
                    override_luminance,
                    override_amount,
                );
            }

            let vertex_indices = [
                faces_a[face_index],
                faces_b[face_index],
                faces_c[face_index],
            ];
            let hsls = [hsl_a, hsl_b, hsl_c];
            let uv_base = face_index
                .checked_mul(6)
                .ok_or_else(|| "model UV offset overflow".to_string())?;
            let packed_priority = if render_layer >= 0 {
                render_layer
            } else {
                priority
            };
            let priority_is_packed = render_layer >= 0;

            for corner in 0..3 {
                let vertex_index = usize::try_from(vertex_indices[corner]).map_err(|_| {
                    format!(
                        "model face {face_index} corner {corner} has negative vertex index {}",
                        vertex_indices[corner]
                    )
                })?;
                if vertex_index >= vertices_x.len() {
                    return Err(format!(
                        "model face {face_index} corner {corner} references vertex {vertex_index}, but model has {} vertices",
                        vertices_x.len()
                    ));
                }

                let (u, v) = if uvs.is_empty() {
                    (0.0, 0.0)
                } else {
                    let corner_uv = uv_base + corner * 2;
                    (uvs[corner_uv], uvs[corner_uv + 1])
                };

                let input = VertexInput {
                    x: scene_x.wrapping_add(vertices_x[vertex_index]),
                    y: scene_height.wrapping_add(vertices_y[vertex_index]),
                    z: scene_z.wrapping_add(vertices_z[vertex_index]),
                    hsl: hsls[corner],
                    alpha,
                    texture_id: texture_index,
                    priority: packed_priority,
                    priority_is_packed,
                    u,
                    v,
                };

                indices.push(self.deduper.push(input, reuse_vertices));
            }
        }

        Ok(indices)
    }

    pub fn packed_vertices(&self) -> Vec<u32> {
        let mut flat = Vec::with_capacity(self.vertex_count() * 3);
        for vertex in self.deduper.vertices() {
            flat.extend_from_slice(&[vertex.v0, vertex.v1, vertex.v2]);
        }
        flat
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::VertexBatchBuilder;
    use wasm_bindgen::prelude::*;

    /// WASM-facing batched vertex packer. It is deliberately separate from
    /// RustWebGlRenderer so geometry preparation can move into Rust without
    /// coupling CPU construction to a particular WebGL context.
    #[wasm_bindgen]
    pub struct RustVertexBufferBuilder {
        inner: VertexBatchBuilder,
    }

    #[wasm_bindgen]
    impl RustVertexBufferBuilder {
        #[wasm_bindgen(constructor)]
        pub fn new() -> Self {
            Self {
                inner: VertexBatchBuilder::default(),
            }
        }

        pub fn clear(&mut self) {
            self.inner.clear();
        }

        pub fn vertex_count(&self) -> u32 {
            self.inner.vertex_count() as u32
        }

        pub fn push_batch(
            &mut self,
            integer_fields: &[i32],
            uv_fields: &[f32],
            flags: &[u8],
        ) -> Result<Vec<u32>, JsValue> {
            self.inner
                .push_batch(integer_fields, uv_fields, flags)
                .map_err(|error| JsValue::from_str(&error))
        }

        pub fn set_texture_id_map(
            &mut self,
            texture_ids: &[i32],
            texture_indices: &[i32],
        ) -> Result<(), JsValue> {
            self.inner
                .set_texture_id_map(texture_ids, texture_indices)
                .map_err(|error| JsValue::from_str(&error))
        }

        pub fn used_texture_ids(&self) -> Vec<i32> {
            self.inner.used_texture_ids()
        }

        #[allow(clippy::too_many_arguments)]
        pub fn push_terrain_tile(
            &mut self,
            vertices_x: &[i32],
            vertices_y: &[i32],
            vertices_z: &[i32],
            faces_a: &[i32],
            faces_b: &[i32],
            faces_c: &[i32],
            colors_a: &[i32],
            colors_b: &[i32],
            colors_c: &[i32],
            texture_ids: &[i32],
            tile_x: i32,
            tile_z: i32,
            offset_x: i32,
            offset_z: i32,
        ) -> Result<Vec<u32>, JsValue> {
            self.inner
                .push_terrain_tile(
                    vertices_x,
                    vertices_y,
                    vertices_z,
                    faces_a,
                    faces_b,
                    faces_c,
                    colors_a,
                    colors_b,
                    colors_c,
                    texture_ids,
                    tile_x,
                    tile_z,
                    offset_x,
                    offset_z,
                )
                .map_err(|error| JsValue::from_str(&error))
        }

        #[allow(clippy::too_many_arguments)]
        pub fn push_terrain_batch(
            &mut self,
            tile_vertex_offsets: &[u32],
            tile_face_offsets: &[u32],
            vertices_x: &[i32],
            vertices_y: &[i32],
            vertices_z: &[i32],
            faces_a: &[i32],
            faces_b: &[i32],
            faces_c: &[i32],
            colors_a: &[i32],
            colors_b: &[i32],
            colors_c: &[i32],
            texture_ids: &[i32],
            tile_x: &[i32],
            tile_z: &[i32],
            offset_x: i32,
            offset_z: i32,
        ) -> Result<Vec<u32>, JsValue> {
            self.inner
                .push_terrain_batch(
                    tile_vertex_offsets,
                    tile_face_offsets,
                    vertices_x,
                    vertices_y,
                    vertices_z,
                    faces_a,
                    faces_b,
                    faces_c,
                    colors_a,
                    colors_b,
                    colors_c,
                    texture_ids,
                    tile_x,
                    tile_z,
                    offset_x,
                    offset_z,
                )
                .map_err(|error| JsValue::from_str(&error))
        }

        #[allow(clippy::too_many_arguments)]
        pub fn push_model_faces(
            &mut self,
            vertices_x: &[i32],
            vertices_y: &[i32],
            vertices_z: &[i32],
            faces_a: &[i32],
            faces_b: &[i32],
            faces_c: &[i32],
            colors_a: &[i32],
            colors_b: &[i32],
            colors_c: &[i32],
            uvs: &[f32],
            face_fields: &[i32],
            scene_x: i32,
            scene_height: i32,
            scene_z: i32,
            override_hue: i32,
            override_saturation: i32,
            override_luminance: i32,
            override_amount: i32,
            reuse_vertices: bool,
        ) -> Result<Vec<u32>, JsValue> {
            self.inner
                .push_model_faces(
                    vertices_x,
                    vertices_y,
                    vertices_z,
                    faces_a,
                    faces_b,
                    faces_c,
                    colors_a,
                    colors_b,
                    colors_c,
                    uvs,
                    face_fields,
                    scene_x,
                    scene_height,
                    scene_z,
                    override_hue,
                    override_saturation,
                    override_luminance,
                    override_amount,
                    reuse_vertices,
                )
                .map_err(|error| JsValue::from_str(&error))
        }

        pub fn packed_vertices(&self) -> Vec<u32> {
            self.inner.packed_vertices()
        }
    }

    impl Default for RustVertexBufferBuilder {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::RustVertexBufferBuilder;

fn apply_color_override(
    original_hsl: i32,
    override_hue: i32,
    override_saturation: i32,
    override_luminance: i32,
    override_amount: i32,
) -> i32 {
    if override_amount == 0 {
        return original_hsl;
    }

    let orig_hue = (original_hsl >> 10) & 0x3f;
    let orig_sat = (original_hsl >> 7) & 0x7;
    let orig_lum = original_hsl & 0x7f;

    let over_hue = override_hue & 0x7f;
    let over_sat = override_saturation & 0x7f;
    let over_lum = override_luminance & 0x7f;
    let blend_factor = f64::from(override_amount & 0xff) / 255.0;

    let over_hue_6bit = f64::from(over_hue) * 63.0 / 127.0;
    let over_sat_3bit = f64::from(over_sat) * 7.0 / 127.0;

    let new_hue =
        (f64::from(orig_hue) * (1.0 - blend_factor) + over_hue_6bit * blend_factor).round() as i32;
    let new_sat =
        (f64::from(orig_sat) * (1.0 - blend_factor) + over_sat_3bit * blend_factor).round() as i32;
    let new_lum = (f64::from(orig_lum) * (1.0 - blend_factor) + f64::from(over_lum) * blend_factor)
        .round() as i32;

    (new_hue.clamp(0, 0x3f) << 10) + (new_sat.clamp(0, 0x7) << 7) + new_lum.clamp(0, 0x7f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packed_vertex::{PackedVertex, VertexInput};

    fn integer_record(priority: i32) -> [i32; INTEGER_FIELD_STRIDE] {
        [128, -32, 256, 0x1234, 255, -1, priority]
    }

    #[test]
    fn batch_builder_deduplicates_complete_packed_vertices() {
        let mut builder = VertexBatchBuilder::default();
        let record = integer_record(11);
        let integer_fields = [record, record].concat();
        let uv_fields = [1.5, 2.25, 1.5, 2.25];
        let flags = [FLAG_REUSE_VERTEX, FLAG_REUSE_VERTEX];

        let indices = builder
            .push_batch(&integer_fields, &uv_fields, &flags)
            .unwrap();

        assert_eq!(indices, vec![0, 0]);
        assert_eq!(builder.vertex_count(), 1);
        assert_eq!(builder.packed_vertices().len(), 3);
    }

    #[test]
    fn batch_builder_matches_single_vertex_codec() {
        let mut builder = VertexBatchBuilder::default();
        let record = integer_record(6);
        let uv_fields = [0.25, 0.75];
        let flags = [FLAG_REUSE_VERTEX | FLAG_PRIORITY_IS_PACKED];

        let indices = builder.push_batch(&record, &uv_fields, &flags).unwrap();
        let expected = PackedVertex::encode(VertexInput {
            x: record[0],
            y: record[1],
            z: record[2],
            hsl: record[3],
            alpha: record[4],
            texture_id: record[5],
            priority: record[6],
            priority_is_packed: true,
            u: uv_fields[0],
            v: uv_fields[1],
        });

        assert_eq!(indices, vec![0]);
        assert_eq!(
            builder.packed_vertices(),
            vec![expected.v0, expected.v1, expected.v2]
        );
    }

    #[test]
    fn batch_builder_validates_parallel_packet_lengths_and_flags() {
        let mut builder = VertexBatchBuilder::default();

        assert!(builder.push_batch(&[1, 2], &[], &[]).is_err());
        assert!(
            builder
                .push_batch(&integer_record(0), &[0.0], &[0])
                .is_err()
        );
        assert!(
            builder
                .push_batch(&integer_record(0), &[0.0, 0.0], &[])
                .is_err()
        );
        assert!(
            builder
                .push_batch(&integer_record(0), &[0.0, 0.0], &[4])
                .is_err()
        );
    }

    #[test]
    fn terrain_builder_skips_hidden_faces_and_matches_uvs() {
        let mut builder = VertexBatchBuilder::default();
        let indices = builder
            .push_terrain_tile(
                &[128, 256, 128, 256],
                &[0, 0, 0, 0],
                &[256, 256, 384, 384],
                &[0, 0],
                &[1, 2],
                &[2, 3],
                &[0x1234, 12_345_678],
                &[0x2345, 12_345_678],
                &[0x3456, 12_345_678],
                &[-1, -1],
                128,
                256,
                -128,
                -256,
            )
            .unwrap();

        assert_eq!(indices.len(), 3);
        assert_eq!(builder.vertex_count(), 3);

        let mut expected = VertexBatchBuilder::default();
        let expected_indices = expected
            .push_batch(
                &[
                    0, 0, 0, 0x1234, 255, -1, 0, 128, 0, 0, 0x2345, 255, -1, 0, 0, 0, 128, 0x3456,
                    255, -1, 0,
                ],
                &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
                &[FLAG_REUSE_VERTEX; 3],
            )
            .unwrap();

        assert_eq!(indices, expected_indices);
        assert_eq!(builder.packed_vertices(), expected.packed_vertices());
    }

    #[test]
    fn terrain_batch_matches_repeated_tile_submission() {
        let mut repeated = VertexBatchBuilder::default();
        let mut repeated_indices = repeated
            .push_terrain_tile(
                &[0, 128, 0],
                &[0, 0, 0],
                &[0, 0, 128],
                &[0],
                &[1],
                &[2],
                &[0x1111],
                &[0x2222],
                &[0x3333],
                &[-1],
                0,
                0,
                0,
                0,
            )
            .unwrap();
        repeated_indices.extend(
            repeated
                .push_terrain_tile(
                    &[128, 256, 128],
                    &[0, 0, 0],
                    &[0, 0, 128],
                    &[0],
                    &[1],
                    &[2],
                    &[0x4444],
                    &[0x5555],
                    &[0x6666],
                    &[-1],
                    128,
                    0,
                    0,
                    0,
                )
                .unwrap(),
        );

        let mut batched = VertexBatchBuilder::default();
        let batch_indices = batched
            .push_terrain_batch(
                &[0, 3, 6],
                &[0, 1, 2],
                &[0, 128, 0, 128, 256, 128],
                &[0, 0, 0, 0, 0, 0],
                &[0, 0, 128, 0, 0, 128],
                &[0, 0],
                &[1, 1],
                &[2, 2],
                &[0x1111, 0x4444],
                &[0x2222, 0x5555],
                &[0x3333, 0x6666],
                &[-1, -1],
                &[0, 128],
                &[0, 0],
                0,
                0,
            )
            .unwrap();

        assert_eq!(batch_indices, repeated_indices);
        assert_eq!(batched.packed_vertices(), repeated.packed_vertices());
    }

    #[test]
    fn model_builder_maps_source_texture_ids_and_tracks_residency() {
        let mut builder = VertexBatchBuilder::default();
        builder.set_texture_id_map(&[7], &[3]).unwrap();

        let indices = builder
            .push_model_faces(
                &[0, 128, 0],
                &[0, 0, 128],
                &[0, 0, 0],
                &[0],
                &[1],
                &[2],
                &[0x1234],
                &[0x2345],
                &[0x3456],
                &[0.0; 6],
                &[0, 255, 0, -1, 7],
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                true,
            )
            .unwrap();

        assert_eq!(indices, vec![0, 1, 2]);
        assert_eq!(builder.used_texture_ids(), vec![7]);

        let expected = crate::packed_vertex::PackedVertex::encode(VertexInput {
            x: 0,
            y: 0,
            z: 0,
            hsl: 0x1234,
            alpha: 255,
            texture_id: 3,
            priority: 0,
            priority_is_packed: false,
            u: 0.0,
            v: 0.0,
        });
        assert_eq!(
            &builder.packed_vertices()[..3],
            &[expected.v0, expected.v1, expected.v2],
        );
    }

    #[test]
    fn model_builder_matches_direct_vertex_semantics() {
        let mut builder = VertexBatchBuilder::default();
        let indices = builder
            .push_model_faces(
                &[0, 128, 0],
                &[0, 0, 128],
                &[0, 0, 0],
                &[0],
                &[1],
                &[2],
                &[0x1234],
                &[0x2345],
                &[-1],
                &[],
                &[0, 255, 11, -1, -1],
                10,
                20,
                30,
                0,
                0,
                0,
                0,
                true,
            )
            .unwrap();

        let mut expected = VertexBatchBuilder::default();
        let expected_indices = expected
            .push_batch(
                &[
                    10, 20, 30, 0x1234, 255, -1, 11, 138, 20, 30, 0x1234, 255, -1, 11, 10, 148, 30,
                    0x1234, 255, -1, 11,
                ],
                &[0.0; 6],
                &[FLAG_REUSE_VERTEX; 3],
            )
            .unwrap();

        assert_eq!(indices, expected_indices);
        assert_eq!(builder.packed_vertices(), expected.packed_vertices());
    }

    #[test]
    fn color_override_matches_typescript_scaling() {
        assert_eq!(apply_color_override(0, 127, 127, 127, 255), 0xffff);
        assert_eq!(apply_color_override(0x1234, 0, 0, 0, 0), 0x1234);
    }

    #[test]
    fn clear_resets_deduplication_state() {
        let mut builder = VertexBatchBuilder::default();
        builder
            .push_batch(&integer_record(0), &[0.0, 0.0], &[FLAG_REUSE_VERTEX])
            .unwrap();
        assert_eq!(builder.vertex_count(), 1);

        builder.clear();

        assert_eq!(builder.vertex_count(), 0);
        assert!(builder.packed_vertices().is_empty());
    }
}
