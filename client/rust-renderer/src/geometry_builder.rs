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
}

impl VertexBatchBuilder {
    pub fn clear(&mut self) {
        self.deduper = PackedVertexDeduper::default();
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
            let texture_index = face[4];

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

            let vertex_indices = [faces_a[face_index], faces_b[face_index], faces_c[face_index]];
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
    let new_lum =
        (f64::from(orig_lum) * (1.0 - blend_factor) + f64::from(over_lum) * blend_factor).round()
            as i32;

    (new_hue.clamp(0, 0x3f) << 10)
        + (new_sat.clamp(0, 0x7) << 7)
        + new_lum.clamp(0, 0x7f)
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
                    10, 20, 30, 0x1234, 255, -1, 11,
                    138, 20, 30, 0x1234, 255, -1, 11,
                    10, 148, 30, 0x1234, 255, -1, 11,
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
