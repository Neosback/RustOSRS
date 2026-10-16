//! Finite appearance values: construction and deserialization enforce equality's invariants.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct ThemeNumber(f32);

// NaN and infinities cannot enter this private representation.
impl Eq for ThemeNumber {}

impl ThemeNumber {
    pub fn new(value: f32) -> Result<Self, &'static str> {
        if value.is_finite() {
            Ok(Self(if value == 0.0 { 0.0 } else { value }))
        } else {
            Err("appearance values must be finite")
        }
    }
    pub const fn get(self) -> f32 {
        self.0
    }
}
impl TryFrom<f32> for ThemeNumber {
    type Error = &'static str;
    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<ThemeNumber> for f32 {
    fn from(value: ThemeNumber) -> Self {
        value.0
    }
}
