use super::AssembledModel;
use crate::definitions::{LocType, ModelScale, ModelTranslation, ObjectDefinition};
use crate::ids::TextureId;

// Pinned Rasterizer3D table values at JAU index 256. The reference table is
// generated as `(int)(65536.0 * sin/cos(index * 0.0030679615D))`.
const SINE_256: i32 = 46_340;
const COSINE_256: i32 = 46_340;
const MODEL_SCALE_IDENTITY: i32 = 128;

/// Apply the exact ModelData object-instance transform pipeline.
///
/// Order is semantic and must not be rearranged:
/// 1. loc type 4 with orientation > 3: 256-JAU rotation, then `(45, 0, -45)`;
/// 2. mask orientation to `0..=3`, then apply the ordinary quarter turn;
/// 3. recolor pairs in authored order;
/// 4. retexture pairs in authored order;
/// 5. resize when the definition scale is not identity;
/// 6. apply the final definition translation.
///
/// Geometry arithmetic uses wrapping `i32` operations where Java `int` would
/// wrap. Right shifts remain arithmetic and resize division truncates toward
/// zero, matching the pinned client behavior.
pub fn apply_object_model_instance_transforms(
    model: &mut AssembledModel,
    definition: &ObjectDefinition,
    requested_type: LocType,
    orientation: u8,
) {
    if requested_type.get() == 4 && orientation > 3 {
        rotate_jau_256(&mut model.vertices);
        translate_vertices(&mut model.vertices, 45, 0, -45);
    }

    match orientation & 3 {
        0 => {}
        1 => rotate_quarter_turn_1(&mut model.vertices),
        2 => rotate_quarter_turn_2(&mut model.vertices),
        3 => rotate_quarter_turn_3(&mut model.vertices),
        _ => unreachable!("orientation & 3 is always in 0..=3"),
    }

    for pair in &definition.recolors {
        for color in &mut model.face_colors {
            if *color == pair.from {
                *color = pair.to;
            }
        }
    }

    if let Some(textures) = model.face_textures.as_mut() {
        for pair in &definition.retextures {
            for texture in textures.iter_mut() {
                if texture_matches_raw_short(*texture, pair.from) {
                    *texture = texture_from_raw_short(pair.to);
                }
            }
        }
    }

    if definition.scale != ModelScale::IDENTITY {
        resize_vertices(&mut model.vertices, definition.scale);
    }

    if definition.translation != ModelTranslation::ZERO {
        translate_vertices(
            &mut model.vertices,
            definition.translation.x,
            definition.translation.y,
            definition.translation.z,
        );
    }
}

fn rotate_jau_256(vertices: &mut [crate::coords::ModelPoint]) {
    for vertex in vertices {
        let old_x = vertex.x;
        let old_z = vertex.z;
        let rotated_x = SINE_256
            .wrapping_mul(old_z)
            .wrapping_add(COSINE_256.wrapping_mul(old_x))
            >> 16;
        let rotated_z = COSINE_256
            .wrapping_mul(old_z)
            .wrapping_sub(SINE_256.wrapping_mul(old_x))
            >> 16;
        vertex.x = rotated_x;
        vertex.z = rotated_z;
    }
}

fn rotate_quarter_turn_1(vertices: &mut [crate::coords::ModelPoint]) {
    for vertex in vertices {
        let old_x = vertex.x;
        vertex.x = vertex.z;
        vertex.z = old_x.wrapping_neg();
    }
}

fn rotate_quarter_turn_2(vertices: &mut [crate::coords::ModelPoint]) {
    for vertex in vertices {
        vertex.x = vertex.x.wrapping_neg();
        vertex.z = vertex.z.wrapping_neg();
    }
}

fn rotate_quarter_turn_3(vertices: &mut [crate::coords::ModelPoint]) {
    for vertex in vertices {
        let old_z = vertex.z;
        vertex.z = vertex.x;
        vertex.x = old_z.wrapping_neg();
    }
}

fn resize_vertices(vertices: &mut [crate::coords::ModelPoint], scale: ModelScale) {
    let scale_x = i32::from(scale.x);
    let scale_y = i32::from(scale.y);
    let scale_z = i32::from(scale.z);
    for vertex in vertices {
        vertex.x = vertex.x.wrapping_mul(scale_x) / MODEL_SCALE_IDENTITY;
        vertex.y = scale_y.wrapping_mul(vertex.y) / MODEL_SCALE_IDENTITY;
        vertex.z = scale_z.wrapping_mul(vertex.z) / MODEL_SCALE_IDENTITY;
    }
}

fn translate_vertices(
    vertices: &mut [crate::coords::ModelPoint],
    x: i32,
    y: i32,
    z: i32,
) {
    for vertex in vertices {
        vertex.x = vertex.x.wrapping_add(x);
        vertex.y = vertex.y.wrapping_add(y);
        vertex.z = vertex.z.wrapping_add(z);
    }
}

fn texture_matches_raw_short(texture: Option<TextureId>, raw: u16) -> bool {
    match texture {
        Some(texture) => texture.get() == u32::from(raw),
        None => raw == u16::MAX,
    }
}

fn texture_from_raw_short(raw: u16) -> Option<TextureId> {
    if raw == u16::MAX {
        None
    } else {
        Some(TextureId::new(u32::from(raw)))
    }
}
