fn vertex(
    vertices_x: &[i32],
    vertices_y: &[i32],
    vertices_z: &[i32],
    index: i32,
) -> Result<(f64, f64, f64), String> {
    if index < 0 || index as usize >= vertices_x.len() {
        return Err(format!("texture mapping references invalid vertex {index}"));
    }
    let index = index as usize;
    Ok((
        vertices_x[index] as f64,
        vertices_y[index] as f64,
        vertices_z[index] as f64,
    ))
}

fn build_rotation_scale_matrix(
    p: i32,
    m: i32,
    n: i32,
    rotation: i32,
    scale_x: f64,
    scale_y: f64,
    scale_z: f64,
) -> [f32; 9] {
    let mut fs = [0.0f32; 9];
    let mut f_552 = 1.0f64;
    let mut f_553 = 0.0f64;
    let mut f_554 = m as f64 / 32767.0;
    let mut f_555 = -(1.0 - f_554 * f_554).sqrt();
    let mut f_556 = 1.0 - f_554;
    let f_557 = ((p as f64) * (p as f64) + (n as f64) * (n as f64)).sqrt();
    if f_557 != 0.0 {
        f_552 = -(n as f64) / f_557;
        f_553 = p as f64 / f_557;
    }
    fs[0] = (f_554 + f_552 * f_552 * f_556) as f32;
    fs[1] = (f_553 * f_555) as f32;
    fs[2] = (f_553 * f_552 * f_556) as f32;
    fs[3] = (-f_553 * f_555) as f32;
    fs[4] = f_554 as f32;
    fs[5] = (f_552 * f_555) as f32;
    fs[6] = (f_552 * f_553 * f_556) as f32;
    fs[7] = (-f_552 * f_555) as f32;
    fs[8] = (f_554 + f_553 * f_553 * f_556) as f32;

    let mut rotation_matrix = [0.0f32; 9];
    f_554 = (rotation as f64 * 0.024543693).cos();
    f_555 = (rotation as f64 * 0.024543693).sin();
    f_556 = 1.0 - f_554;
    let _ = f_556;
    rotation_matrix[0] = f_554 as f32;
    rotation_matrix[1] = 0.0;
    rotation_matrix[2] = f_555 as f32;
    rotation_matrix[3] = 0.0;
    rotation_matrix[4] = 1.0;
    rotation_matrix[5] = 0.0;
    rotation_matrix[6] = (-f_555) as f32;
    rotation_matrix[7] = 0.0;
    rotation_matrix[8] = f_554 as f32;

    let mut out = [0.0f32; 9];
    out[0] = (rotation_matrix[0] as f64 * fs[0] as f64
        + rotation_matrix[1] as f64 * fs[3] as f64
        + rotation_matrix[2] as f64 * fs[6] as f64) as f32;
    out[1] = (rotation_matrix[0] as f64 * fs[1] as f64
        + rotation_matrix[1] as f64 * fs[4] as f64
        + rotation_matrix[2] as f64 * fs[7] as f64) as f32;
    out[2] = (rotation_matrix[0] as f64 * fs[2] as f64
        + rotation_matrix[1] as f64 * fs[5] as f64
        + rotation_matrix[2] as f64 * fs[8] as f64) as f32;
    out[3] = (rotation_matrix[3] as f64 * fs[0] as f64
        + rotation_matrix[4] as f64 * fs[3] as f64
        + rotation_matrix[5] as f64 * fs[6] as f64) as f32;
    out[4] = (rotation_matrix[3] as f64 * fs[1] as f64
        + rotation_matrix[4] as f64 * fs[4] as f64
        + rotation_matrix[5] as f64 * fs[7] as f64) as f32;
    out[5] = (rotation_matrix[3] as f64 * fs[2] as f64
        + rotation_matrix[4] as f64 * fs[5] as f64
        + rotation_matrix[5] as f64 * fs[8] as f64) as f32;
    out[6] = (rotation_matrix[6] as f64 * fs[0] as f64
        + rotation_matrix[7] as f64 * fs[3] as f64
        + rotation_matrix[8] as f64 * fs[6] as f64) as f32;
    out[7] = (rotation_matrix[6] as f64 * fs[1] as f64
        + rotation_matrix[7] as f64 * fs[4] as f64
        + rotation_matrix[8] as f64 * fs[7] as f64) as f32;
    out[8] = (rotation_matrix[6] as f64 * fs[2] as f64
        + rotation_matrix[7] as f64 * fs[5] as f64
        + rotation_matrix[8] as f64 * fs[8] as f64) as f32;

    for value in &mut out[0..3] {
        *value = (*value as f64 * scale_x) as f32;
    }
    for value in &mut out[3..6] {
        *value = (*value as f64 * scale_y) as f32;
    }
    for value in &mut out[6..9] {
        *value = (*value as f64 * scale_z) as f32;
    }
    out
}

fn apply_direction(mut u: f64, mut v: f64, direction: i32) -> (f64, f64) {
    match direction {
        1 => {
            let old_u = u;
            u = -v;
            v = old_u;
        }
        2 => {
            u = -u;
            v = -v;
        }
        3 => {
            let old_u = u;
            u = v;
            v = -old_u;
        }
        _ => {}
    }
    // TextureMapper.ts writes every projected pair through a shared
    // Float32Array before seam correction. Round here so cylindrical and
    // spherical wrap decisions see the same values as the TypeScript path.
    (u as f32 as f64, v as f32 as f64)
}

fn project_cylindrical(
    vertex: (f64, f64, f64),
    center: (i32, i32, i32),
    scales: &[f32; 9],
    scale_z: f64,
    direction: i32,
    speed: f64,
) -> (f64, f64) {
    let vx = vertex.0 - center.0 as f64;
    let vy = vertex.1 - center.1 as f64;
    let vz = vertex.2 - center.2 as f64;
    let f_651 = vx * scales[0] as f64 + vy * scales[1] as f64 + vz * scales[2] as f64;
    let f_652 = vx * scales[3] as f64 + vy * scales[4] as f64 + vz * scales[5] as f64;
    let f_653 = vx * scales[6] as f64 + vy * scales[7] as f64 + vz * scales[8] as f64;
    let mut u = f_651.atan2(f_653) / 6.2831855 + 0.5;
    if scale_z != 1.0 {
        u *= scale_z;
    }
    let v = f_652 + 0.5 + speed;
    apply_direction(u, v, direction)
}

fn dominant_axis(f: f64, f_715: f64, f_716: f64) -> i32 {
    let f_717 = f.abs();
    let f_718 = f_715.abs();
    let f_719 = f_716.abs();
    if f_718 > f_717 && f_718 > f_719 {
        return if f_715 > 0.0 { 0 } else { 1 };
    }
    if f_719 > f_717 && f_719 > f_718 {
        return if f_716 > 0.0 { 2 } else { 3 };
    }
    if f > 0.0 { 4 } else { 5 }
}

#[allow(clippy::too_many_arguments)]
fn project_planar(
    vertex: (f64, f64, f64),
    center: (i32, i32, i32),
    scale_type: i32,
    scales: &[f32; 9],
    direction: i32,
    speed: f64,
    u_offset: f64,
    v_offset: f64,
) -> (f64, f64) {
    let vx = vertex.0 - center.0 as f64;
    let vy = vertex.1 - center.1 as f64;
    let vz = vertex.2 - center.2 as f64;
    let f_223 = vx * scales[0] as f64 + vy * scales[1] as f64 + vz * scales[2] as f64;
    let f_224 = vx * scales[3] as f64 + vy * scales[4] as f64 + vz * scales[5] as f64;
    let f_225 = vx * scales[6] as f64 + vy * scales[7] as f64 + vz * scales[8] as f64;
    let (u, v) = match scale_type {
        0 => (f_223 + speed + 0.5, -f_225 + v_offset + 0.5),
        1 => (f_223 + speed + 0.5, f_225 + v_offset + 0.5),
        2 => (-f_223 + speed + 0.5, -f_224 + u_offset + 0.5),
        3 => (f_223 + speed + 0.5, -f_224 + u_offset + 0.5),
        4 => (f_225 + v_offset + 0.5, -f_224 + u_offset + 0.5),
        _ => (-f_225 + v_offset + 0.5, -f_224 + u_offset + 0.5),
    };
    apply_direction(u, v, direction)
}

#[allow(clippy::approx_constant)]
fn project_spherical(
    vertex: (f64, f64, f64),
    center: (i32, i32, i32),
    scales: &[f32; 9],
    direction: i32,
    speed: f64,
) -> (f64, f64) {
    let vx = vertex.0 - center.0 as f64;
    let vy = vertex.1 - center.1 as f64;
    let vz = vertex.2 - center.2 as f64;
    let f_682 = vx * scales[0] as f64 + vy * scales[1] as f64 + vz * scales[2] as f64;
    let f_683 = vx * scales[3] as f64 + vy * scales[4] as f64 + vz * scales[5] as f64;
    let f_684 = vx * scales[6] as f64 + vy * scales[7] as f64 + vz * scales[8] as f64;
    let f_685 = (f_682 * f_682 + f_683 * f_683 + f_684 * f_684).sqrt();
    let u = f_682.atan2(f_684) / 6.2831855 + 0.5;
    let v = (f_683 / f_685).asin() / 3.1415927 + 0.5 + speed;
    apply_direction(u, v, direction)
}

#[allow(clippy::too_many_arguments)]
pub fn compute_model_uvs(
    vertices_x: &[i32],
    vertices_y: &[i32],
    vertices_z: &[i32],
    indices0: &[i32],
    indices1: &[i32],
    indices2: &[i32],
    face_textures: &[i16],
    texture_coords: &[i8],
    texture_render_types: &[i8],
    texture_mapping_p: &[i16],
    texture_mapping_m: &[i16],
    texture_mapping_n: &[i16],
    texture_scale_x: &[i32],
    texture_scale_y: &[i32],
    texture_scale_z: &[i32],
    texture_rotation: &[i8],
    texture_direction: &[i8],
    texture_speed: &[i32],
    texture_trans_u: &[i32],
    texture_trans_v: &[i32],
) -> Result<Vec<f32>, String> {
    let vertex_count = vertices_x.len();
    if vertices_y.len() != vertex_count || vertices_z.len() != vertex_count {
        return Err("texture mapper vertices must have matching x/y/z lengths".to_string());
    }
    let face_count = indices0.len();
    for (name, len) in [
        ("indices1", indices1.len()),
        ("indices2", indices2.len()),
        ("faceTextures", face_textures.len()),
    ] {
        if len != face_count {
            return Err(format!(
                "texture mapper {name} has {len} entries for {face_count} faces"
            ));
        }
    }
    if !texture_coords.is_empty() && texture_coords.len() != face_count {
        return Err(format!(
            "texture mapper textureCoords has {} entries for {} faces",
            texture_coords.len(),
            face_count
        ));
    }

    let texture_count = texture_render_types.len();
    let mut centers_x = vec![0i32; texture_count];
    let mut centers_y = vec![0i32; texture_count];
    let mut centers_z = vec![0i32; texture_count];
    let mut matrices: Vec<Option<[f32; 9]>> = vec![None; texture_count];

    if !texture_coords.is_empty() {
        let mut min_x = vec![2_147_483_647i32; texture_count];
        let mut max_x = vec![-2_147_483_647i32; texture_count];
        let mut min_y = vec![2_147_483_647i32; texture_count];
        let mut max_y = vec![-2_147_483_647i32; texture_count];
        let mut min_z = vec![2_147_483_647i32; texture_count];
        let mut max_z = vec![-2_147_483_647i32; texture_count];

        for face in 0..face_count {
            let coord = texture_coords[face];
            if coord == -1 {
                continue;
            }
            let coord = coord as u8 as usize;
            if coord >= texture_count {
                return Err(format!("texture coordinate {coord} is out of range"));
            }
            for index in [indices0[face], indices1[face], indices2[face]] {
                if index < 0 || index as usize >= vertex_count {
                    return Err(format!("texture face references invalid vertex {index}"));
                }
                let index = index as usize;
                let vx = vertices_x[index];
                let vy = vertices_y[index];
                let vz = vertices_z[index];
                min_x[coord] = min_x[coord].min(vx);
                max_x[coord] = max_x[coord].max(vx);
                min_y[coord] = min_y[coord].min(vy);
                max_y[coord] = max_y[coord].max(vy);
                min_z[coord] = min_z[coord].min(vz);
                max_z[coord] = max_z[coord].max(vz);
            }
        }

        for texture in 0..texture_count {
            let mapping_type = texture_render_types[texture] as i32;
            if mapping_type <= 0 {
                continue;
            }
            for (name, len) in [
                ("textureMappingP", texture_mapping_p.len()),
                ("textureMappingM", texture_mapping_m.len()),
                ("textureMappingN", texture_mapping_n.len()),
                ("textureScaleX", texture_scale_x.len()),
                ("textureScaleY", texture_scale_y.len()),
                ("textureScaleZ", texture_scale_z.len()),
                ("textureRotation", texture_rotation.len()),
            ] {
                if texture >= len {
                    return Err(format!("{name} is missing texture {texture}"));
                }
            }
            centers_x[texture] = ((min_x[texture] as i64 + max_x[texture] as i64) / 2) as i32;
            centers_y[texture] = ((min_y[texture] as i64 + max_y[texture] as i64) / 2) as i32;
            centers_z[texture] = ((min_z[texture] as i64 + max_z[texture] as i64) / 2) as i32;

            let (scale_x, scale_y, scale_z) = if mapping_type == 1 {
                let scale_x0 = texture_scale_x[texture];
                let scale_y = 64.0 / texture_scale_y[texture] as f64;
                if scale_x0 == 0 {
                    (1.0, scale_y, 1.0)
                } else if scale_x0 <= 0 {
                    (-(scale_x0 as f64) / 1024.0, scale_y, 1.0)
                } else {
                    (1.0, scale_y, scale_x0 as f64 / 1024.0)
                }
            } else if mapping_type == 2 {
                (
                    64.0 / texture_scale_x[texture] as f64,
                    64.0 / texture_scale_y[texture] as f64,
                    64.0 / texture_scale_z[texture] as f64,
                )
            } else {
                (
                    texture_scale_x[texture] as f64 / 1024.0,
                    texture_scale_y[texture] as f64 / 1024.0,
                    texture_scale_z[texture] as f64 / 1024.0,
                )
            };
            matrices[texture] = Some(build_rotation_scale_matrix(
                texture_mapping_p[texture] as i32,
                texture_mapping_m[texture] as i32,
                texture_mapping_n[texture] as i32,
                texture_rotation[texture] as u8 as i32,
                scale_x,
                scale_y,
                scale_z,
            ));
        }
    }

    let mut uvs = vec![0.0f32; face_count * 6];
    for face in 0..face_count {
        if face_textures[face] == -1 {
            continue;
        }

        let mut coord = if texture_coords.is_empty() {
            -1
        } else {
            texture_coords[face] as i32
        };
        let mut mapping_type = 0i32;
        if coord != -1 {
            coord &= 0xff;
            if coord as usize >= texture_count {
                return Err(format!("texture coordinate {coord} is out of range"));
            }
            mapping_type = texture_render_types[coord as usize] as i32;
        }

        let index0 = indices0[face];
        let index1 = indices1[face];
        let index2 = indices2[face];
        let mut u0 = 0.0f64;
        let mut v0 = 0.0f64;
        let mut u1 = 0.0f64;
        let mut v1 = 0.0f64;
        let mut u2 = 0.0f64;
        let mut v2 = 0.0f64;

        if mapping_type == 0 {
            let mut p = index0;
            let mut m = index1;
            let mut n = index2;
            if coord != -1 {
                let c = coord as usize;
                if c >= texture_mapping_p.len()
                    || c >= texture_mapping_m.len()
                    || c >= texture_mapping_n.len()
                {
                    return Err(format!("texture mapping {coord} is incomplete"));
                }
                p = texture_mapping_p[c] as i32;
                m = texture_mapping_m[c] as i32;
                n = texture_mapping_n[c] as i32;
            }

            let (vx, vy, vz) = vertex(vertices_x, vertices_y, vertices_z, p)?;
            let (mx, my, mz) = vertex(vertices_x, vertices_y, vertices_z, m)?;
            let (nx, ny, nz) = vertex(vertices_x, vertices_y, vertices_z, n)?;
            let (x0, y0, z0) = vertex(vertices_x, vertices_y, vertices_z, index0)?;
            let (x1, y1, z1) = vertex(vertices_x, vertices_y, vertices_z, index1)?;
            let (x2, y2, z2) = vertex(vertices_x, vertices_y, vertices_z, index2)?;

            let f_882 = mx - vx;
            let f_883 = my - vy;
            let f_884 = mz - vz;
            let f_885 = nx - vx;
            let f_886 = ny - vy;
            let f_887 = nz - vz;
            let f_888 = x0 - vx;
            let f_889 = y0 - vy;
            let f_890 = z0 - vz;
            let f_891 = x1 - vx;
            let f_892 = y1 - vy;
            let f_893 = z1 - vz;
            let f_894 = x2 - vx;
            let f_895 = y2 - vy;
            let f_896 = z2 - vz;

            let f_897 = f_883 * f_887 - f_884 * f_886;
            let f_898 = f_884 * f_885 - f_882 * f_887;
            let f_899 = f_882 * f_886 - f_883 * f_885;
            let mut f_900 = f_886 * f_899 - f_887 * f_898;
            let mut f_901 = f_887 * f_897 - f_885 * f_899;
            let mut f_902 = f_885 * f_898 - f_886 * f_897;
            let mut f_903 = 1.0 / (f_900 * f_882 + f_901 * f_883 + f_902 * f_884);

            u0 = (f_900 * f_888 + f_901 * f_889 + f_902 * f_890) * f_903;
            u1 = (f_900 * f_891 + f_901 * f_892 + f_902 * f_893) * f_903;
            u2 = (f_900 * f_894 + f_901 * f_895 + f_902 * f_896) * f_903;

            f_900 = f_883 * f_899 - f_884 * f_898;
            f_901 = f_884 * f_897 - f_882 * f_899;
            f_902 = f_882 * f_898 - f_883 * f_897;
            f_903 = 1.0 / (f_900 * f_885 + f_901 * f_886 + f_902 * f_887);

            v0 = (f_900 * f_888 + f_901 * f_889 + f_902 * f_890) * f_903;
            v1 = (f_900 * f_891 + f_901 * f_892 + f_902 * f_893) * f_903;
            v2 = (f_900 * f_894 + f_901 * f_895 + f_902 * f_896) * f_903;

            if u1 - u0 > 0.99 && u1 - u0 < 1.1 {
                u1 = 1.0;
            }
            if u2 - u1 > 0.99 && u2 - u1 < 1.1 {
                u2 = 1.0;
            }
            if u0 - u2 > 0.99 && u0 - u2 < 1.1 {
                u0 = 1.0;
            }
            if u0 - u1 > 0.99 && u0 - u1 < 1.1 {
                u0 = 1.0;
            }
            if u1 - u2 > 0.99 && u1 - u2 < 1.1 {
                u1 = 1.0;
            }
            if u2 - u0 > 0.99 && u2 - u0 < 1.1 {
                u2 = 1.0;
            }
        } else {
            let c = coord as usize;
            let Some(scales) = matrices.get(c).and_then(|matrix| matrix.as_ref()) else {
                continue;
            };
            for (name, len) in [
                ("textureDirection", texture_direction.len()),
                ("textureSpeed", texture_speed.len()),
            ] {
                if c >= len {
                    return Err(format!("{name} is missing texture {c}"));
                }
            }
            let center = (centers_x[c], centers_y[c], centers_z[c]);
            let direction = texture_direction[c] as i32;
            let speed = texture_speed[c] as f64 / 256.0;
            let vertex0 = vertex(vertices_x, vertices_y, vertices_z, index0)?;
            let vertex1 = vertex(vertices_x, vertices_y, vertices_z, index1)?;
            let vertex2 = vertex(vertices_x, vertices_y, vertices_z, index2)?;

            if mapping_type == 1 {
                if c >= texture_scale_z.len() {
                    return Err(format!("textureScaleZ is missing texture {c}"));
                }
                let scale_z = texture_scale_z[c] as f64 / 1024.0;
                (u0, v0) = project_cylindrical(vertex0, center, scales, scale_z, direction, speed);
                (u1, v1) = project_cylindrical(vertex1, center, scales, scale_z, direction, speed);
                (u2, v2) = project_cylindrical(vertex2, center, scales, scale_z, direction, speed);
                let half = scale_z / 2.0;
                if direction & 1 == 0 {
                    if u1 - u0 > half {
                        u1 -= scale_z;
                    } else if u0 - u1 > half {
                        u1 += scale_z;
                    }
                    if u2 - u0 > half {
                        u2 -= scale_z;
                    } else if u0 - u2 > half {
                        u2 += scale_z;
                    }
                } else {
                    if v1 - v0 > half {
                        v1 -= scale_z;
                    } else if v0 - v1 > half {
                        v1 += scale_z;
                    }
                    if v2 - v0 > half {
                        v2 -= scale_z;
                    } else if v0 - v2 > half {
                        v2 += scale_z;
                    }
                }
            } else if mapping_type == 2 {
                for (name, len) in [
                    ("textureTransU", texture_trans_u.len()),
                    ("textureTransV", texture_trans_v.len()),
                    ("textureScaleX", texture_scale_x.len()),
                    ("textureScaleY", texture_scale_y.len()),
                    ("textureScaleZ", texture_scale_z.len()),
                ] {
                    if c >= len {
                        return Err(format!("{name} is missing texture {c}"));
                    }
                }
                let u_offset = texture_trans_u[c] as f64 / 256.0;
                let v_offset = texture_trans_v[c] as f64 / 256.0;
                let dx1 = vertex1.0 - vertex0.0;
                let dy1 = vertex1.1 - vertex0.1;
                let dz1 = vertex1.2 - vertex0.2;
                let dx2 = vertex2.0 - vertex0.0;
                let dy2 = vertex2.1 - vertex0.1;
                let dz2 = vertex2.2 - vertex0.2;
                let vx = dy1 * dz2 - dy2 * dz1;
                let vy = dz1 * dx2 - dz2 * dx1;
                let vz = dx1 * dy2 - dx2 * dy1;
                let scale_x = 64.0 / texture_scale_x[c] as f64;
                let scale_y = 64.0 / texture_scale_y[c] as f64;
                let scale_z = 64.0 / texture_scale_z[c] as f64;
                let f_829 = (vx * scales[0] as f64 + vy * scales[1] as f64 + vz * scales[2] as f64)
                    / scale_x;
                let f_830 = (vx * scales[3] as f64 + vy * scales[4] as f64 + vz * scales[5] as f64)
                    / scale_y;
                let f_831 = (vx * scales[6] as f64 + vy * scales[7] as f64 + vz * scales[8] as f64)
                    / scale_z;
                let scale_type = dominant_axis(f_829, f_830, f_831);
                (u0, v0) = project_planar(
                    vertex0, center, scale_type, scales, direction, speed, u_offset, v_offset,
                );
                (u1, v1) = project_planar(
                    vertex1, center, scale_type, scales, direction, speed, u_offset, v_offset,
                );
                (u2, v2) = project_planar(
                    vertex2, center, scale_type, scales, direction, speed, u_offset, v_offset,
                );
            } else if mapping_type == 3 {
                (u0, v0) = project_spherical(vertex0, center, scales, direction, speed);
                (u1, v1) = project_spherical(vertex1, center, scales, direction, speed);
                (u2, v2) = project_spherical(vertex2, center, scales, direction, speed);
                if direction & 1 == 0 {
                    if u1 - u0 > 0.5 {
                        u1 -= 1.0;
                    } else if u0 - u1 > 0.0 {
                        u1 += 1.0;
                    }
                    if u2 - u0 > 0.5 {
                        u2 -= 1.0;
                    } else if u0 - u2 > 0.5 {
                        u2 += 1.0;
                    }
                } else {
                    if v1 - v0 > 0.5 {
                        v1 -= 1.0;
                    } else if v0 - v1 > 0.0 {
                        v1 += 1.0;
                    }
                    if v2 - v0 > 0.5 {
                        v2 -= 1.0;
                    } else if v0 - v2 > 0.5 {
                        v2 += 1.0;
                    }
                }
            }
        }

        for (component, value) in [u0, v0, u1, v1, u2, v2].into_iter().enumerate() {
            uvs[face * 6 + component] = value as f32;
        }
    }
    Ok(uvs)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(js_name = compute_model_uvs)]
#[allow(clippy::too_many_arguments)]
pub fn compute_model_uvs_wasm(
    vertices_x: &[i32],
    vertices_y: &[i32],
    vertices_z: &[i32],
    indices0: &[i32],
    indices1: &[i32],
    indices2: &[i32],
    face_textures: &[i16],
    texture_coords: &[i8],
    texture_render_types: &[i8],
    texture_mapping_p: &[i16],
    texture_mapping_m: &[i16],
    texture_mapping_n: &[i16],
    texture_scale_x: &[i32],
    texture_scale_y: &[i32],
    texture_scale_z: &[i32],
    texture_rotation: &[i8],
    texture_direction: &[i8],
    texture_speed: &[i32],
    texture_trans_u: &[i32],
    texture_trans_v: &[i32],
) -> Result<Vec<f32>, wasm_bindgen::JsValue> {
    compute_model_uvs(
        vertices_x,
        vertices_y,
        vertices_z,
        indices0,
        indices1,
        indices2,
        face_textures,
        texture_coords,
        texture_render_types,
        texture_mapping_p,
        texture_mapping_m,
        texture_mapping_n,
        texture_scale_x,
        texture_scale_y,
        texture_scale_z,
        texture_rotation,
        texture_direction,
        texture_speed,
        texture_trans_u,
        texture_trans_v,
    )
    .map_err(|error| wasm_bindgen::JsValue::from_str(&error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_zero_triangle_maps_to_unit_uvs() {
        let uvs = compute_model_uvs(
            &[0, 128, 0],
            &[0, 0, 128],
            &[0, 0, 0],
            &[0],
            &[1],
            &[2],
            &[7],
            &[-1],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(uvs, vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn untextured_face_stays_zeroed() {
        let uvs = compute_model_uvs(
            &[0, 1, 0],
            &[0, 0, 1],
            &[0, 0, 0],
            &[0],
            &[1],
            &[2],
            &[-1],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(uvs, vec![0.0; 6]);
    }
}
