use super::{EffectRuntimeError, RuntimeSequence};
use crate::animation::SequenceAdvanceReport;
use crate::definitions::SequenceDefinition;
use crate::entity::SpotAnimationDefinition;
use crate::ids::SpotAnimationId;

const PROJECTILE_SLOPE_RADIANS: f64 = 0.02454369;
const ANGLE_UNITS_PER_RADIAN: f64 = 325.949;
const PROJECTILE_ZERO_DISTANCE: f64 = 0.009999999776482582;
const ANGLE_MASK: i32 = 2047;

/// Double-precision world point used by the source projectile integrator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl EffectPoint {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// Deterministic projectile trajectory/orientation state.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectileState {
    pub id: SpotAnimationId,
    pub cycle_start: i32,
    pub cycle_end: i32,
    pub slope: i32,
    pub start_pos: i32,
    pub position: Option<EffectPoint>,
    pub velocity_x: f64,
    pub velocity_y: f64,
    pub velocity_z: f64,
    pub acceleration_z: f64,
    pub horizontal_speed: f64,
    pub yaw: i32,
    pub pitch: i32,
    pub moving: bool,
    pub sequence: Option<RuntimeSequence>,
}

impl ProjectileState {
    pub fn new(
        definition: &SpotAnimationDefinition,
        cycle_start: i32,
        cycle_end: i32,
        slope: i32,
        start_pos: i32,
    ) -> Self {
        Self {
            id: definition.identity.id,
            cycle_start,
            cycle_end,
            slope,
            start_pos,
            position: None,
            velocity_x: 0.0,
            velocity_y: 0.0,
            velocity_z: 0.0,
            acceleration_z: 0.0,
            horizontal_speed: 0.0,
            yaw: 0,
            pitch: 0,
            moving: false,
            sequence: definition.sequence.map(RuntimeSequence::new),
        }
    }

    /// Port of `Projectile.setDestination` after world-view coordinate/height resolution.
    pub fn update_destination(
        &mut self,
        current_cycle: i32,
        delta_cycles: i32,
        source: EffectPoint,
        target: EffectPoint,
    ) -> Result<(), EffectRuntimeError> {
        if delta_cycles < 0 {
            return Err(EffectRuntimeError::NegativeCycleDelta(delta_cycles));
        }
        if current_cycle < self.cycle_start {
            return Ok(());
        }

        if !self.moving {
            let dx = target.x - source.x;
            let dy = target.y - source.y;
            let distance = (dx * dx + dy * dy).sqrt();
            self.position = Some(if distance.abs() < PROJECTILE_ZERO_DISTANCE {
                source
            } else {
                let start = f64::from(self.start_pos) / distance;
                EffectPoint::new(source.x + dx * start, source.y + dy * start, source.z)
            });
        }

        let remaining_i32 = self.cycle_end.wrapping_add(1).wrapping_sub(current_cycle);
        if remaining_i32 <= 0 {
            return Err(EffectRuntimeError::InvalidFlightWindow {
                current_cycle,
                cycle_end: self.cycle_end,
            });
        }
        let remaining = f64::from(remaining_i32);
        let mut position = self.position.unwrap_or(source);
        self.velocity_x = (target.x - position.x) / remaining;
        self.velocity_y = (target.y - position.y) / remaining;
        self.horizontal_speed =
            (self.velocity_x * self.velocity_x + self.velocity_y * self.velocity_y).sqrt();

        if !self.moving {
            self.velocity_z =
                -self.horizontal_speed * (PROJECTILE_SLOPE_RADIANS * f64::from(self.slope)).tan();
        }

        self.acceleration_z =
            2.0 * (target.z - position.z - remaining * self.velocity_z) / (remaining * remaining);
        self.moving = true;

        let delta = f64::from(delta_cycles);
        position.x += delta * self.velocity_x;
        position.y += delta * self.velocity_y;
        position.z += self.velocity_z * delta + delta * 0.5 * self.acceleration_z * delta;
        self.velocity_z += self.acceleration_z * delta;
        self.position = Some(position);

        self.yaw = ((self.velocity_x.atan2(self.velocity_y) * ANGLE_UNITS_PER_RADIAN) as i32
            + 1024)
            & ANGLE_MASK;
        self.pitch = ((self.velocity_z.atan2(self.horizontal_speed) * ANGLE_UNITS_PER_RADIAN)
            as i32)
            & ANGLE_MASK;
        Ok(())
    }

    pub fn advance_sequence(
        &mut self,
        definition: &SequenceDefinition,
        delta_cycles: i32,
    ) -> Result<SequenceAdvanceReport, EffectRuntimeError> {
        let Some(sequence) = &mut self.sequence else {
            return Ok(SequenceAdvanceReport::default());
        };
        sequence.advance(definition, delta_cycles)
    }
}
