//! NPC/player legacy actor posing, without dynamic-loc orientation transforms.
use crate::{
    animation_pose::{LegacyAnimationFrame, LegacyPoseError, apply_legacy_frame_filtered},
    entity::EntityScale,
    lighting::ReferenceLitModel,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorPoseError {
    Legacy(LegacyPoseError),
    IncompatibleSkeletons,
}
impl std::fmt::Display for ActorPoseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Legacy(e) => e.fmt(f),
            Self::IncompatibleSkeletons => {
                f.write_str("dual actor frames use incompatible skeletons")
            }
        }
    }
}
impl std::error::Error for ActorPoseError {}
impl From<LegacyPoseError> for ActorPoseError {
    fn from(value: LegacyPoseError) -> Self {
        Self::Legacy(value)
    }
}

/// Make a private posed instance. Action without an interleave mask takes
/// precedence; dual legacy animation applies masked transforms with fresh pivots.
pub fn pose_legacy_actor(
    base: &ReferenceLitModel,
    action: Option<&LegacyAnimationFrame>,
    pose: Option<&LegacyAnimationFrame>,
    interleave: Option<&[u16]>,
    scale: EntityScale,
) -> Result<ReferenceLitModel, ActorPoseError> {
    let mut result = base.clone();
    match (action, pose, interleave) {
        (Some(a), Some(p), Some(mask)) => {
            if a.skeleton != p.skeleton {
                return Err(ActorPoseError::IncompatibleSkeletons);
            }
            apply_legacy_frame_filtered(&mut result, a, |slot, kind| {
                kind == 0 || !mask.iter().any(|v| usize::from(*v) == slot)
            })?;
            apply_legacy_frame_filtered(&mut result, p, |slot, kind| {
                kind == 0 || mask.iter().any(|v| usize::from(*v) == slot)
            })?;
        }
        (Some(frame), _, _) | (_, Some(frame), _) => {
            apply_legacy_frame_filtered(&mut result, frame, |_, _| true)?
        }
        _ => {}
    }
    if scale != EntityScale::IDENTITY {
        for vertex in &mut result.vertices {
            vertex.x = vertex.x.wrapping_mul(i32::from(scale.horizontal)) / 128;
            vertex.y = vertex.y.wrapping_mul(i32::from(scale.vertical)) / 128;
            vertex.z = vertex.z.wrapping_mul(i32::from(scale.horizontal)) / 128;
        }
    }
    Ok(result)
}
