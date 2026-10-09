//! Exact semantic base-normal generation for ModelData-compatible working models.
//!
//! M7 owns normal and lighting semantics. This first slice mirrors the pinned
//! `ModelData.calculateVertexNormals()` integer algorithm without mutating
//! topology, shared source assets, or renderer state.

use crate::{
    coords::ModelPoint,
    model::{FaceNormal, ModelNormals, Triangle, VertexNormal, WorkingModel},
};

/// Calculate the pinned reference base normals for one semantic working model.
///
/// This operation is pure with respect to the model. Later M7 lifecycle stages
/// may store the returned state on a scene-local working instance before normal
/// reconciliation and final lighting.
pub fn calculate_base_normals(model: &WorkingModel) -> ModelNormals {
    calculate_base_normals_from_parts(model.vertices(), model.faces(), model.face_render_types())
}

fn calculate_base_normals_from_parts(
    vertices: &[ModelPoint],
    faces: &[Triangle],
    face_render_types: Option<&[i8]>,
) -> ModelNormals {
    let mut vertex_normals = vec![VertexNormal::default(); vertices.len()];
    let mut face_normals: Option<Vec<Option<FaceNormal>>> = None;

    for (face_index, face) in faces.iter().copied().enumerate() {
        let a = vertices[face.a.get() as usize];
        let b = vertices[face.b.get() as usize];
        let c = vertices[face.c.get() as usize];
        let normal = reference_face_normal(a, b, c);
        let render_type = face_render_types.map_or(0, |types| types[face_index]);

        if render_type == 0 {
            for vertex_index in [face.a, face.b, face.c] {
                let target = &mut vertex_normals[vertex_index.get() as usize];
                target.x = target.x.wrapping_add(normal.x);
                target.y = target.y.wrapping_add(normal.y);
                target.z = target.z.wrapping_add(normal.z);
                target.magnitude = target.magnitude.wrapping_add(1);
            }
        } else if render_type == 1 {
            let normals = face_normals.get_or_insert_with(|| vec![None; faces.len()]);
            normals[face_index] = Some(normal);
        }
    }

    ModelNormals {
        base_vertex_normals: vertex_normals,
        face_normals,
        merged_vertex_normals: None,
    }
}

fn reference_face_normal(a: ModelPoint, b: ModelPoint, c: ModelPoint) -> FaceNormal {
    let edge1_x = b.x.wrapping_sub(a.x);
    let edge1_y = b.y.wrapping_sub(a.y);
    let edge1_z = b.z.wrapping_sub(a.z);
    let edge2_x = c.x.wrapping_sub(a.x);
    let edge2_y = c.y.wrapping_sub(a.y);
    let edge2_z = c.z.wrapping_sub(a.z);

    let mut normal_x = edge1_y
        .wrapping_mul(edge2_z)
        .wrapping_sub(edge2_y.wrapping_mul(edge1_z));
    let mut normal_y = edge1_z
        .wrapping_mul(edge2_x)
        .wrapping_sub(edge2_z.wrapping_mul(edge1_x));
    let mut normal_z = edge1_x
        .wrapping_mul(edge2_y)
        .wrapping_sub(edge2_x.wrapping_mul(edge1_y));

    while normal_x > 8192
        || normal_y > 8192
        || normal_z > 8192
        || normal_x < -8192
        || normal_y < -8192
        || normal_z < -8192
    {
        normal_x >>= 1;
        normal_y >>= 1;
        normal_z >>= 1;
    }

    let squared_length = normal_x
        .wrapping_mul(normal_x)
        .wrapping_add(normal_y.wrapping_mul(normal_y))
        .wrapping_add(normal_z.wrapping_mul(normal_z));
    let mut length = f64::from(squared_length).sqrt() as i32;
    if length <= 0 {
        length = 1;
    }

    FaceNormal {
        x: normal_x.wrapping_mul(256) / length,
        y: normal_y.wrapping_mul(256) / length,
        z: normal_z.wrapping_mul(256) / length,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIANGLE_VERTICES: [ModelPoint; 3] = [
        ModelPoint::new(0, 0, 0),
        ModelPoint::new(128, 0, 0),
        ModelPoint::new(0, 0, 128),
    ];

    #[test]
    fn smooth_triangle_accumulates_exact_reference_vertex_normals() {
        let normals = calculate_base_normals_from_parts(
            &TRIANGLE_VERTICES,
            &[Triangle::new(0, 1, 2)],
            None,
        );
        assert_eq!(
            normals.base_vertex_normals,
            vec![
                VertexNormal {
                    x: 0,
                    y: -256,
                    z: 0,
                    magnitude: 1,
                };
                3
            ]
        );
        assert_eq!(normals.face_normals, None);
        assert_eq!(normals.merged_vertex_normals, None);
    }

    #[test]
    fn flat_triangle_preserves_vertex_zeroes_and_authors_face_normal() {
        let normals = calculate_base_normals_from_parts(
            &TRIANGLE_VERTICES,
            &[Triangle::new(0, 1, 2)],
            Some(&[1]),
        );
        assert_eq!(
            normals.base_vertex_normals,
            vec![VertexNormal::default(); 3]
        );
        assert_eq!(
            normals.face_normals,
            Some(vec![Some(FaceNormal {
                x: 0,
                y: -256,
                z: 0,
            })])
        );
    }

    #[test]
    fn two_triangle_quad_accumulates_shared_smooth_vertices() {
        let vertices = [
            ModelPoint::new(0, 0, 0),
            ModelPoint::new(128, 0, 0),
            ModelPoint::new(128, 0, 128),
            ModelPoint::new(0, 0, 128),
        ];
        let normals = calculate_base_normals_from_parts(
            &vertices,
            &[Triangle::new(0, 1, 2), Triangle::new(0, 2, 3)],
            None,
        );
        assert_eq!(normals.base_vertex_normals[0].magnitude, 2);
        assert_eq!(normals.base_vertex_normals[1].magnitude, 1);
        assert_eq!(normals.base_vertex_normals[2].magnitude, 2);
        assert_eq!(normals.base_vertex_normals[3].magnitude, 1);
        assert!(normals
            .base_vertex_normals
            .iter()
            .all(|normal| normal.x == 0 && normal.y < 0 && normal.z == 0));
    }

    #[test]
    fn winding_reversal_reverses_the_exact_normal_direction() {
        let forward = calculate_base_normals_from_parts(
            &TRIANGLE_VERTICES,
            &[Triangle::new(0, 1, 2)],
            None,
        );
        let reversed = calculate_base_normals_from_parts(
            &TRIANGLE_VERTICES,
            &[Triangle::new(0, 2, 1)],
            None,
        );
        assert_eq!(forward.base_vertex_normals[0].y, -256);
        assert_eq!(reversed.base_vertex_normals[0].y, 256);
    }

    #[test]
    fn non_smooth_non_flat_render_types_do_not_contribute_base_normals() {
        let normals = calculate_base_normals_from_parts(
            &TRIANGLE_VERTICES,
            &[Triangle::new(0, 1, 2)],
            Some(&[2]),
        );
        assert_eq!(normals.base_vertex_normals, vec![VertexNormal::default(); 3]);
        assert_eq!(normals.face_normals, None);
    }
}
