//! Pure actor/effect runtime transitions shared by the future headless playtest runtime.
//!
//! These types port source-audited state transitions from the checked-in RuneLite client.
//! They do not decode packets, own world collections, or render GPU resources.

mod projectile;
mod spot;

pub use projectile::{EffectPoint, ProjectileState};
pub use spot::{ActorSpotAnimation, ActorSpotAnimations, GraphicsObjectState};

use crate::animation::{
    SequenceAdvanceError, SequenceAdvanceReport, SequencePlaybackState, advance_dynamic_sequence,
};
use crate::definitions::SequenceDefinition;
use crate::ids::SequenceId;
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectRuntimeError {
    NegativeCycleDelta(i32),
    SequenceMismatch {
        expected: SequenceId,
        actual: SequenceId,
    },
    InvalidFlightWindow {
        current_cycle: i32,
        cycle_end: i32,
    },
    Sequence(SequenceAdvanceError),
}

impl fmt::Display for EffectRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeCycleDelta(delta) => {
                write!(formatter, "effect cycle delta cannot be negative: {delta}")
            }
            Self::SequenceMismatch { expected, actual } => write!(
                formatter,
                "runtime sequence {expected} was advanced with definition {actual}"
            ),
            Self::InvalidFlightWindow {
                current_cycle,
                cycle_end,
            } => write!(
                formatter,
                "projectile flight window ended before cycle {current_cycle} (end {cycle_end})"
            ),
            Self::Sequence(error) => error.fmt(formatter),
        }
    }
}

impl Error for EffectRuntimeError {}

impl From<SequenceAdvanceError> for EffectRuntimeError {
    fn from(value: SequenceAdvanceError) -> Self {
        Self::Sequence(value)
    }
}

/// Instance-owned sequence id plus mutable frame/cycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuntimeSequence {
    pub id: SequenceId,
    pub playback: SequencePlaybackState,
}

impl RuntimeSequence {
    pub const fn new(id: SequenceId) -> Self {
        Self {
            id,
            playback: SequencePlaybackState::new(),
        }
    }

    pub fn advance(
        &mut self,
        definition: &SequenceDefinition,
        delta_cycles: i32,
    ) -> Result<SequenceAdvanceReport, EffectRuntimeError> {
        if definition.identity.id != self.id {
            return Err(EffectRuntimeError::SequenceMismatch {
                expected: self.id,
                actual: definition.identity.id,
            });
        }
        Ok(advance_dynamic_sequence(
            definition,
            &mut self.playback,
            delta_cycles,
        )?)
    }
}
