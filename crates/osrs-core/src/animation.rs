//! Deterministic dynamic-object sequence progression for M8.
//!
//! This module ports the frame/cycle state transitions used by the pinned
//! `AnimationSequence` + `GrandExchangeOfferUnitPriceComparator.method8811`
//! dynamic-object path. Actual skeleton/model deformation remains separate:
//! this code only decides which sequence frame is active at a preview tick.

use crate::{definitions::SequenceDefinition, ids::SequenceId};
use std::{error::Error, fmt};

/// Mutable per-instance sequence state.
///
/// This state is intentionally instance-owned. Shared decoded sequence
/// definitions remain immutable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SequencePlaybackState {
    frame: u32,
    frame_cycle: i32,
    completed_loops: i32,
    active: bool,
}

impl SequencePlaybackState {
    /// Start a sequence at the deterministic client default: frame 0, cycle 0.
    ///
    /// Randomized starts from the dynamic-object constructor are policy owned by
    /// a later M8 checkpoint and are not implicit here.
    pub const fn new() -> Self {
        Self {
            frame: 0,
            frame_cycle: 0,
            completed_loops: 0,
            active: true,
        }
    }

    pub const fn is_active(self) -> bool {
        self.active
    }

    /// Current zero-based frame index, or `None` after a non-looping sequence
    /// has completed and the dynamic object has reset its animation sequence.
    pub const fn frame(self) -> Option<u32> {
        if self.active { Some(self.frame) } else { None }
    }

    pub const fn frame_cycle(self) -> i32 {
        self.frame_cycle
    }

    pub const fn completed_loops(self) -> i32 {
        self.completed_loops
    }

    pub fn reset(&mut self) {
        self.active = false;
    }
}

impl Default for SequencePlaybackState {
    fn default() -> Self {
        Self::new()
    }
}

/// Primary dynamic-object animation state relevant to replacement semantics.
///
/// This mirrors the audited `field775` + `cycleStart` ownership without
/// importing the client's secondary fallback animation resource state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DynamicSequenceState {
    pub sequence_id: SequenceId,
    pub playback: SequencePlaybackState,
    pub cycle_start: i32,
}

impl DynamicSequenceState {
    /// Create the deterministic non-randomized constructor state.
    ///
    /// The client initializes `cycleStart` to `Client.cycle - 1`, so the first
    /// model resolution consumes one cycle. Java int wraparound is preserved.
    pub fn start(sequence: &SequenceDefinition, current_cycle: i32) -> Self {
        Self {
            sequence_id: sequence.identity.id,
            playback: SequencePlaybackState::new(),
            cycle_start: current_cycle.wrapping_sub(1),
        }
    }
}

/// Result of replacing one primary dynamic-object sequence instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SequenceReplacementOutcome {
    Preserved,
    Restarted,
}

/// Apply the pinned `DynamicObject` primary-sequence replacement contract.
///
/// Playback state and `cycleStart` carry forward only when the previous dynamic
/// object uses the same sequence id and the new sequence's build-241
/// `restartMode` is exactly `0`. All other replacements restart from the
/// deterministic constructor state. Randomized starts and the client's
/// secondary fallback sequence/resource-readiness path are intentionally out of
/// scope here.
pub fn replace_dynamic_sequence(
    sequence: &SequenceDefinition,
    current_cycle: i32,
    previous: Option<&DynamicSequenceState>,
) -> (DynamicSequenceState, SequenceReplacementOutcome) {
    if let Some(previous) = previous
        && previous.sequence_id == sequence.identity.id
        && sequence.restart_mode == 0
    {
        return (*previous, SequenceReplacementOutcome::Preserved);
    }

    (
        DynamicSequenceState::start(sequence, current_cycle),
        SequenceReplacementOutcome::Restarted,
    )
}

/// Observable semantic effects of one deterministic sequence advance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SequenceAdvanceReport {
    /// At least one frame boundary was crossed.
    pub frame_advanced: bool,
    /// At least one end-of-sequence boundary was crossed.
    pub wrapped: bool,
    /// The reference loop-limit/invalid-loopback condition was encountered.
    ///
    /// This is diagnostic state; the audited dynamic-object path does not stop
    /// a valid looping sequence merely because this condition becomes true.
    pub loop_limit_reached: bool,
    /// The sequence had no frame-step/loopback and therefore reset after its
    /// first end-of-sequence boundary.
    pub deactivated: bool,
}

/// Malformed sequence/runtime state that cannot be advanced safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceAdvanceError {
    NegativeCycleDelta(i32),
    MissingLegacyFrames,
    LegacyFrameLengthMismatch {
        frame_ids: usize,
        frame_delays: usize,
    },
    MissingSkeletalRange,
    InvalidSkeletalRange {
        start: u16,
        end: u16,
    },
    EmptySkeletalRange,
    FrameIndexOverflow,
    InvalidActiveFrame {
        frame: u32,
        frame_count: u32,
    },
    NonProgressingSequence,
}

impl fmt::Display for SequenceAdvanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeCycleDelta(delta) => {
                write!(
                    formatter,
                    "sequence cycle delta cannot be negative: {delta}"
                )
            }
            Self::MissingLegacyFrames => {
                formatter.write_str("frame-based sequence has no frame ids")
            }
            Self::LegacyFrameLengthMismatch {
                frame_ids,
                frame_delays,
            } => write!(
                formatter,
                "frame-based sequence has {frame_ids} frame ids but {frame_delays} frame delays"
            ),
            Self::MissingSkeletalRange => formatter
                .write_str("cached/skeletal sequence is missing its canonical animation range"),
            Self::InvalidSkeletalRange { start, end } => write!(
                formatter,
                "cached/skeletal sequence range ends before it starts: {start}..{end}"
            ),
            Self::EmptySkeletalRange => {
                formatter.write_str("cached/skeletal sequence range contains zero frames")
            }
            Self::FrameIndexOverflow => {
                formatter.write_str("sequence frame count exceeds supported reference int range")
            }
            Self::InvalidActiveFrame { frame, frame_count } => write!(
                formatter,
                "active sequence frame {frame} is outside frame count {frame_count}"
            ),
            Self::NonProgressingSequence => formatter.write_str(
                "sequence cannot consume positive cycle time because all frame delays are zero",
            ),
        }
    }
}

impl Error for SequenceAdvanceError {}

/// Advance one dynamic-object sequence by an exact client-cycle delta.
///
/// This composes the pinned sequence-step helper with the dynamic object's
/// immediate reset rule for sequences that have no frame-step. Event callbacks,
/// asynchronous animation-resource readiness, randomized construction starts,
/// model posing, and replacement carry-forward are deliberately outside this
/// checkpoint.
pub fn advance_dynamic_sequence(
    sequence: &SequenceDefinition,
    state: &mut SequencePlaybackState,
    delta_cycles: i32,
) -> Result<SequenceAdvanceReport, SequenceAdvanceError> {
    if delta_cycles < 0 {
        return Err(SequenceAdvanceError::NegativeCycleDelta(delta_cycles));
    }
    if !state.active {
        return Ok(SequenceAdvanceReport::default());
    }

    if sequence.skeletal_animation.is_some() {
        advance_skeletal(sequence, state, delta_cycles)
    } else {
        advance_legacy(sequence, state, delta_cycles)
    }
}

fn advance_legacy(
    sequence: &SequenceDefinition,
    state: &mut SequencePlaybackState,
    delta_cycles: i32,
) -> Result<SequenceAdvanceReport, SequenceAdvanceError> {
    if sequence.frame_ids.is_empty() {
        return Err(SequenceAdvanceError::MissingLegacyFrames);
    }
    if sequence.frame_ids.len() != sequence.frame_delays.len() {
        return Err(SequenceAdvanceError::LegacyFrameLengthMismatch {
            frame_ids: sequence.frame_ids.len(),
            frame_delays: sequence.frame_delays.len(),
        });
    }

    let frame_count = i32::try_from(sequence.frame_ids.len())
        .map_err(|_| SequenceAdvanceError::FrameIndexOverflow)?;
    let frame_count_u32 =
        u32::try_from(frame_count).map_err(|_| SequenceAdvanceError::FrameIndexOverflow)?;
    if state.frame >= frame_count_u32 {
        // `method8811` repairs this legacy state before consuming time.
        state.frame = 0;
        state.frame_cycle = 0;
    }

    let total_delay = sequence
        .frame_delays
        .iter()
        .fold(0_i32, |total, delay| total.wrapping_add(i32::from(*delay)));
    let mut remaining = delta_cycles;
    if sequence.frame_step.is_some_and(|step| step > 0) && total_delay > 0 {
        remaining %= total_delay;
    }

    if remaining > 0 && total_delay == 0 {
        return Err(SequenceAdvanceError::NonProgressingSequence);
    }

    let mut frame =
        i32::try_from(state.frame).map_err(|_| SequenceAdvanceError::FrameIndexOverflow)?;
    let mut frame_cycle = state.frame_cycle.wrapping_add(remaining);
    let mut completed_loops = state.completed_loops;
    let mut report = SequenceAdvanceReport::default();

    loop {
        let frame_index =
            usize::try_from(frame).map_err(|_| SequenceAdvanceError::InvalidActiveFrame {
                frame: state.frame,
                frame_count: frame_count_u32,
            })?;
        let delay = i32::from(sequence.frame_delays[frame_index]);
        if frame_cycle <= delay {
            state.frame =
                u32::try_from(frame).map_err(|_| SequenceAdvanceError::FrameIndexOverflow)?;
            state.frame_cycle = frame_cycle;
            state.completed_loops = completed_loops;
            return Ok(report);
        }

        frame_cycle = frame_cycle.wrapping_sub(delay);
        frame = frame.wrapping_add(1);
        report.frame_advanced = true;
        if frame < frame_count {
            continue;
        }

        completed_loops = completed_loops.wrapping_add(1);
        report.wrapped = true;

        let Some(frame_step) = sequence.frame_step else {
            state.frame = 0;
            state.frame_cycle = frame_cycle;
            state.completed_loops = completed_loops;
            state.reset();
            report.loop_limit_reached = true;
            report.deactivated = true;
            return Ok(report);
        };

        frame = frame.wrapping_sub(i32::from(frame_step));
        if completed_loops >= i32::from(sequence.max_loops) {
            report.loop_limit_reached = true;
        }
        if frame < 0 || frame >= frame_count {
            report.loop_limit_reached = true;
            frame = 0;
        }
    }
}

fn advance_skeletal(
    sequence: &SequenceDefinition,
    state: &mut SequencePlaybackState,
    delta_cycles: i32,
) -> Result<SequenceAdvanceReport, SequenceAdvanceError> {
    let range = sequence
        .skeletal_range
        .ok_or(SequenceAdvanceError::MissingSkeletalRange)?;
    if range.end < range.start {
        return Err(SequenceAdvanceError::InvalidSkeletalRange {
            start: range.start,
            end: range.end,
        });
    }

    let frame_count = u32::from(range.end - range.start);
    if frame_count == 0 {
        return Err(SequenceAdvanceError::EmptySkeletalRange);
    }
    if state.frame >= frame_count {
        return Err(SequenceAdvanceError::InvalidActiveFrame {
            frame: state.frame,
            frame_count,
        });
    }

    let mut remaining = delta_cycles;
    if let Some(frame_step) = sequence.frame_step {
        let frame_step = i32::from(frame_step);
        if frame_step > 0 && remaining > 0 {
            remaining = remaining.wrapping_sub(
                remaining
                    .wrapping_sub(1)
                    .wrapping_div(frame_step)
                    .wrapping_mul(frame_step),
            );
        }
    }

    let frame_count_i32 =
        i32::try_from(frame_count).map_err(|_| SequenceAdvanceError::FrameIndexOverflow)?;
    let mut frame =
        i32::try_from(state.frame).map_err(|_| SequenceAdvanceError::FrameIndexOverflow)?;
    let mut completed_loops = state.completed_loops;
    let mut report = SequenceAdvanceReport::default();

    while remaining > 0 {
        remaining = remaining.wrapping_sub(1);
        frame = frame.wrapping_add(1);
        report.frame_advanced = true;
        if frame < frame_count_i32 {
            continue;
        }

        completed_loops = completed_loops.wrapping_add(1);
        report.wrapped = true;

        let Some(frame_step) = sequence.frame_step else {
            state.frame = 0;
            state.frame_cycle = 0;
            state.completed_loops = completed_loops;
            state.reset();
            report.loop_limit_reached = true;
            report.deactivated = true;
            return Ok(report);
        };

        frame = frame.wrapping_sub(i32::from(frame_step));
        if completed_loops >= i32::from(sequence.max_loops) {
            report.loop_limit_reached = true;
        }
        if frame < 0 || frame >= frame_count_i32 {
            report.loop_limit_reached = true;
            frame = 0;
        }
    }

    state.frame = u32::try_from(frame).map_err(|_| SequenceAdvanceError::FrameIndexOverflow)?;
    state.frame_cycle = 0;
    state.completed_loops = completed_loops;
    Ok(report)
}
