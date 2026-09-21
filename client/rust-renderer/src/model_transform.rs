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
    if !bone_matrices.len().is_multiple_of(16) {
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

            out_x += weight * (matrix[0] * vx + matrix[4] * vy + matrix[8] * vz + matrix[12]);
            out_y += weight * (matrix[1] * vx + matrix[5] * vy + matrix[9] * vz + matrix[13]);
            out_z += weight * (matrix[2] * vx + matrix[6] * vy + matrix[10] * vz + matrix[14]);
        }

        transformed.push(js_round_to_i32(out_x));
        transformed.push(-js_round_to_i32(out_y));
        transformed.push(-js_round_to_i32(out_z));
    }

    Ok(transformed)
}

const LEGACY_TRANSFORM_ORIGIN: i32 = 0;
const LEGACY_TRANSFORM_TRANSLATE: i32 = 1;
const LEGACY_TRANSFORM_ROTATE: i32 = 2;
const LEGACY_TRANSFORM_SCALE: i32 = 3;
const LEGACY_TRANSFORM_ALPHA: i32 = 5;
const LEGACY_TRANSFORM_LIGHT: i32 = 7;
const LEGACY_RESULT_HEADER: usize = 7;

fn rs_trig(angle: i32) -> (i32, i32) {
    let radians = angle as f64 * std::f64::consts::TAU / 2048.0;
    (
        (65536.0 * radians.sin()).trunc() as i32,
        (65536.0 * radians.cos()).trunc() as i32,
    )
}

fn visit_label_indices(
    label_offsets: &[u32],
    label_indices: &[i32],
    labels: &[i32],
    mut visitor: impl FnMut(usize),
) {
    if label_offsets.is_empty() {
        return;
    }
    let label_count = label_offsets.len() - 1;
    for &label in labels {
        if label < 0 || label as usize >= label_count {
            continue;
        }
        let start = label_offsets[label as usize] as usize;
        let end = label_offsets[label as usize + 1] as usize;
        if start > end || end > label_indices.len() {
            continue;
        }
        for &index in &label_indices[start..end] {
            if index >= 0 {
                visitor(index as usize);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn apply_legacy_transforms(
    vertices_x: &[i32],
    vertices_y: &[i32],
    vertices_z: &[i32],
    face_alphas: &[i8],
    face_colors: &[u16],
    vertex_label_offsets: &[u32],
    vertex_label_indices: &[i32],
    face_label_offsets: &[u32],
    face_label_indices: &[i32],
    operation_fields: &[i32],
    operation_label_offsets: &[u32],
    operation_labels: &[i32],
    initial_origin_x: i32,
    initial_origin_y: i32,
    initial_origin_z: i32,
) -> Result<Vec<i32>, String> {
    let vertex_count = vertices_x.len();
    if vertices_y.len() != vertex_count || vertices_z.len() != vertex_count {
        return Err("legacy vertices must have matching x/y/z lengths".to_string());
    }
    if !operation_fields.len().is_multiple_of(4) {
        return Err(format!(
            "legacy operation field length {} is not divisible by 4",
            operation_fields.len()
        ));
    }
    let operation_count = operation_fields.len() / 4;
    if operation_label_offsets.len() != operation_count + 1 {
        return Err(format!(
            "legacy operation label offsets have {} entries for {} operations",
            operation_label_offsets.len(),
            operation_count
        ));
    }
    if operation_label_offsets.last().copied().unwrap_or(0) as usize != operation_labels.len() {
        return Err("legacy operation label offsets do not cover label packet".to_string());
    }

    let mut x = vertices_x.to_vec();
    let mut y = vertices_y.to_vec();
    let mut z = vertices_z.to_vec();
    let mut alphas: Vec<i32> = face_alphas
        .iter()
        .map(|value| (*value as i32) & 0xff)
        .collect();
    let mut colors: Vec<i32> = face_colors.iter().map(|value| *value as i32).collect();

    let mut origin_x = initial_origin_x;
    let mut origin_y = initial_origin_y;
    let mut origin_z = initial_origin_z;
    let mut changed_light = false;

    for operation in 0..operation_count {
        let field = operation * 4;
        let transform_type = operation_fields[field];
        let tx = operation_fields[field + 1];
        let ty = operation_fields[field + 2];
        let tz = operation_fields[field + 3];
        let label_start = operation_label_offsets[operation] as usize;
        let label_end = operation_label_offsets[operation + 1] as usize;
        if label_start > label_end || label_end > operation_labels.len() {
            return Err("legacy operation label offsets are not monotonic".to_string());
        }
        let labels = &operation_labels[label_start..label_end];

        match transform_type {
            LEGACY_TRANSFORM_ORIGIN => {
                let mut sum_x = 0i64;
                let mut sum_y = 0i64;
                let mut sum_z = 0i64;
                let mut count = 0i64;
                visit_label_indices(
                    vertex_label_offsets,
                    vertex_label_indices,
                    labels,
                    |vertex| {
                        if vertex < vertex_count {
                            sum_x += x[vertex] as i64;
                            sum_y += y[vertex] as i64;
                            sum_z += z[vertex] as i64;
                            count += 1;
                        }
                    },
                );
                if count > 0 {
                    origin_x = tx.wrapping_add((sum_x / count) as i32);
                    origin_y = ty.wrapping_add((sum_y / count) as i32);
                    origin_z = tz.wrapping_add((sum_z / count) as i32);
                } else {
                    origin_x = tx;
                    origin_y = ty;
                    origin_z = tz;
                }
            }
            LEGACY_TRANSFORM_TRANSLATE => {
                visit_label_indices(
                    vertex_label_offsets,
                    vertex_label_indices,
                    labels,
                    |vertex| {
                        if vertex < vertex_count {
                            x[vertex] = x[vertex].wrapping_add(tx);
                            y[vertex] = y[vertex].wrapping_add(ty);
                            z[vertex] = z[vertex].wrapping_add(tz);
                        }
                    },
                );
            }
            LEGACY_TRANSFORM_ROTATE => {
                let angle_x = (tx & 0xff) * 8;
                let angle_y = (ty & 0xff) * 8;
                let angle_z = (tz & 0xff) * 8;
                let (sin_x, cos_x) = rs_trig(angle_x);
                let (sin_y, cos_y) = rs_trig(angle_y);
                let (sin_z, cos_z) = rs_trig(angle_z);
                visit_label_indices(
                    vertex_label_offsets,
                    vertex_label_indices,
                    labels,
                    |vertex| {
                        if vertex >= vertex_count {
                            return;
                        }
                        let mut vx = x[vertex].wrapping_sub(origin_x);
                        let mut vy = y[vertex].wrapping_sub(origin_y);
                        let mut vz = z[vertex].wrapping_sub(origin_z);

                        if angle_z != 0 {
                            let temp =
                                sin_z.wrapping_mul(vy).wrapping_add(cos_z.wrapping_mul(vx)) >> 16;
                            vy = cos_z.wrapping_mul(vy).wrapping_sub(sin_z.wrapping_mul(vx)) >> 16;
                            vx = temp;
                        }
                        if angle_x != 0 {
                            let temp =
                                cos_x.wrapping_mul(vy).wrapping_sub(sin_x.wrapping_mul(vz)) >> 16;
                            vz = sin_x.wrapping_mul(vy).wrapping_add(cos_x.wrapping_mul(vz)) >> 16;
                            vy = temp;
                        }
                        if angle_y != 0 {
                            let temp =
                                sin_y.wrapping_mul(vz).wrapping_add(cos_y.wrapping_mul(vx)) >> 16;
                            vz = cos_y.wrapping_mul(vz).wrapping_sub(sin_y.wrapping_mul(vx)) >> 16;
                            vx = temp;
                        }

                        x[vertex] = vx.wrapping_add(origin_x);
                        y[vertex] = vy.wrapping_add(origin_y);
                        z[vertex] = vz.wrapping_add(origin_z);
                    },
                );
            }
            LEGACY_TRANSFORM_SCALE => {
                visit_label_indices(
                    vertex_label_offsets,
                    vertex_label_indices,
                    labels,
                    |vertex| {
                        if vertex >= vertex_count {
                            return;
                        }
                        let vx = x[vertex].wrapping_sub(origin_x);
                        let vy = y[vertex].wrapping_sub(origin_y);
                        let vz = z[vertex].wrapping_sub(origin_z);
                        x[vertex] = (((tx as i64 * vx as i64) / 128) as i32).wrapping_add(origin_x);
                        y[vertex] = (((ty as i64 * vy as i64) / 128) as i32).wrapping_add(origin_y);
                        z[vertex] = (((tz as i64 * vz as i64) / 128) as i32).wrapping_add(origin_z);
                    },
                );
            }
            LEGACY_TRANSFORM_ALPHA => {
                visit_label_indices(face_label_offsets, face_label_indices, labels, |face| {
                    if face < alphas.len() {
                        alphas[face] = (alphas[face] + tx * 8).clamp(0, 255);
                    }
                });
            }
            LEGACY_TRANSFORM_LIGHT => {
                visit_label_indices(face_label_offsets, face_label_indices, labels, |face| {
                    if face >= colors.len() {
                        return;
                    }
                    let color = colors[face] & 0xffff;
                    let hue = (((color >> 10) & 0x3f) + tx) & 0x3f;
                    let saturation = (((color >> 7) & 0x7) + ty).clamp(0, 7);
                    let lightness = ((color & 0x7f) + tz).clamp(0, 127);
                    colors[face] = (hue << 10) + (saturation << 7) + lightness;
                    changed_light = true;
                });
            }
            _ => {}
        }
    }

    let mut result =
        Vec::with_capacity(LEGACY_RESULT_HEADER + vertex_count * 3 + alphas.len() + colors.len());
    result.push(origin_x);
    result.push(origin_y);
    result.push(origin_z);
    result.push(i32::from(changed_light));
    result.push(vertex_count as i32);
    result.push(alphas.len() as i32);
    result.push(colors.len() as i32);
    for vertex in 0..vertex_count {
        result.push(x[vertex]);
        result.push(y[vertex]);
        result.push(z[vertex]);
    }
    result.extend(alphas);
    result.extend(colors);
    Ok(result)
}

fn sample_height(
    heights: &[i32],
    width: usize,
    height: usize,
    x: i32,
    z: i32,
    rx: i32,
    rz: i32,
) -> Option<i32> {
    if x < 0 || z < 0 || x as usize + 1 >= width || z as usize + 1 >= height {
        return None;
    }
    let x0 = x as usize;
    let z0 = z as usize;
    let index = |xx: usize, zz: usize| xx * height + zz;
    let h0 = heights[index(x0, z0)]
        .wrapping_mul(128 - rx)
        .wrapping_add(heights[index(x0 + 1, z0)].wrapping_mul(rx))
        >> 7;
    let h1 = heights[index(x0, z0 + 1)]
        .wrapping_mul(128 - rx)
        .wrapping_add(heights[index(x0 + 1, z0 + 1)].wrapping_mul(rx))
        >> 7;
    Some(h0.wrapping_mul(128 - rz).wrapping_add(h1.wrapping_mul(rz)) >> 7)
}

#[allow(clippy::too_many_arguments)]
pub fn contour_vertices_y(
    vertices_x: &[i32],
    vertices_y: &[i32],
    vertices_z: &[i32],
    used_vertex_count: usize,
    contour_type: i32,
    param: i32,
    height_map: &[i32],
    height_width: usize,
    height_depth: usize,
    height_map_above: &[i32],
    above_width: usize,
    above_depth: usize,
    scene_x: i32,
    scene_height: i32,
    scene_z: i32,
    type2_denominator: i32,
    min_y: i32,
    max_y: i32,
    preserve_type1_unused_oob: bool,
) -> Result<Vec<i32>, String> {
    let vertex_count = vertices_x.len();
    if vertices_y.len() != vertex_count || vertices_z.len() != vertex_count {
        return Err("contour vertices must have matching x/y/z lengths".to_string());
    }
    if height_width.saturating_mul(height_depth) != height_map.len() {
        return Err("contour height-map dimensions do not match data length".to_string());
    }
    if !height_map_above.is_empty()
        && above_width.saturating_mul(above_depth) != height_map_above.len()
    {
        return Err("contour above-height-map dimensions do not match data length".to_string());
    }

    let used = used_vertex_count.min(vertex_count);
    let mut output = vec![0; vertex_count];
    let delta_y = max_y.wrapping_sub(min_y);

    for vertex in 0..vertex_count {
        let is_used = vertex < used;
        let vx = vertices_x[vertex].wrapping_add(scene_x);
        let vz = vertices_z[vertex].wrapping_add(scene_z);
        let rx = vx & 0x7f;
        let rz = vz & 0x7f;
        let tile_x = vx >> 7;
        let tile_z = vz >> 7;
        let base_height = || {
            sample_height(
                height_map,
                height_width,
                height_depth,
                tile_x,
                tile_z,
                rx,
                rz,
            )
        };
        let above_height = || {
            sample_height(
                height_map_above,
                above_width,
                above_depth,
                tile_x,
                tile_z,
                rx,
                rz,
            )
        };

        match contour_type {
            1 => {
                if let Some(height) = base_height() {
                    output[vertex] = vertices_y[vertex]
                        .wrapping_add(height)
                        .wrapping_sub(scene_height);
                } else if !is_used && preserve_type1_unused_oob {
                    output[vertex] = vertices_y[vertex];
                }
            }
            2 => {
                let shifted = vertices_y[vertex].wrapping_shl(16);
                let y_ratio = if type2_denominator == 0 {
                    0
                } else {
                    shifted / type2_denominator
                };
                if y_ratio < param {
                    if let Some(height) = base_height() {
                        if param == 0 {
                            output[vertex] = vertices_y[vertex];
                        } else {
                            let delta = (height.wrapping_sub(scene_height) as i64)
                                * (param.wrapping_sub(y_ratio) as i64);
                            output[vertex] =
                                vertices_y[vertex].wrapping_add((delta / param as i64) as i32);
                        }
                    }
                } else {
                    output[vertex] = vertices_y[vertex];
                }
            }
            3 => {
                if !is_used {
                    continue;
                }
                if let Some(height) = base_height() {
                    let mut delta = height.wrapping_sub(scene_height);
                    let clamp_limit = param.saturating_abs();
                    if clamp_limit > 0 {
                        delta = delta.clamp(-clamp_limit, clamp_limit);
                    }
                    output[vertex] = vertices_y[vertex].wrapping_add(delta);
                } else {
                    output[vertex] = vertices_y[vertex];
                }
            }
            4 => {
                if !is_used {
                    continue;
                }
                if let Some(height) = above_height() {
                    output[vertex] = vertices_y[vertex]
                        .wrapping_add(height)
                        .wrapping_sub(scene_height)
                        .wrapping_add(delta_y);
                }
            }
            5 => {
                if !is_used {
                    continue;
                }
                if let (Some(height), Some(height_above)) = (base_height(), above_height()) {
                    let delta_height = height.wrapping_sub(height_above);
                    let shifted = vertices_y[vertex].wrapping_shl(8);
                    let ratio = if delta_y == 0 { 0 } else { shifted / delta_y };
                    output[vertex] = ratio
                        .wrapping_mul(delta_height)
                        .wrapping_shr(8)
                        .wrapping_sub(scene_height.wrapping_sub(height));
                }
            }
            _ => {
                output[vertex] = vertices_y[vertex];
            }
        }
    }

    Ok(output)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(js_name = contour_vertices_y)]
#[allow(clippy::too_many_arguments)]
pub fn contour_vertices_y_wasm(
    vertices_x: &[i32],
    vertices_y: &[i32],
    vertices_z: &[i32],
    used_vertex_count: usize,
    contour_type: i32,
    param: i32,
    height_map: &[i32],
    height_width: usize,
    height_depth: usize,
    height_map_above: &[i32],
    above_width: usize,
    above_depth: usize,
    scene_x: i32,
    scene_height: i32,
    scene_z: i32,
    type2_denominator: i32,
    min_y: i32,
    max_y: i32,
    preserve_type1_unused_oob: bool,
) -> Result<Vec<i32>, wasm_bindgen::JsValue> {
    contour_vertices_y(
        vertices_x,
        vertices_y,
        vertices_z,
        used_vertex_count,
        contour_type,
        param,
        height_map,
        height_width,
        height_depth,
        height_map_above,
        above_width,
        above_depth,
        scene_x,
        scene_height,
        scene_z,
        type2_denominator,
        min_y,
        max_y,
        preserve_type1_unused_oob,
    )
    .map_err(|error| wasm_bindgen::JsValue::from_str(&error))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(js_name = apply_legacy_transforms)]
#[allow(clippy::too_many_arguments)]
pub fn apply_legacy_transforms_wasm(
    vertices_x: &[i32],
    vertices_y: &[i32],
    vertices_z: &[i32],
    face_alphas: &[i8],
    face_colors: &[u16],
    vertex_label_offsets: &[u32],
    vertex_label_indices: &[i32],
    face_label_offsets: &[u32],
    face_label_indices: &[i32],
    operation_fields: &[i32],
    operation_label_offsets: &[u32],
    operation_labels: &[i32],
    initial_origin_x: i32,
    initial_origin_y: i32,
    initial_origin_z: i32,
) -> Result<Vec<i32>, wasm_bindgen::JsValue> {
    apply_legacy_transforms(
        vertices_x,
        vertices_y,
        vertices_z,
        face_alphas,
        face_colors,
        vertex_label_offsets,
        vertex_label_indices,
        face_label_offsets,
        face_label_indices,
        operation_fields,
        operation_label_offsets,
        operation_labels,
        initial_origin_x,
        initial_origin_y,
        initial_origin_z,
    )
    .map_err(|error| wasm_bindgen::JsValue::from_str(&error))
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
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]
    }

    #[test]
    fn contour_type_one_matches_bilinear_height_offset() {
        let heights = vec![0, 128, 128, 256];
        let result = contour_vertices_y(
            &[64],
            &[10],
            &[64],
            1,
            1,
            0,
            &heights,
            2,
            2,
            &[],
            0,
            0,
            0,
            0,
            0,
            -1,
            -20,
            20,
            true,
        )
        .unwrap();
        assert_eq!(result, vec![138]);
    }

    #[test]
    fn contour_type_three_clamps_delta() {
        let heights = vec![0, 200, 200, 400];
        let result = contour_vertices_y(
            &[64],
            &[5],
            &[64],
            1,
            3,
            25,
            &heights,
            2,
            2,
            &[],
            0,
            0,
            0,
            0,
            0,
            -1,
            -20,
            20,
            false,
        )
        .unwrap();
        assert_eq!(result, vec![30]);
    }

    #[test]
    fn legacy_transform_batches_origin_translate_scale_and_alpha() {
        let result = apply_legacy_transforms(
            &[10, 30],
            &[20, 40],
            &[0, 0],
            &[0],
            &[0x1234],
            &[0, 2],
            &[0, 1],
            &[0, 1],
            &[0],
            &[
                LEGACY_TRANSFORM_ORIGIN,
                0,
                0,
                0,
                LEGACY_TRANSFORM_SCALE,
                256,
                128,
                128,
                LEGACY_TRANSFORM_TRANSLATE,
                5,
                -3,
                7,
                LEGACY_TRANSFORM_ALPHA,
                2,
                0,
                0,
                LEGACY_TRANSFORM_LIGHT,
                1,
                1,
                1,
            ],
            &[0, 1, 2, 3, 4, 5],
            &[0, 0, 0, 0, 0],
            0,
            0,
            0,
        )
        .unwrap();

        assert_eq!(&result[..7], &[20, 30, 0, 1, 2, 1, 1]);
        assert_eq!(&result[7..13], &[5, 17, 7, 45, 37, 7]);
        assert_eq!(result[13], 16);
        assert_ne!(result[14], 0x1234);
    }

    #[test]
    fn skeletal_skinning_preserves_unweighted_vertices() {
        let out = skin_skeletal_vertices(&[10], &[-20], &[30], &[0, 0], &[], &[], &[]).unwrap();
        assert_eq!(out, vec![10, -20, 30]);
    }

    #[test]
    fn skeletal_skinning_matches_identity_weight() {
        let out = skin_skeletal_vertices(&[10], &[-20], &[30], &[0, 1], &[0], &[255], &identity())
            .unwrap();
        assert_eq!(out, vec![10, -20, 30]);
    }

    #[test]
    fn skeletal_skinning_applies_weighted_translation_with_model_axes() {
        let mut matrix = identity();
        matrix[12] = 5.0;
        matrix[13] = 7.0;
        matrix[14] = -9.0;
        let out =
            skin_skeletal_vertices(&[10], &[-20], &[30], &[0, 1], &[0], &[255], &matrix).unwrap();
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

        let out =
            skin_skeletal_vertices(&[20], &[0], &[0], &[0, 2], &[0, 1], &[128, 127], &matrices)
                .unwrap();
        assert_eq!(out[0], 20);
    }
}
