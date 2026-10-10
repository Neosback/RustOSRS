//! Dynamic-object primary sequence replacement semantics for M8.
//!
//! The pinned `DynamicObject` constructor preserves primary animation state only
//! when the replaced object is running the same sequence id and that sequence's
//! opcode-11 `restartMode` is zero. Every other case starts the replacement at
//! the deterministic initial state.

use crate::{animation::SequencePlaybackState, definitions::SequenceDefinition, ids::SequenceId};

/// Observable result of applying the pinned primary-sequence replacement rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReplacementPlaybackDecision {
    /// Same sequence id and restart mode zero: copy the prior playback state.
    Preserved,
    /// Different/missing prior sequence, inactive prior state, or nonzero
    /// restart mode: start the replacement sequence from its initial state.
    Restarted,
}

/// Apply the pinned `DynamicObject` primary animation carry-forward rule.
///
/// `previous` represents the replaced dynamic object's active primary sequence
/// id and its instance-owned playback state. The cache field currently stored as
/// `SequenceDefinition::reply_mode` is opcode 11; in the pinned build-241 client
/// that exact field is exported as `restartMode` and defaults to `2`.
///
/// The secondary/fallback `animationSequence` carry-forward path is deliberately
/// not modeled here; it is a separate constructor branch and remains later M8
/// ownership.
pub fn replacement_primary_playback(
    new_sequence: &SequenceDefinition,
    previous: Option<(SequenceId, SequencePlaybackState)>,
) -> (SequencePlaybackState, ReplacementPlaybackDecision) {
    if let Some((previous_sequence_id, previous_state)) = previous
        && previous_sequence_id == new_sequence.identity.id
        && new_sequence.reply_mode == 0
        && previous_state.is_active()
    {
        return (previous_state, ReplacementPlaybackDecision::Preserved);
    }

    (
        SequencePlaybackState::new(),
        ReplacementPlaybackDecision::Restarted,
    )
}
