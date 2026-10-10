//! Legacy dynamic-object frame posing for M8.
//!
//! This module ports the pinned `Model.animate`, `Model.transform`, and
//! `SequenceDefinition.transformObjectModel` behavior over renderer-neutral
//! semantic models. Frame/skeleton cache transport is deliberately outside this
//! checkpoint: callers provide canonical frame transforms, and the cached lit
//! base model is always cloned before any pose mutation.

use crate::lighting::ReferenceLitModel;
use std::{array, error::Error, fmt, sync::OnceLock};

const SKIN_GROUP_COUNT: usize = 256;
const TRIG_TABLE_SIZE: usize = 2048;
const TRIG_SCALE: f64 = 65_536.0;
const TRIG_STEP: f64 = 0.003_067_961_5;

/// One skeleton transform slot referenced by a legacy animation frame.
///
/// `transform_type` keeps the source numeric domain because the reference
/// silently ignores transform types it does not handle. Labels are model skin
/// group ids; labels beyond the model's populated group range are skipped by the
/// reference transform path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacySkeletonTransform {
    pub transform_type: u8,
    pub labels: Vec<u16>,
}

/// One frame-authored transform against a skeleton transform slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyFrameTransform {
    pub skeleton_transform: usize,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Canonical legacy frame inputs sufficient for exact object-model posing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyAnimationFrame {
    pub skeleton: Vec<LegacySkeletonTransform>,
    pub transforms: Vec<LegacyFrameTransform>,
}

/// Invalid semantic frame/model state that the reference client would be unable
/// to consume safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyPoseError {
    InvalidVertexSkin { vertex: usize, skin: i32 },
    InvalidFaceSkin { face: usize, skin: i32 },
    SkeletonTransformOutOfRange {
        frame_transform: usize,
        skeleton_transform: usize,
        skeleton_len: usize,
    },
    PivotVertexCountOverflow,
}

impl fmt::Display for LegacyPoseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidVertexSkin { vertex, skin } => write!(
                formatter,
                "vertex {vertex} has animation skin {skin}, outside reference group range 0..=255"
            ),
            Self::InvalidFaceSkin { face, skin } => write!(
                formatter,
                "face {face} has alpha-animation skin {skin}, outside reference group range 0..=255"
            ),
            Self::SkeletonTransformOutOfRange {
                frame_transform,
                skeleton_transform,
                skeleton_len,
            } => write!(
                formatter,
                "frame transform {frame_transform} references skeleton slot {skeleton_transform}, but skeleton length is {skeleton_len}"
            ),
            Self::PivotVertexCountOverflow => {
                formatter.write_str("animation pivot vertex count exceeds reference int range")
            }
        }
    }
}

impl Error for LegacyPoseError {}

#[derive(Debug)]
struct AnimationGroups {
    vertex_groups: Vec<Vec<usize>>,
    face_alpha_groups: Option<Vec<Vec<usize>>>,
}

#[derive(Debug, Clone, Copy, Default)]
struct TransformPivot {
    x: i32,
    y: i32,
    z: i32,
}

struct TrigTables {
    sine: [i32; TRIG_TABLE_SIZE],
    cosine: [i32; TRIG_TABLE_SIZE],
}

/// Clone a cached lit base model, apply one legacy frame in object-animation
/// orientation space, and return the private posed instance.
///
/// The orientation wrapper exactly follows `transformObjectModel`: orientation
/// 1 pre-rotates 270 degrees and post-rotates 90 degrees; orientation 2 uses
/// 180/180; orientation 3 uses 90/270. Orientation is masked with `& 3`.
/// The input model is never mutated.
pub fn pose_legacy_object_model(
    base: &ReferenceLitModel,
    frame: &LegacyAnimationFrame,
    orientation: u8,
) -> Result<ReferenceLitModel, LegacyPoseError> {
    let mut working = base.clone();
    let orientation = orientation & 3;

    rotate_into_animation_space(&mut working, orientation);
    apply_legacy_frame(&mut working, frame)?;
    rotate_from_animation_space(&mut working, orientation);

    Ok(working)
}

fn apply_legacy_frame(
    model: &mut ReferenceLitModel,
    frame: &LegacyAnimationFrame,
) -> Result<(), LegacyPoseError> {
    // `Model.animate` is guarded by `vertexLabels != null`. Preserve that
    // behavior even for frames containing only alpha transforms.
    let Some(vertex_skins) = model.vertex_skins.as_deref() else {
        return Ok(());
    };

    let groups = build_animation_groups(
        vertex_skins,
        model.face_skins.as_deref(),
        model.vertices.len(),
        model.faces.len(),
    )?;
    let mut pivot = TransformPivot::default();

    for (frame_transform_index, authored) in frame.transforms.iter().copied().enumerate() {
        let Some(skeleton) = frame.skeleton.get(authored.skeleton_transform) else {
            return Err(LegacyPoseError::SkeletonTransformOutOfRange {
                frame_transform: frame_transform_index,
                skeleton_transform: authored.skeleton_transform,
                skeleton_len: frame.skeleton.len(),
            });
        };

        apply_transform(
            model,
            &groups,
            &mut pivot,
            skeleton.transform_type,
            &skeleton.labels,
            authored.x,
            authored.y,
            authored.z,
        )?;
    }

    Ok(())
}

fn build_animation_groups(
    vertex_skins: &[i32],
    face_skins: Option<&[i32]>,
    vertex_count: usize,
    face_count: usize,
) -> Result<AnimationGroups, LegacyPoseError> {
    debug_assert_eq!(vertex_skins.len(), vertex_count);
    let mut vertex_groups = vec![Vec::new(); SKIN_GROUP_COUNT];
    for (vertex, skin) in vertex_skins.iter().copied().enumerate() {
        let Ok(group) = usize::try_from(skin) else {
            return Err(LegacyPoseError::InvalidVertexSkin { vertex, skin });
        };
        if group >= SKIN_GROUP_COUNT {
            return Err(LegacyPoseError::InvalidVertexSkin { vertex, skin });
        }
        vertex_groups[group].push(vertex);
    }
    trim_empty_tail(&mut vertex_groups);

    let face_alpha_groups = match face_skins {
        None => None,
        Some(face_skins) => {
            debug_assert_eq!(face_skins.len(), face_count);
            let mut groups = vec![Vec::new(); SKIN_GROUP_COUNT];
            for (face, skin) in face_skins.iter().copied().enumerate() {
                let Ok(group) = usize::try_from(skin) else {
                    return Err(LegacyPoseError::InvalidFaceSkin { face, skin });
                };
                if group >= SKIN_GROUP_COUNT {
                    return Err(LegacyPoseError::InvalidFaceSkin { face, skin });
                }
                groups[group].push(face);
            }
            trim_empty_tail(&mut groups);
            Some(groups)
        }
    };

    Ok(AnimationGroups {
        vertex_groups,
        face_alpha_groups,
    })
}

fn trim_empty_tail(groups: &mut Vec<Vec<usize>>) {
    while groups.last().is_some_and(Vec::is_empty) {
        groups.pop();
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_transform(
    model: &mut ReferenceLitModel,
    groups: &AnimationGroups,
    pivot: &mut TransformPivot,
    transform_type: u8,
    labels: &[u16],
    x: i32,
    y: i32,
    z: i32,
) -> Result<(), LegacyPoseError> {
    match transform_type {
        0 => set_pivot(model, groups, pivot, labels, x, y, z),
        1 => translate_groups(model, groups, labels, x, y, z),
        2 => rotate_groups(model, groups, *pivot, labels, x, y, z),
        3 => scale_groups(model, groups, *pivot, labels, x, y, z),
        5 => alpha_groups(model, groups, labels, x),
        _ => Ok(()),
    }
}

fn set_pivot(
    model: &ReferenceLitModel,
    groups: &AnimationGroups,
    pivot: &mut TransformPivot,
    labels: &[u16],
    x: i32,
    y: i32,
    z: i32,
) -> Result<(), LegacyPoseError> {
    let mut sum_x = 0_i32;
    let mut sum_y = 0_i32;
    let mut sum_z = 0_i32;
    let mut count = 0_usize;

    for &label in labels {
        let label = usize::from(label);
        let Some(vertices) = groups.vertex_groups.get(label) else {
            continue;
        };
        for &vertex in vertices {
            let point = model.vertices[vertex];
            sum_x = sum_x.wrapping_add(point.x);
            sum_y = sum_y.wrapping_add(point.y);
            sum_z = sum_z.wrapping_add(point.z);
            count = count.wrapping_add(1);
        }
    }

    if count == 0 {
        *pivot = TransformPivot { x, y, z };
        return Ok(());
    }

    let count = i32::try_from(count).map_err(|_| LegacyPoseError::PivotVertexCountOverflow)?;
    *pivot = TransformPivot {
        x: x.wrapping_add(sum_x / count),
        y: y.wrapping_add(sum_y / count),
        z: z.wrapping_add(sum_z / count),
    };
    Ok(())
}

fn translate_groups(
    model: &mut ReferenceLitModel,
    groups: &AnimationGroups,
    labels: &[u16],
    x: i32,
    y: i32,
    z: i32,
) -> Result<(), LegacyPoseError> {
    for_each_vertex(groups, labels, |vertex| {
        let point = &mut model.vertices[vertex];
        point.x = point.x.wrapping_add(x);
        point.y = point.y.wrapping_add(y);
        point.z = point.z.wrapping_add(z);
    });
    Ok(())
}

fn rotate_groups(
    model: &mut ReferenceLitModel,
    groups: &AnimationGroups,
    pivot: TransformPivot,
    labels: &[u16],
    x: i32,
    y: i32,
    z: i32,
) -> Result<(), LegacyPoseError> {
    let angle_z = ((z & 255) * 8) as usize;
    let angle_x = ((x & 255) * 8) as usize;
    let angle_y = ((y & 255) * 8) as usize;
    let tables = trig_tables();

    for_each_vertex(groups, labels, |vertex| {
        let point = &mut model.vertices[vertex];
        point.x = point.x.wrapping_sub(pivot.x);
        point.y = point.y.wrapping_sub(pivot.y);
        point.z = point.z.wrapping_sub(pivot.z);

        if angle_z != 0 {
            let sine = tables.sine[angle_z];
            let cosine = tables.cosine[angle_z];
            let new_x = sine
                .wrapping_mul(point.y)
                .wrapping_add(cosine.wrapping_mul(point.x))
                >> 16;
            point.y = cosine
                .wrapping_mul(point.y)
                .wrapping_sub(sine.wrapping_mul(point.x))
                >> 16;
            point.x = new_x;
        }

        if angle_x != 0 {
            let sine = tables.sine[angle_x];
            let cosine = tables.cosine[angle_x];
            let new_y = cosine
                .wrapping_mul(point.y)
                .wrapping_sub(sine.wrapping_mul(point.z))
                >> 16;
            point.z = sine
                .wrapping_mul(point.y)
                .wrapping_add(cosine.wrapping_mul(point.z))
                >> 16;
            point.y = new_y;
        }

        if angle_y != 0 {
            let sine = tables.sine[angle_y];
            let cosine = tables.cosine[angle_y];
            let new_x = sine
                .wrapping_mul(point.z)
                .wrapping_add(cosine.wrapping_mul(point.x))
                >> 16;
            point.z = cosine
                .wrapping_mul(point.z)
                .wrapping_sub(sine.wrapping_mul(point.x))
                >> 16;
            point.x = new_x;
        }

        point.x = point.x.wrapping_add(pivot.x);
        point.y = point.y.wrapping_add(pivot.y);
        point.z = point.z.wrapping_add(pivot.z);
    });
    Ok(())
}

fn scale_groups(
    model: &mut ReferenceLitModel,
    groups: &AnimationGroups,
    pivot: TransformPivot,
    labels: &[u16],
    x: i32,
    y: i32,
    z: i32,
) -> Result<(), LegacyPoseError> {
    for_each_vertex(groups, labels, |vertex| {
        let point = &mut model.vertices[vertex];
        point.x = point.x.wrapping_sub(pivot.x);
        point.y = point.y.wrapping_sub(pivot.y);
        point.z = point.z.wrapping_sub(pivot.z);
        point.x = point.x.wrapping_mul(x) / 128;
        point.y = point.y.wrapping_mul(y) / 128;
        point.z = point.z.wrapping_mul(z) / 128;
        point.x = point.x.wrapping_add(pivot.x);
        point.y = point.y.wrapping_add(pivot.y);
        point.z = point.z.wrapping_add(pivot.z);
    });
    Ok(())
}

fn alpha_groups(
    model: &mut ReferenceLitModel,
    groups: &AnimationGroups,
    labels: &[u16],
    x: i32,
) -> Result<(), LegacyPoseError> {
    let (Some(face_groups), Some(alphas)) =
        (groups.face_alpha_groups.as_ref(), model.face_alphas.as_mut())
    else {
        return Ok(());
    };

    for &label in labels {
        let label = usize::from(label);
        let Some(faces) = face_groups.get(label) else {
            continue;
        };
        for &face in faces {
            let source = i32::from(alphas[face] as u8);
            let adjusted = source.wrapping_add(x.wrapping_mul(8)).clamp(0, 255);
            alphas[face] = adjusted as u8 as i8;
        }
    }
    Ok(())
}

fn for_each_vertex(mut groups: &AnimationGroups, labels: &[u16], mut apply: impl FnMut(usize)) {
    for &label in labels {
        let label = usize::from(label);
        let Some(vertices) = groups.vertex_groups.get(label) else {
            continue;
        };
        for &vertex in vertices {
            apply(vertex);
        }
    }
}

fn rotate_into_animation_space(model: &mut ReferenceLitModel, orientation: u8) {
    match orientation {
        1 => rotate_y_270_ccw(model),
        2 => rotate_y_180(model),
        3 => rotate_y_90_ccw(model),
        _ => {}
    }
}

fn rotate_from_animation_space(model: &mut ReferenceLitModel, orientation: u8) {
    match orientation {
        1 => rotate_y_90_ccw(model),
        2 => rotate_y_180(model),
        3 => rotate_y_270_ccw(model),
        _ => {}
    }
}

fn rotate_y_90_ccw(model: &mut ReferenceLitModel) {
    for point in &mut model.vertices {
        let x = point.x;
        point.x = point.z;
        point.z = x.wrapping_neg();
    }
}

fn rotate_y_180(model: &mut ReferenceLitModel) {
    for point in &mut model.vertices {
        point.x = point.x.wrapping_neg();
        point.z = point.z.wrapping_neg();
    }
}

fn rotate_y_270_ccw(model: &mut ReferenceLitModel) {
    for point in &mut model.vertices {
        let z = point.z;
        point.z = point.x;
        point.x = z.wrapping_neg();
    }
}

fn trig_tables() -> &'static TrigTables {
    static TABLES: OnceLock<TrigTables> = OnceLock::new();
    TABLES.get_or_init(|| TrigTables {
        sine: array::from_fn(|index| {
            (TRIG_SCALE * ((index as f64) * TRIG_STEP).sin()) as i32
        }),
        cosine: array::from_fn(|index| {
            (TRIG_SCALE * ((index as f64) * TRIG_STEP).cos()) as i32
        }),
    })
}
