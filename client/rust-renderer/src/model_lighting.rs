fn adjust_lightness(hsl: i32, mut lightness: i32) -> i32 {
    lightness = (hsl & 127).wrapping_mul(lightness) >> 7;
    lightness = lightness.clamp(2, 126);
    (hsl & 0xff80).wrapping_add(lightness)
}

fn clamp_lightness(lightness: f64) -> i32 {
    if lightness < 2.0 {
        2
    } else if lightness > 126.0 {
        126
    } else {
        lightness.trunc() as i32
    }
}

#[allow(clippy::too_many_arguments)]
pub fn calculate_model_normals(
    vertices_x: &[i32],
    vertices_y: &[i32],
    vertices_z: &[i32],
    used_vertex_count: usize,
    indices1: &[i32],
    indices2: &[i32],
    indices3: &[i32],
    face_render_types: &[i8],
) -> Result<Vec<i32>, String> {
    let vertex_count = vertices_x.len();
    if vertices_y.len() != vertex_count || vertices_z.len() != vertex_count {
        return Err("normal vertices must have matching x/y/z lengths".to_string());
    }
    let face_count = indices1.len();
    if indices2.len() != face_count || indices3.len() != face_count {
        return Err("normal face index arrays must have matching lengths".to_string());
    }
    if !face_render_types.is_empty() && face_render_types.len() != face_count {
        return Err(format!(
            "normal face render types have {} entries for {} faces",
            face_render_types.len(),
            face_count
        ));
    }

    let used = used_vertex_count.min(vertex_count);
    let mut vertex_normals = vec![[0i32; 4]; used];
    let mut face_normals = vec![[0i32; 4]; face_count];

    for face in 0..face_count {
        let a = indices1[face];
        let b = indices2[face];
        let c = indices3[face];
        if a < 0
            || b < 0
            || c < 0
            || a as usize >= used
            || b as usize >= used
            || c as usize >= used
        {
            return Err(format!("normal face {face} references vertex outside used range"));
        }
        let a = a as usize;
        let b = b as usize;
        let c = c as usize;

        let var5 = vertices_x[b] as i64 - vertices_x[a] as i64;
        let var6 = vertices_y[b] as i64 - vertices_y[a] as i64;
        let var7 = vertices_z[b] as i64 - vertices_z[a] as i64;
        let var8 = vertices_x[c] as i64 - vertices_x[a] as i64;
        let var9 = vertices_y[c] as i64 - vertices_y[a] as i64;
        let var10 = vertices_z[c] as i64 - vertices_z[a] as i64;
        let mut nx = var6 * var10 - var9 * var7;
        let mut ny = var7 * var8 - var10 * var5;
        let mut nz = var5 * var9 - var8 * var6;

        while nx > 8192 || ny > 8192 || nz > 8192 || nx < -8192 || ny < -8192 || nz < -8192 {
            nx >>= 1;
            ny >>= 1;
            nz >>= 1;
        }

        let mut magnitude =
            ((nx * nx + ny * ny + nz * nz) as f64).sqrt().trunc() as i64;
        if magnitude <= 0 {
            magnitude = 1;
        }
        let nx = ((nx * 256) / magnitude) as i32;
        let ny = ((ny * 256) / magnitude) as i32;
        let nz = ((nz * 256) / magnitude) as i32;
        let render_type = if face_render_types.is_empty() {
            0
        } else {
            face_render_types[face] as i32
        };

        if render_type == 0 {
            for vertex in [a, b, c] {
                vertex_normals[vertex][0] = vertex_normals[vertex][0].wrapping_add(nx);
                vertex_normals[vertex][1] = vertex_normals[vertex][1].wrapping_add(ny);
                vertex_normals[vertex][2] = vertex_normals[vertex][2].wrapping_add(nz);
                vertex_normals[vertex][3] = vertex_normals[vertex][3].wrapping_add(1);
            }
        } else if render_type == 1 {
            face_normals[face] = [1, nx, ny, nz];
        }
    }

    let mut result = Vec::with_capacity(2 + used * 4 + face_count * 4);
    result.push(used as i32);
    result.push(face_count as i32);
    for normal in vertex_normals {
        result.extend(normal);
    }
    for normal in face_normals {
        result.extend(normal);
    }
    Ok(result)
}

fn select_normal(
    vertex: i32,
    vertex_normals: &[i32],
    merged_normals: &[i32],
) -> Result<(i32, i32, i32, i32), String> {
    if vertex < 0 {
        return Err(format!("lighting references negative vertex {vertex}"));
    }
    let vertex = vertex as usize;
    let base_offset = vertex * 4;
    if base_offset + 3 >= vertex_normals.len() {
        return Err(format!("lighting references missing vertex normal {vertex}"));
    }
    if !merged_normals.is_empty() {
        let merged_offset = vertex * 5;
        if merged_offset + 4 >= merged_normals.len() {
            return Err(format!("lighting merged normal packet missing vertex {vertex}"));
        }
        if merged_normals[merged_offset] != 0 {
            return Ok((
                merged_normals[merged_offset + 1],
                merged_normals[merged_offset + 2],
                merged_normals[merged_offset + 3],
                merged_normals[merged_offset + 4],
            ));
        }
    }
    Ok((
        vertex_normals[base_offset],
        vertex_normals[base_offset + 1],
        vertex_normals[base_offset + 2],
        vertex_normals[base_offset + 3],
    ))
}

fn vertex_light(
    normal: (i32, i32, i32, i32),
    ambient: i32,
    light_intensity: i32,
    light_x: i32,
    light_y: i32,
    light_z: i32,
) -> f64 {
    let dot = light_y as i64 * normal.1 as i64
        + light_z as i64 * normal.2 as i64
        + light_x as i64 * normal.0 as i64;
    let denominator = light_intensity as i64 * normal.3 as i64;
    if denominator == 0 {
        ambient as f64
    } else {
        ambient as f64 + dot as f64 / denominator as f64
    }
}

#[allow(clippy::too_many_arguments)]
pub fn light_model_faces(
    indices1: &[i32],
    indices2: &[i32],
    indices3: &[i32],
    face_colors: &[u16],
    face_render_types: &[i8],
    face_alphas: &[i8],
    face_textures: &[i16],
    vertex_normals: &[i32],
    merged_normals: &[i32],
    face_normals: &[i32],
    ambient: i32,
    contrast: i32,
    light_x: i32,
    light_y: i32,
    light_z: i32,
) -> Result<Vec<i32>, String> {
    let face_count = indices1.len();
    for (name, len) in [
        ("indices2", indices2.len()),
        ("indices3", indices3.len()),
        ("faceColors", face_colors.len()),
    ] {
        if len != face_count {
            return Err(format!("lighting {name} has {len} entries for {face_count} faces"));
        }
    }
    for (name, len) in [
        ("faceRenderTypes", face_render_types.len()),
        ("faceAlphas", face_alphas.len()),
        ("faceTextures", face_textures.len()),
    ] {
        if len != 0 && len != face_count {
            return Err(format!("lighting {name} has {len} entries for {face_count} faces"));
        }
    }
    if !vertex_normals.len().is_multiple_of(4) {
        return Err("lighting vertex normal packet is malformed".to_string());
    }
    let vertex_count = vertex_normals.len() / 4;
    if !merged_normals.is_empty() && merged_normals.len() != vertex_count * 5 {
        return Err("lighting merged normal packet is malformed".to_string());
    }
    if face_normals.len() != face_count * 4 {
        return Err("lighting face normal packet is malformed".to_string());
    }

    let magnitude = ((light_z as f64 * light_z as f64)
        + (light_x as f64 * light_x as f64)
        + (light_y as f64 * light_y as f64))
        .sqrt()
        .trunc() as i32;
    let light_intensity = magnitude.wrapping_mul(contrast) >> 8;
    let face_intensity = (light_intensity >> 1).wrapping_add(light_intensity);

    let mut result = vec![0i32; face_count * 3];

    for face in 0..face_count {
        let mut render_type = if face_render_types.is_empty() {
            0
        } else {
            face_render_types[face] as i32
        };
        let alpha = if face_alphas.is_empty() {
            0
        } else {
            face_alphas[face] as i32
        };
        let texture = if face_textures.is_empty() {
            -1
        } else {
            face_textures[face] as i32
        };

        if alpha == -2 {
            render_type = 3;
        }
        if alpha == -1 {
            render_type = 2;
        }

        let out = face * 3;
        if texture == -1 {
            if render_type == 0 {
                let color = face_colors[face] as i32;
                for (component, vertex) in
                    [indices1[face], indices2[face], indices3[face]]
                        .into_iter()
                        .enumerate()
                {
                    let normal = select_normal(vertex, vertex_normals, merged_normals)?;
                    let light =
                        vertex_light(normal, ambient, light_intensity, light_x, light_y, light_z);
                    let packed = (light.trunc() as i32).wrapping_shl(17);
                    result[out + component] =
                        packed | adjust_lightness(color, packed >> 17);
                }
            } else if render_type == 1 {
                let normal_offset = face * 4;
                if face_normals[normal_offset] != 0 {
                    let normal = (
                        face_normals[normal_offset + 1],
                        face_normals[normal_offset + 2],
                        face_normals[normal_offset + 3],
                    );
                    let dot = light_y as i64 * normal.1 as i64
                        + light_z as i64 * normal.2 as i64
                        + light_x as i64 * normal.0 as i64;
                    let light = if face_intensity == 0 {
                        ambient as f64
                    } else {
                        ambient as f64 + dot as f64 / face_intensity as f64
                    };
                    let packed = (light.trunc() as i32).wrapping_shl(17);
                    result[out] =
                        packed | adjust_lightness(face_colors[face] as i32, packed >> 17);
                    result[out + 2] = -1;
                }
            } else if render_type == 3 {
                result[out] = 128;
                result[out + 2] = -1;
            } else {
                result[out + 2] = -2;
            }
        } else if render_type == 0 {
            for (component, vertex) in
                [indices1[face], indices2[face], indices3[face]]
                    .into_iter()
                    .enumerate()
            {
                let normal = select_normal(vertex, vertex_normals, merged_normals)?;
                let light =
                    vertex_light(normal, ambient, light_intensity, light_x, light_y, light_z);
                result[out + component] = clamp_lightness(light);
            }
        } else if render_type == 1 {
            let normal_offset = face * 4;
            if face_normals[normal_offset] != 0 {
                let nx = face_normals[normal_offset + 1];
                let ny = face_normals[normal_offset + 2];
                let nz = face_normals[normal_offset + 3];
                let dot = light_y as i64 * ny as i64
                    + light_z as i64 * nz as i64
                    + light_x as i64 * nx as i64;
                let light = if face_intensity == 0 {
                    ambient as f64
                } else {
                    ambient as f64 + dot as f64 / face_intensity as f64
                };
                result[out] = clamp_lightness(light);
                result[out + 2] = -1;
            }
        } else {
            result[out + 2] = -2;
        }
    }

    Ok(result)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(js_name = calculate_model_normals)]
#[allow(clippy::too_many_arguments)]
pub fn calculate_model_normals_wasm(
    vertices_x: &[i32],
    vertices_y: &[i32],
    vertices_z: &[i32],
    used_vertex_count: usize,
    indices1: &[i32],
    indices2: &[i32],
    indices3: &[i32],
    face_render_types: &[i8],
) -> Result<Vec<i32>, wasm_bindgen::JsValue> {
    calculate_model_normals(
        vertices_x,
        vertices_y,
        vertices_z,
        used_vertex_count,
        indices1,
        indices2,
        indices3,
        face_render_types,
    )
    .map_err(|error| wasm_bindgen::JsValue::from_str(&error))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(js_name = light_model_faces)]
#[allow(clippy::too_many_arguments)]
pub fn light_model_faces_wasm(
    indices1: &[i32],
    indices2: &[i32],
    indices3: &[i32],
    face_colors: &[u16],
    face_render_types: &[i8],
    face_alphas: &[i8],
    face_textures: &[i16],
    vertex_normals: &[i32],
    merged_normals: &[i32],
    face_normals: &[i32],
    ambient: i32,
    contrast: i32,
    light_x: i32,
    light_y: i32,
    light_z: i32,
) -> Result<Vec<i32>, wasm_bindgen::JsValue> {
    light_model_faces(
        indices1,
        indices2,
        indices3,
        face_colors,
        face_render_types,
        face_alphas,
        face_textures,
        vertex_normals,
        merged_normals,
        face_normals,
        ambient,
        contrast,
        light_x,
        light_y,
        light_z,
    )
    .map_err(|error| wasm_bindgen::JsValue::from_str(&error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_triangle_builds_expected_vertex_normals() {
        let result = calculate_model_normals(
            &[0, 128, 0],
            &[0, 0, 0],
            &[0, 0, 128],
            3,
            &[0],
            &[1],
            &[2],
            &[],
        )
        .unwrap();

        assert_eq!(&result[..2], &[3, 1]);
        assert_eq!(&result[2..6], &[0, -256, 0, 1]);
        assert_eq!(&result[6..10], &[0, -256, 0, 1]);
        assert_eq!(&result[10..14], &[0, -256, 0, 1]);
        assert_eq!(&result[14..18], &[0, 0, 0, 0]);
    }

    #[test]
    fn textured_flat_triangle_lights_all_corners() {
        let normals = [0, -256, 0, 1, 0, -256, 0, 1, 0, -256, 0, 1];
        let face_normals = [0, 0, 0, 0];
        let result = light_model_faces(
            &[0],
            &[1],
            &[2],
            &[0x1234],
            &[],
            &[],
            &[7],
            &normals,
            &[],
            &face_normals,
            64,
            768,
            -50,
            -10,
            -50,
        )
        .unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], result[1]);
        assert_eq!(result[1], result[2]);
        assert!((2..=126).contains(&result[0]));
    }
}
