use super::{EffectRuntimeError, RuntimeSequence};
use crate::animation::SequenceAdvanceReport;
use crate::definitions::SequenceDefinition;
use crate::entity::SpotAnimationDefinition;
use crate::ids::SpotAnimationId;
use std::collections::BTreeMap;

/// Runtime state corresponding to the client's `ActorSpotAnim`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActorSpotAnimation {
    pub id: SpotAnimationId,
    pub height: i32,
    pub start_cycle: i32,
    pub sequence: Option<RuntimeSequence>,
}

impl ActorSpotAnimation {
    pub fn from_definition(
        definition: &SpotAnimationDefinition,
        height: i32,
        current_cycle: i32,
        delay_cycles: i32,
    ) -> Self {
        Self {
            id: definition.identity.id,
            height,
            start_cycle: current_cycle.wrapping_add(delay_cycles),
            sequence: definition.sequence.map(RuntimeSequence::new),
        }
    }

    pub const fn is_started(self, current_cycle: i32) -> bool {
        current_cycle >= self.start_cycle
    }
}

/// Keyed actor spot attachments. Updating one key replaces only that attachment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActorSpotAnimations {
    entries: BTreeMap<u32, ActorSpotAnimation>,
}

impl ActorSpotAnimations {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, key: u32) -> Option<&ActorSpotAnimation> {
        self.entries.get(&key)
    }

    /// Revision-specific removal sentinels must be normalized to `None` before this call.
    pub fn update(
        &mut self,
        key: u32,
        definition: Option<&SpotAnimationDefinition>,
        height: i32,
        current_cycle: i32,
        delay_cycles: i32,
    ) -> Option<ActorSpotAnimation> {
        let previous = self.entries.remove(&key);
        if let Some(definition) = definition {
            self.entries.insert(
                key,
                ActorSpotAnimation::from_definition(
                    definition,
                    height,
                    current_cycle,
                    delay_cycles,
                ),
            );
        }
        previous
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Standalone world graphics state corresponding to the client's `GraphicsObject`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsObjectState {
    pub id: SpotAnimationId,
    pub plane: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub start_cycle: i32,
    pub sequence: Option<RuntimeSequence>,
    finished: bool,
}

impl GraphicsObjectState {
    pub fn new(
        definition: &SpotAnimationDefinition,
        plane: i32,
        x: i32,
        y: i32,
        z: i32,
        delay_cycles: i32,
        current_cycle: i32,
    ) -> Self {
        let sequence = definition.sequence.map(RuntimeSequence::new);
        Self {
            id: definition.identity.id,
            plane,
            x,
            y,
            z,
            start_cycle: current_cycle.wrapping_add(delay_cycles),
            finished: sequence.is_none(),
            sequence,
        }
    }

    pub const fn is_started(&self, current_cycle: i32) -> bool {
        current_cycle >= self.start_cycle
    }

    pub const fn is_finished(&self) -> bool {
        self.finished
    }

    /// The client retires a graphics object on its first sequence wrap/completion.
    pub fn advance(
        &mut self,
        definition: &SequenceDefinition,
        delta_cycles: i32,
    ) -> Result<SequenceAdvanceReport, EffectRuntimeError> {
        if delta_cycles < 0 {
            return Err(EffectRuntimeError::NegativeCycleDelta(delta_cycles));
        }
        let Some(sequence) = &mut self.sequence else {
            return Ok(SequenceAdvanceReport::default());
        };
        if self.finished {
            return Ok(SequenceAdvanceReport::default());
        }

        let report = sequence.advance(definition, delta_cycles)?;
        if report.wrapped || report.deactivated {
            sequence.playback.reset();
            self.finished = true;
        }
        Ok(report)
    }
}
