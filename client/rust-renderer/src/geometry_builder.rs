use crate::packed_vertex::{PackedVertexDeduper, VertexInput};

const INTEGER_FIELD_STRIDE: usize = 7;
const UV_FIELD_STRIDE: usize = 2;
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

        for index in 0..vertex_count {
            let integer_offset = index * INTEGER_FIELD_STRIDE;
            let uv_offset = index * UV_FIELD_STRIDE;
            let record_flags = flags[index];

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
        assert!(builder
            .push_batch(&integer_record(0), &[0.0], &[0])
            .is_err());
        assert!(builder
            .push_batch(&integer_record(0), &[0.0, 0.0], &[])
            .is_err());
        assert!(builder
            .push_batch(&integer_record(0), &[0.0, 0.0], &[4])
            .is_err());
    }

    #[test]
    fn clear_resets_deduplication_state() {
        let mut builder = VertexBatchBuilder::default();
        builder
            .push_batch(
                &integer_record(0),
                &[0.0, 0.0],
                &[FLAG_REUSE_VERTEX],
            )
            .unwrap();
        assert_eq!(builder.vertex_count(), 1);

        builder.clear();

        assert_eq!(builder.vertex_count(), 0);
        assert!(builder.packed_vertices().is_empty());
    }
}
