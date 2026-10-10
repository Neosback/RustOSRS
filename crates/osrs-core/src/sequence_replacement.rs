//! Dynamic-object primary sequence replacement semantics for M8.
//!
//! The pinned `DynamicObject` constructor preserves primary animation state and
//! `cycleStart` only when the replaced object is running the same sequence id and
//! that sequence's opcode-11 `restartMode` is zero. Every other case starts the
//! replacement at the deterministic initial state.

use crate::{animation::SequencePlaybackState, definitions::SequenceDefinition, ids::SequenceId};

/// Instance-owned primary dynamic-object sequence state relevant to replacement.
///
/// `None` at the call site represents a replaced object with no active primary
/// sequence. Secondary/fallback animation-resource state is intentionally not
/// included in this M8 slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrimarySequenceState {
    pub sequence_id: SequenceId,
    pub playback: SequencePlaybackState,
    pub cycle_start: i32,
}

impl PrimarySequenceState {
    /// Deterministic constructor state before any optional randomized start.
    ///
    /// The client initializes `cycleStart` to `Client.cycle - 1`; Java int
    /// wraparound is preserved explicitly.
    pub fn start(sequence: &SequenceDefinition, current_cycle: i32) -> Self {
        Self {
            sequence_id: sequence.identity.id,
            playback: SequencePlaybackState::new(),
            cycle_start: current_cycle.wrapping_sub(1),
        }
    }
}

/// Observable result of applying the pinned primary-sequence replacement rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReplacementPlaybackDecision {
    /// Same sequence id and restart mode zero: copy prior playback + cycleStart.
    Preserved,
    /// Missing/different prior sequence or nonzero restart mode: restart.
    Restarted,
}

/// Apply the pinned `DynamicObject` primary animation carry-forward rule.
///
/// The constructor copies the previous primary `AnimationSequence` and
/// `cycleStart` only when the sequence id is unchanged and the new definition's
/// build-241 `restartMode` is exactly zero. Otherwise the new primary sequence
/// begins at frame zero with `cycleStart = current_cycle - 1`.
///
/// Randomized starts and the secondary/fallback `animationSequence`
/// resource-readiness branch are deliberately outside this checkpoint.
pub fn replacement_primary_sequence(
    new_sequence: &SequenceDefinition,
    current_cycle: i32,
    previous: Option<&PrimarySequenceState>,
) -> (PrimarySequenceState, ReplacementPlaybackDecision) {
    if let Some(previous) = previous
        && previous.sequence_id == new_sequence.identity.id
        && new_sequence.restart_mode == 0
    {
        return (*previous, ReplacementPlaybackDecision::Preserved);
    }

    (
        PrimarySequenceState::start(new_sequence, current_cycle),
        ReplacementPlaybackDecision::Restarted,
    )
}
