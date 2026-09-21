fn js_round_to_i32(value: f32) -> i32 {
    // JavaScript Math.round() resolves .5 ties toward +infinity.
    (value + 0.5).floor() as i32
}

pub fn skin_skeletal_vertices(
    vertices_x: &[i32],
    vertices_y: &[i32],
    vertices_z: &[i32],
    vertex_group_offsets: &[u32],
    bone_ids: &[i32],
    bone_scales: &[i32],
    bone_matrices: &[f32],
) -> Result<Vec<i32>, String> {
    let vertex_count = vertices_x.len();
    if vertices_y.len() != vertex_count || vertices_z.len() != vertex_count {
        return Err("skeletal vertices must have matching x/y/z lengths".to_string());
    }
    if vertex_group_offsets.len() != vertex_count + 1 {
        return Err(format!(
            "skeletal group offsets have {} entries for {} vertices",
            vertex_group_offsets.len(),
            vertex_count
        ));
    }
    if bone_ids.len() != bone_scales.len() {
        return Err(format!(
            "skeletal bone packet has {} ids but {} scales",
            bone_ids.len(),
            bone_scales.len()
        ));
    }
    if bone_matrices.len() % 16 != 0 {
        return Err(format!(
            "skeletal bone matrix packet length {} is not divisible by 16",
            bone_matrices.len()
        ));
    }

    let influence_count = bone_ids.len();
    let last_offset = vertex_group_offsets.last().copied().unwrap_or(0) as usize;
    if last_offset != influence_count {
        return Err(format!(
            "skeletal group offsets end at {} but packet has {} influences",
            last_offset, influence_count
        ));
    }
    for pair in vertex_group_offsets.windows(2) {
        if pair[0] > pair[1] || pair[1] as usize > influence_count {
            return Err("skeletal group offsets are not monotonic".to_string());
        }
    }

    let bone_count = bone_matrices.len() / 16;
    let mut transformed = Vec::with_capacity(vertex_count.saturating_mul(3));

    for vertex in 0..vertex_count {
        let start = vertex_group_offsets[vertex] as usize;
        let end = vertex_group_offsets[vertex + 1] as usize;
        if start == end {
            transformed.push(vertices_x[vertex]);
            transformed.push(vertices_y[vertex]);
            transformed.push(vertices_z[vertex]);
            continue;
        }

        let vx = vertices_x[vertex] as f32;
        let vy = -(vertices_y[vertex] as f32);
        let vz = -(vertices_z[vertex] as f32);

        let mut out_x = 0.0f32;
        let mut out_y = 0.0f32;
        let mut out_z = 0.0f32;

        for influence in start..end {
            let bone_id = bone_ids[influence];
            if bone_id < 0 || bone_id as usize >= bone_count {
                continue;
            }
            let matrix_offset = bone_id as usize * 16;
            let matrix = &bone_matrices[matrix_offset..matrix_offset + 16];
            let weight = bone_scales[influence] as f32 / 255.0;

            out_x += weight
                * (matrix[0] * vx + matrix[4] * vy + matrix[8] * vz + matrix[12]);
            out_y += weight
                * (matrix[1] * vx + matrix[5] * vy + matrix[9] * vz + matrix[13]);
            out_z += weight
                * (matrix[2] * vx + matrix[6] * vy + matrix[10] * vz + matrix[14]);
        }

        transformed.push(js_round_to_i32(out_x));
        transformed.push(-js_round_to_i32(out_y));
        transformed.push(-js_round_to_i32(out_z));
    }

    Ok(transformed)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(js_name = skin_skeletal_vertices)]
pub fn skin_skeletal_vertices_wasm(
    vertices_x: &[i32],
    vertices_y: &[i32],
    vertices_z: &[i32],
    vertex_group_offsets: &[u32],
    bone_ids: &[i32],
    bone_scales: &[i32],
    bone_matrices: &[f32],
) -> Result<Vec<i32>, wasm_bindgen::JsValue> {
    skin_skeletal_vertices(
        vertices_x,
        vertices_y,
        vertices_z,
        vertex_group_offsets,
        bone_ids,
        bone_scales,
        bone_matrices,
    )
    .map_err(|error| wasm_bindgen::JsValue::from_str(&error))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> [f32; 16] {
        [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ]
    }

    #[test]
    fn skeletal_skinning_preserves_unweighted_vertices() {
        let out = skin_skeletal_vertices(
            &[10],
            &[-20],
            &[30],
            &[0, 0],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(out, vec![10, -20, 30]);
    }

    #[test]
    fn skeletal_skinning_matches_identity_weight() {
        let out = skin_skeletal_vertices(
            &[10],
            &[-20],
            &[30],
            &[0, 1],
            &[0],
            &[255],
            &identity(),
        )
        .unwrap();
        assert_eq!(out, vec![10, -20, 30]);
    }

    #[test]
    fn skeletal_skinning_applies_weighted_translation_with_model_axes() {
        let mut matrix = identity();
        matrix[12] = 5.0;
        matrix[13] = 7.0;
        matrix[14] = -9.0;
        let out = skin_skeletal_vertices(
            &[10],
            &[-20],
            &[30],
            &[0, 1],
            &[0],
            &[255],
            &matrix,
        )
        .unwrap();
        assert_eq!(out, vec![15, -27, 39]);
    }

    #[test]
    fn skeletal_skinning_blends_multiple_bones() {
        let mut matrices = Vec::new();
        let mut left = identity();
        left[12] = -10.0;
        let mut right = identity();
        right[12] = 10.0;
        matrices.extend_from_slice(&left);
        matrices.extend_from_slice(&right);

        let out = skin_skeletal_vertices(
            &[20],
            &[0],
            &[0],
            &[0, 2],
            &[0, 1],
            &[128, 127],
            &matrices,
        )
        .unwrap();
        assert_eq!(out[0], 20);
    }
}
