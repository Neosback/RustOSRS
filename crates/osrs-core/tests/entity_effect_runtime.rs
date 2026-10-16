use osrs_core::definitions::{DefinitionIdentity, SequenceDefinition};
use osrs_core::entity::{EntityScale, SpotAnimationDefinition};
use osrs_core::entity_effect::{
    ActorSpotAnimations, EffectPoint, EffectRuntimeError, GraphicsObjectState, ProjectileState,
    RuntimeSequence,
};
use osrs_core::ids::{FrameId, ModelId, SequenceId, SpotAnimationId};
use osrs_core::provenance::{CacheFingerprint, ProfileDigest, TargetProvenance};
use std::error::Error;

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str = "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

fn provenance() -> Result<TargetProvenance, Box<dyn Error>> {
    Ok(TargetProvenance::new(
        "osrs-live-241-2026-09-30-openrs2-2727",
        ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
        CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
        1,
    )?)
}

fn spot(sequence: Option<SequenceId>) -> Result<SpotAnimationDefinition, Box<dyn Error>> {
    Ok(SpotAnimationDefinition {
        identity: DefinitionIdentity::new(SpotAnimationId::new(77), provenance()?),
        model: Some(ModelId::new(12)),
        sequence,
        scale: EntityScale::IDENTITY,
        orientation: 0,
        ambient: 0,
        contrast: 0,
        recolors: Vec::new(),
        retextures: Vec::new(),
    })
}

fn one_shot_sequence() -> Result<SequenceDefinition, Box<dyn Error>> {
    Ok(SequenceDefinition {
        identity: DefinitionIdentity::new(SequenceId::new(9), provenance()?),
        frame_ids: vec![FrameId::new(1), FrameId::new(2)],
        frame_delays: vec![1, 1],
        frame_step: None,
        interleave: None,
        max_loops: 99,
        precedence_animating: 0,
        priority: 0,
        restart_mode: 0,
        left_hand_item: None,
        right_hand_item: None,
        skeletal_animation: None,
        skeletal_range: None,
        skeletal_mask: None,
    })
}

#[test]
fn actor_spot_update_replaces_only_matching_key_and_preserves_delay() -> Result<(), Box<dyn Error>>
{
    let definition = spot(Some(SequenceId::new(9)))?;
    let mut spots = ActorSpotAnimations::default();
    spots.update(3, Some(&definition), 15, 100, 7);
    spots.update(4, Some(&definition), 20, 100, 0);

    assert_eq!(spots.len(), 2);
    assert!(spots.get(3).is_some_and(|spot| !spot.is_started(106)));
    assert!(spots.get(3).is_some_and(|spot| spot.is_started(107)));

    let replaced = spots.update(3, None, 0, 200, 0);
    assert_eq!(replaced.map(|spot| spot.height), Some(15));
    assert!(spots.get(3).is_none());
    assert!(spots.get(4).is_some());
    Ok(())
}

#[test]
fn graphics_object_finishes_when_one_shot_sequence_completes() -> Result<(), Box<dyn Error>> {
    let definition = spot(Some(SequenceId::new(9)))?;
    let sequence = one_shot_sequence()?;
    let mut graphic = GraphicsObjectState::new(&definition, 2, 10, 20, 30, 4, 100);

    assert!(!graphic.is_started(103));
    assert!(graphic.is_started(104));
    let report = graphic.advance(&sequence, 3)?;
    assert!(report.wrapped);
    assert!(graphic.is_finished());
    Ok(())
}

#[test]
fn graphics_object_without_sequence_is_finished_immediately() -> Result<(), Box<dyn Error>> {
    let definition = spot(None)?;
    let graphic = GraphicsObjectState::new(&definition, 0, 0, 0, 0, 0, 0);
    assert!(graphic.is_finished());
    Ok(())
}

#[test]
fn projectile_matches_source_flat_trajectory_and_orientation() -> Result<(), Box<dyn Error>> {
    let definition = spot(None)?;
    let mut projectile = ProjectileState::new(&definition, 0, 9, 0, 0);
    projectile.update_destination(
        0,
        1,
        EffectPoint::new(0.0, 0.0, 0.0),
        EffectPoint::new(100.0, 0.0, 0.0),
    )?;

    let Some(position) = projectile.position else {
        panic!("projectile did not enter its moving state");
    };
    assert!((position.x - 10.0).abs() < 1.0e-12);
    assert!(position.y.abs() < 1.0e-12);
    assert!(position.z.abs() < 1.0e-12);
    assert!((projectile.velocity_x - 10.0).abs() < 1.0e-12);
    assert_eq!(projectile.yaw, 1535);
    assert_eq!(projectile.pitch, 0);
    Ok(())
}

#[test]
fn runtime_sequence_rejects_the_wrong_definition() -> Result<(), Box<dyn Error>> {
    let sequence = one_shot_sequence()?;
    let mut runtime = RuntimeSequence::new(SequenceId::new(10));
    assert!(matches!(
        runtime.advance(&sequence, 1),
        Err(EffectRuntimeError::SequenceMismatch { .. })
    ));
    Ok(())
}
