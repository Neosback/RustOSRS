use osrs_cache::{
    decode::{ArchiveFileProvenance, DecoderContext, decode_sequence_definition},
    profile::TargetProfile,
};
use osrs_core::{
    animation::advance_dynamic_sequence,
    ids::SequenceId,
    sequence_replacement::{
        PrimarySequenceState, ReplacementPlaybackDecision, replacement_primary_sequence,
    },
};
use std::error::Error;

const PROFILE_YAML: &str =
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");

fn decoder_context() -> Result<DecoderContext, Box<dyn Error>> {
    let profile = TargetProfile::from_yaml_str(PROFILE_YAML)?;
    Ok(DecoderContext::from_profile(&profile)?)
}

fn sequence_bytes(restart_mode: u8) -> Vec<u8> {
    vec![
        1,
        0,
        2, // two legacy frames
        0,
        2,
        0,
        3, // frame delays
        0,
        0,
        0,
        1, // frame-id low halves
        0,
        1,
        0,
        1, // frame-id high halves
        2,
        0,
        2, // frame step / loopback
        11,
        restart_mode, // pinned restartMode
        0,
    ]
}

fn decoded_sequence(
    id: u32,
    restart_mode: u8,
) -> Result<osrs_core::definitions::SequenceDefinition, Box<dyn Error>> {
    let context = decoder_context()?;
    let source = ArchiveFileProvenance::new(2, 12, Some(id));
    Ok(decode_sequence_definition(
        SequenceId::new(id),
        &sequence_bytes(restart_mode),
        &context,
        &source,
    )?)
}

fn progressed_state(
    sequence: &osrs_core::definitions::SequenceDefinition,
    current_cycle: i32,
) -> Result<PrimarySequenceState, Box<dyn Error>> {
    let mut state = PrimarySequenceState::start(sequence, current_cycle);
    advance_dynamic_sequence(sequence, &mut state.playback, 4)?;
    assert_eq!(state.playback.frame(), Some(1));
    assert_eq!(state.playback.frame_cycle(), 2);
    Ok(state)
}

#[test]
fn same_sequence_restart_mode_zero_preserves_playback_and_cycle_start()
-> Result<(), Box<dyn Error>> {
    let sequence = decoded_sequence(40, 0)?;
    let mut previous = progressed_state(&sequence, 100)?;
    previous.cycle_start = 77;

    let (replacement, decision) = replacement_primary_sequence(&sequence, 200, Some(&previous));

    assert_eq!(decision, ReplacementPlaybackDecision::Preserved);
    assert_eq!(replacement, previous);
    assert_eq!(replacement.playback.frame(), Some(1));
    assert_eq!(replacement.playback.frame_cycle(), 2);
    assert_eq!(replacement.cycle_start, 77);
    Ok(())
}

#[test]
fn same_sequence_nonzero_restart_mode_restarts_primary_state() -> Result<(), Box<dyn Error>> {
    let source_sequence = decoded_sequence(41, 0)?;
    let replacement_sequence = decoded_sequence(41, 2)?;
    let previous = progressed_state(&source_sequence, 100)?;

    let (replacement, decision) =
        replacement_primary_sequence(&replacement_sequence, 200, Some(&previous));

    assert_eq!(decision, ReplacementPlaybackDecision::Restarted);
    assert_eq!(replacement.sequence_id, SequenceId::new(41));
    assert_eq!(replacement.playback.frame(), Some(0));
    assert_eq!(replacement.playback.frame_cycle(), 0);
    assert_eq!(replacement.playback.completed_loops(), 0);
    assert_eq!(replacement.cycle_start, 199);
    Ok(())
}

#[test]
fn different_sequence_id_restarts_even_when_restart_mode_is_zero() -> Result<(), Box<dyn Error>> {
    let old_sequence = decoded_sequence(42, 0)?;
    let new_sequence = decoded_sequence(43, 0)?;
    let previous = progressed_state(&old_sequence, 100)?;

    let (replacement, decision) =
        replacement_primary_sequence(&new_sequence, 300, Some(&previous));

    assert_eq!(decision, ReplacementPlaybackDecision::Restarted);
    assert_eq!(replacement.sequence_id, SequenceId::new(43));
    assert_eq!(replacement.playback.frame(), Some(0));
    assert_eq!(replacement.cycle_start, 299);
    Ok(())
}

#[test]
fn fresh_primary_state_uses_client_cycle_minus_one_with_java_wraparound()
-> Result<(), Box<dyn Error>> {
    let sequence = decoded_sequence(44, 0)?;

    let (replacement, decision) = replacement_primary_sequence(&sequence, i32::MIN, None);

    assert_eq!(decision, ReplacementPlaybackDecision::Restarted);
    assert_eq!(replacement.sequence_id, SequenceId::new(44));
    assert_eq!(replacement.playback.frame(), Some(0));
    assert_eq!(replacement.cycle_start, i32::MAX);
    Ok(())
}
