//! Exact OSRS orientation/angular value types and quarter-turn helpers.
//!
//! These helpers are semantic integer operations. They deliberately do not use
//! renderer Euler/quaternion conventions or floating-point matrices.

use crate::coords::ModelPoint;

/// Number of Jagex angular units (JAU) in one complete turn.
pub const JAU_FULL_TURN: u16 = 2048;
/// One semantic quarter turn in JAU.
pub const JAU_QUARTER_TURN: u16 = 512;
/// The special type-4 diagonal-decoration recenter angle from `MODEL-BUILD-003`.
pub const JAU_DIAGONAL_DECORATION: u16 = 256;

/// Ordinary loc orientation encoded by map/location data (`0..=3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocOrientation(u8);

impl LocOrientation {
    pub const fn new(value: u8) -> Option<Self> {
        if value <= 3 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub const fn quarter_turn(self) -> QuarterTurn {
        QuarterTurn::from_low_bits(self.0)
    }

    pub const fn model_orientation(self) -> ModelOrientation {
        ModelOrientation(self.0)
    }
}

/// Model-construction orientation accepted by audited object paths (`0..=7`).
///
/// Values above three preserve the semantic `orientation > 3` branch before
/// ordinary orientation is reduced with `orientation & 3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModelOrientation(u8);

impl ModelOrientation {
    pub const fn new(value: u8) -> Option<Self> {
        if value <= 7 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub const fn has_extended_variant(self) -> bool {
        self.0 > 3
    }

    pub const fn quarter_turn(self) -> QuarterTurn {
        QuarterTurn::from_low_bits(self.0)
    }
}

/// Dedicated quarter-turn operation used by ModelData construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum QuarterTurn {
    Turn0 = 0,
    Turn1 = 1,
    Turn2 = 2,
    Turn3 = 3,
}

impl QuarterTurn {
    pub const fn from_index(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Turn0),
            1 => Some(Self::Turn1),
            2 => Some(Self::Turn2),
            3 => Some(Self::Turn3),
            _ => None,
        }
    }

    const fn from_low_bits(value: u8) -> Self {
        match value & 3 {
            0 => Self::Turn0,
            1 => Self::Turn1,
            2 => Self::Turn2,
            _ => Self::Turn3,
        }
    }

    pub const fn index(self) -> u8 {
        self as u8
    }

    /// Apply the audited ModelData Y-axis quarter-turn operation.
    ///
    /// Wrapping negation matches Java `int` two's-complement behavior even for
    /// values outside practical model-coordinate ranges.
    pub const fn rotate_model_point(self, point: ModelPoint) -> ModelPoint {
        match self {
            Self::Turn0 => point,
            Self::Turn1 => ModelPoint::new(point.z, point.y, point.x.wrapping_neg()),
            Self::Turn2 => ModelPoint::new(
                point.x.wrapping_neg(),
                point.y,
                point.z.wrapping_neg(),
            ),
            Self::Turn3 => ModelPoint::new(point.z.wrapping_neg(), point.y, point.x),
        }
    }
}

/// Wrapped Jagex angular-unit value in the canonical `0..2047` range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JauAngle(u16);

impl JauAngle {
    pub const ZERO: Self = Self(0);
    pub const QUARTER: Self = Self(JAU_QUARTER_TURN);
    pub const DIAGONAL_DECORATION: Self = Self(JAU_DIAGONAL_DECORATION);

    pub const fn new_wrapped(value: u16) -> Self {
        Self(value & (JAU_FULL_TURN - 1))
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_and_extended_orientation_ranges_are_distinct() {
        assert_eq!(LocOrientation::new(3).map(LocOrientation::get), Some(3));
        assert_eq!(LocOrientation::new(4), None);
        assert_eq!(ModelOrientation::new(7).map(ModelOrientation::get), Some(7));
        assert_eq!(ModelOrientation::new(8), None);
    }

    #[test]
    fn extended_orientation_preserves_branch_then_masks_to_quarter_turn() {
        let orientation = ModelOrientation::new(6);
        assert_eq!(
            orientation.map(ModelOrientation::has_extended_variant),
            Some(true)
        );
        assert_eq!(
            orientation.map(ModelOrientation::quarter_turn),
            Some(QuarterTurn::Turn2)
        );
    }

    #[test]
    fn quarter_turns_match_modeldata_integer_operations() {
        let point = ModelPoint::new(10, 20, 30);
        assert_eq!(
            QuarterTurn::Turn0.rotate_model_point(point),
            ModelPoint::new(10, 20, 30)
        );
        assert_eq!(
            QuarterTurn::Turn1.rotate_model_point(point),
            ModelPoint::new(30, 20, -10)
        );
        assert_eq!(
            QuarterTurn::Turn2.rotate_model_point(point),
            ModelPoint::new(-10, 20, -30)
        );
        assert_eq!(
            QuarterTurn::Turn3.rotate_model_point(point),
            ModelPoint::new(-30, 20, 10)
        );
    }

    #[test]
    fn four_quarter_turns_return_exact_original_point() {
        let mut point = ModelPoint::new(-123, 456, 789);
        for _ in 0..4 {
            point = QuarterTurn::Turn1.rotate_model_point(point);
        }
        assert_eq!(point, ModelPoint::new(-123, 456, 789));
    }

    #[test]
    fn quarter_turn_uses_java_style_wrapping_negation() {
        let point = ModelPoint::new(i32::MIN, 0, i32::MIN);
        assert_eq!(
            QuarterTurn::Turn2.rotate_model_point(point),
            ModelPoint::new(i32::MIN, 0, i32::MIN)
        );
    }

    #[test]
    fn jau_values_wrap_to_reference_domain() {
        assert_eq!(JauAngle::new_wrapped(0).get(), 0);
        assert_eq!(JauAngle::new_wrapped(2047).get(), 2047);
        assert_eq!(JauAngle::new_wrapped(2048).get(), 0);
        assert_eq!(JauAngle::new_wrapped(2304).get(), 256);
        assert_eq!(JauAngle::DIAGONAL_DECORATION.get(), 256);
    }
}
