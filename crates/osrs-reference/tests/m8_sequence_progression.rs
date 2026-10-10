use osrs_core::{
    animation::{SequenceAdvanceError, SequencePlaybackState, advance_dynamic_sequence},
    definitions::{DefinitionIdentity, SequenceDefinition, SequenceRange},
    ids::{FrameId, SequenceId, SkeletalAnimationId},
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};
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

fn legacy_sequence(
    id: u32,
    frame_delays: &[u16],
    frame_step: Option<u16>,
    max_loops: u16,
) -> Result<SequenceDefinition, Box<dyn Error>> {
    Ok(SequenceDefinition {
        identity: DefinitionIdentity::new(SequenceId::new(id), provenance()?),
        frame_ids: (0..frame_delays.len())
            .map(|index| FrameId::new(0x0012_0000 | index as u32))
            .collect(),
        frame_delays: frame_delays.to_vec(),
        frame_step,
        interleave: None,
        max_loops,
        precedence_animating: 0,
        priority: 0,
        reply_mode: 2,
        left_hand_item: None,
        right_hand_item: None,
        skeletal_animation: None,
        skeletal_range: None,
        skeletal_mask: None,
    })
}

fn skeletal_sequence(
    id: u32,
    range: SequenceRange,
    frame_step: Option<u16>,
    max_loops: u16,
) -> Result<SequenceDefinition, Box<dyn Error>> {
    Ok(SequenceDefinition {
        identity: DefinitionIdentity::new(SequenceId::new(id), provenance()?),
        frame_ids: Vec::new(),
        frame_delays: Vec::new(),
        frame_step,
        interleave: None,
        max_loops,
        precedence_animating: 0,
        priority: 0,
        reply_mode: 2,
        left_hand_item: None,
        right_hand_item: None,
        skeletal_animation: Some(SkeletalAnimationId::new(900 + id)),
        skeletal_range: Some(range),
        skeletal_mask: None,
    })
}

#[test]
fn legacy_frame_boundary_uses_reference_strict_greater_than_delay() -> Result<(), Box<dyn Error>> {
    let sequence = legacy_sequence(1, &[2, 3], Some(2), 99)?;
    let mut state = SequencePlaybackState::new();

    advance_dynamic_sequence(&sequence, &mut state, 1)?;
    assert_eq!(state.frame(), Some(0));
    assert_eq!(state.frame_cycle(), 1);

    advance_dynamic_sequence(&sequence, &mut state, 1)?;
    assert_eq!(state.frame(), Some(0));
    assert_eq!(state.frame_cycle(), 2);

    let report = advance_dynamic_sequence(&sequence, &mut state, 1)?;
    assert!(report.frame_advanced);
    assert!(!report.wrapped);
    assert_eq!(state.frame(), Some(1));
    assert_eq!(state.frame_cycle(), 1);
    Ok(())
}

#[test]
fn legacy_loopback_subtracts_frame_step_instead_of_restarting_at_zero() -> Result<(), Box<dyn Error>>
{
    let sequence = legacy_sequence(2, &[1, 1, 1, 1], Some(2), 99)?;
    let mut state = SequencePlaybackState::new();

    for _ in 0..4 {
        advance_dynamic_sequence(&sequence, &mut state, 1)?;
    }
    assert_eq!(state.frame(), Some(3));

    let report = advance_dynamic_sequence(&sequence, &mut state, 1)?;
    assert!(report.wrapped);
    assert!(!report.deactivated);
    assert_eq!(state.frame(), Some(2));
    assert_eq!(state.completed_loops(), 1);
    Ok(())
}

#[test]
fn legacy_sequence_without_frame_step_resets_after_first_wrap() -> Result<(), Box<dyn Error>> {
    let sequence = legacy_sequence(3, &[1, 1], None, 99)?;
    let mut state = SequencePlaybackState::new();

    advance_dynamic_sequence(&sequence, &mut state, 1)?;
    advance_dynamic_sequence(&sequence, &mut state, 1)?;
    let report = advance_dynamic_sequence(&sequence, &mut state, 1)?;

    assert!(report.wrapped);
    assert!(report.deactivated);
    assert!(!state.is_active());
    assert_eq!(state.frame(), None);
    assert_eq!(state.completed_loops(), 1);
    Ok(())
}

#[test]
fn legacy_large_delta_uses_reference_total_delay_reduction() -> Result<(), Box<dyn Error>> {
    let sequence = legacy_sequence(4, &[2, 3], Some(1), 99)?;
    let mut state = SequencePlaybackState::new();

    advance_dynamic_sequence(&sequence, &mut state, 6)?;

    assert_eq!(state.frame(), Some(0));
    assert_eq!(state.frame_cycle(), 1);
    assert_eq!(state.completed_loops(), 0);
    Ok(())
}

#[test]
fn loop_limit_is_diagnostic_for_a_valid_looping_sequence() -> Result<(), Box<dyn Error>> {
    let sequence = legacy_sequence(5, &[1, 1], Some(1), 1)?;
    let mut state = SequencePlaybackState::new();

    advance_dynamic_sequence(&sequence, &mut state, 1)?;
    advance_dynamic_sequence(&sequence, &mut state, 1)?;
    let report = advance_dynamic_sequence(&sequence, &mut state, 1)?;

    assert!(report.wrapped);
    assert!(report.loop_limit_reached);
    assert!(!report.deactivated);
    assert!(state.is_active());
    assert_eq!(state.frame(), Some(1));
    assert_eq!(state.completed_loops(), 1);
    Ok(())
}

#[test]
fn skeletal_sequence_advances_one_frame_per_cycle_and_loops_by_frame_step()
-> Result<(), Box<dyn Error>> {
    let sequence = skeletal_sequence(6, SequenceRange { start: 10, end: 14 }, Some(2), 99)?;
    let mut state = SequencePlaybackState::new();

    advance_dynamic_sequence(&sequence, &mut state, 1)?;
    assert_eq!(state.frame(), Some(1));
    advance_dynamic_sequence(&sequence, &mut state, 1)?;
    assert_eq!(state.frame(), Some(2));
    advance_dynamic_sequence(&sequence, &mut state, 1)?;
    assert_eq!(state.frame(), Some(3));

    let report = advance_dynamic_sequence(&sequence, &mut state, 1)?;
    assert!(report.wrapped);
    assert_eq!(state.frame(), Some(2));
    assert_eq!(state.completed_loops(), 1);
    Ok(())
}

#[test]
fn skeletal_sequence_without_frame_step_deactivates_at_end() -> Result<(), Box<dyn Error>> {
    let sequence = skeletal_sequence(7, SequenceRange { start: 20, end: 23 }, None, 99)?;
    let mut state = SequencePlaybackState::new();

    advance_dynamic_sequence(&sequence, &mut state, 1)?;
    advance_dynamic_sequence(&sequence, &mut state, 1)?;
    let report = advance_dynamic_sequence(&sequence, &mut state, 1)?;

    assert!(report.wrapped);
    assert!(report.deactivated);
    assert_eq!(state.frame(), None);
    Ok(())
}

#[test]
fn skeletal_large_delta_uses_reference_loop_step_reduction() -> Result<(), Box<dyn Error>> {
    let sequence = skeletal_sequence(8, SequenceRange { start: 30, end: 35 }, Some(2), 99)?;
    let mut state = SequencePlaybackState::new();

    advance_dynamic_sequence(&sequence, &mut state, 5)?;

    assert_eq!(state.frame(), Some(1));
    assert_eq!(state.completed_loops(), 0);
    Ok(())
}

#[test]
fn malformed_sequence_inputs_fail_explicitly() -> Result<(), Box<dyn Error>> {
    let mut missing_frames = legacy_sequence(9, &[1], Some(1), 99)?;
    missing_frames.frame_ids.clear();
    let mut state = SequencePlaybackState::new();
    assert_eq!(
        advance_dynamic_sequence(&missing_frames, &mut state, 1),
        Err(SequenceAdvanceError::MissingLegacyFrames)
    );

    let invalid_range = skeletal_sequence(10, SequenceRange { start: 40, end: 39 }, Some(1), 99)?;
    let mut state = SequencePlaybackState::new();
    assert_eq!(
        advance_dynamic_sequence(&invalid_range, &mut state, 1),
        Err(SequenceAdvanceError::InvalidSkeletalRange { start: 40, end: 39 })
    );
    Ok(())
}
