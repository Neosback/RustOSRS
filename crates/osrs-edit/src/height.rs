//! Height sculpting: what the Height tool does to the tiles under its brush.
//!
//! Pure arithmetic over client heights (up is negative, stored in steps of 8). Each op reads the
//! current height of a tile through a lookup (so it sees earlier edits) and returns the new heights;
//! the editor turns them into edits and clamps them to what the map file can store.

pub(crate) mod commit;
pub use commit::{HeightCommitError, HeightStrokeSpec, build_terrain_height_patch};

use crate::brush::{BrushShape, footprint};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HeightOp {
    /// Lift the terrain.
    #[default]
    Raise,
    /// Sink the terrain.
    Lower,
    /// Pull each tile towards the average of its neighbours.
    Smooth,
    /// Pull each tile towards one height (the one under the cursor when the stroke began, or a set value).
    Flatten,
    /// Add random bumps.
    Roughen,
}

impl HeightOp {
    pub const ALL: [Self; 5] = [
        Self::Raise,
        Self::Lower,
        Self::Smooth,
        Self::Flatten,
        Self::Roughen,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Raise => "Raise",
            Self::Lower => "Lower",
            Self::Smooth => "Smooth",
            Self::Flatten => "Flatten",
            Self::Roughen => "Roughen",
        }
    }

    pub const fn hint(self) -> &'static str {
        match self {
            Self::Raise => "Lift the terrain under the brush",
            Self::Lower => "Sink the terrain under the brush",
            Self::Smooth => "Blend each tile towards its neighbours",
            Self::Flatten => "Level the terrain to one height",
            Self::Roughen => "Add random bumps",
        }
    }
}

/// How strongly the brush acts from its centre to its edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Falloff {
    /// Full strength everywhere.
    Flat,
    /// Fades in a straight line.
    Linear,
    /// Fades smoothly (ease in and out).
    #[default]
    Smooth,
}

impl Falloff {
    pub const ALL: [Self; 3] = [Self::Flat, Self::Linear, Self::Smooth];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Flat => "Flat",
            Self::Linear => "Linear",
            Self::Smooth => "Smooth",
        }
    }

    /// Strength `0..=1` at `distance` tiles from the centre of a brush of `radius`.
    pub fn weight(self, distance: f32, radius: u8) -> f32 {
        if radius == 0 {
            return 1.0;
        }
        let t = (distance / (f32::from(radius) + 1.0)).clamp(0.0, 1.0);
        match self {
            Self::Flat => 1.0,
            Self::Linear => 1.0 - t,
            Self::Smooth => {
                let s = 1.0 - t;
                s * s * (3.0 - 2.0 * s)
            }
        }
    }
}

/// The Height tool's settings, persisted with the editor settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HeightTool {
    pub op: HeightOp,
    /// Client units moved per application at full weight (Raise, Lower, Roughen), or the blend
    /// strength out of 64 (Smooth).
    pub strength: i32,
    pub falloff: Falloff,
    /// Flatten to this client height; `None` flattens to the height under the cursor at the start of
    /// the stroke.
    pub flatten_to: Option<i32>,
}

impl Default for HeightTool {
    fn default() -> Self {
        Self {
            op: HeightOp::Raise,
            strength: 16,
            falloff: Falloff::Smooth,
            flatten_to: None,
        }
    }
}

/// The nearest multiple of 8.
pub fn round8(value: f32) -> i32 {
    ((value / 8.0).round() as i32) * 8
}

/// Deterministic noise in `-1..=1` from a tile and a stroke seed (so the result of a stroke does not
/// change when it is previewed or repeated).
fn noise(tile: (i32, i32), seed: u64) -> f32 {
    let mut x = (tile.0 as i64 as u64).wrapping_mul(0x9E37_79B1_85EB_CA87)
        ^ (tile.1 as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ seed;
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    ((x >> 40) as f32 / (1u64 << 23) as f32) - 1.0
}

/// The new heights for one application of the tool at `center`.
///
/// `height` gives a tile's current height (`None` outside loaded data); `anchor` is the height under
/// the cursor when the stroke began. Only tiles whose height changes are returned.
pub fn apply(
    tool: &HeightTool,
    center: (i32, i32),
    radius: u8,
    shape: BrushShape,
    anchor: i32,
    seed: u64,
    height: &dyn Fn((i32, i32)) -> Option<i32>,
) -> Vec<((i32, i32), i32)> {
    let target = tool.flatten_to.unwrap_or(anchor);
    let mut changes = Vec::new();
    for tile in footprint(radius, shape, center) {
        let Some(current) = height(tile) else {
            continue;
        };
        let distance = (((tile.0 - center.0).pow(2) + (tile.1 - center.1).pow(2)) as f32).sqrt();
        let weight = tool.falloff.weight(distance, radius);
        let strength = tool.strength as f32;
        let new = match tool.op {
            // Up is negative.
            HeightOp::Raise => current - round8(strength * weight),
            HeightOp::Lower => current + round8(strength * weight),
            HeightOp::Smooth => {
                let mut sum = 0.0;
                let mut count = 0.0;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        if let Some(h) = height((tile.0 + dx, tile.1 + dy)) {
                            sum += h as f32;
                            count += 1.0;
                        }
                    }
                }
                let average = if count > 0.0 {
                    sum / count
                } else {
                    current as f32
                };
                let blend = (strength / 64.0).clamp(0.0, 1.0) * weight;
                round8(current as f32 + (average - current as f32) * blend)
            }
            HeightOp::Flatten => {
                let blend = if tool.falloff == Falloff::Flat {
                    1.0
                } else {
                    weight
                };
                round8(current as f32 + (target - current) as f32 * blend)
            }
            HeightOp::Roughen => current + round8(noise(tile, seed) * strength * weight),
        };
        if new != current {
            changes.push((tile, new));
        }
    }
    changes
}

/// The byte the map file stores for a tile at `height` when the plane below is at `base` (`0` for plane 0):
/// the height is `base - 8 * byte`, and a stored `1` reads as `0`.
pub fn encode_height(base: i32, height: i32) -> u8 {
    let steps = ((base - height + 4).div_euclid(8)).clamp(0, 255);
    // A stored 1 is read as 0, so the nearest storable neighbours of one step are 0 and 2.
    match steps {
        1 => 0,
        steps => steps as u8,
    }
}

/// The height a stored byte reads back as.
pub fn decode_height(base: i32, byte: u8) -> i32 {
    let steps = if byte == 1 { 0 } else { i32::from(byte) };
    base - steps * 8
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn flat(height: i32) -> impl Fn((i32, i32)) -> Option<i32> {
        move |_| Some(height)
    }

    #[test]
    fn raising_moves_up_which_is_negative() {
        let tool = HeightTool {
            op: HeightOp::Raise,
            strength: 16,
            falloff: Falloff::Flat,
            flatten_to: None,
        };
        let out = apply(&tool, (0, 0), 0, BrushShape::Square, 0, 1, &flat(-40));
        assert_eq!(out, vec![((0, 0), -56)]);
        let lower = HeightTool {
            op: HeightOp::Lower,
            ..tool
        };
        let out = apply(&lower, (0, 0), 0, BrushShape::Square, 0, 1, &flat(-40));
        assert_eq!(out, vec![((0, 0), -24)]);
    }

    #[test]
    fn falloff_is_one_at_the_centre_and_fades_to_the_edge() {
        for falloff in [Falloff::Linear, Falloff::Smooth] {
            assert!((falloff.weight(0.0, 4) - 1.0).abs() < 1e-6);
            assert!(falloff.weight(2.0, 4) < 1.0);
            assert!(falloff.weight(5.0, 4) < 1e-6);
        }
        assert_eq!(Falloff::Flat.weight(4.0, 4), 1.0);
        assert_eq!(Falloff::Smooth.weight(3.0, 0), 1.0);
    }

    #[test]
    fn flatten_levels_towards_the_anchor_and_the_edge_keeps_more() {
        let heights: HashMap<(i32, i32), i32> = (-3..=3)
            .flat_map(|x| (-3..=3).map(move |y| ((x, y), -64)))
            .collect();
        let lookup = |tile| heights.get(&tile).copied();
        let tool = HeightTool {
            op: HeightOp::Flatten,
            strength: 16,
            falloff: Falloff::Linear,
            flatten_to: None,
        };
        let out: HashMap<_, _> = apply(&tool, (0, 0), 2, BrushShape::Square, 0, 1, &lookup)
            .into_iter()
            .collect();
        // The centre goes all the way to the anchor (0); tiles further out keep part of -64.
        assert_eq!(out[&(0, 0)], 0);
        assert!(out[&(2, 0)] < out[&(0, 0)]);
        let fixed = HeightTool {
            flatten_to: Some(-200),
            falloff: Falloff::Flat,
            ..tool
        };
        let out = apply(&fixed, (0, 0), 1, BrushShape::Square, 0, 1, &lookup);
        assert!(out.iter().all(|(_, h)| *h == -200));
    }

    #[test]
    fn smoothing_pulls_a_spike_towards_its_neighbours() {
        let lookup = |tile: (i32, i32)| Some(if tile == (0, 0) { -400 } else { 0 });
        let tool = HeightTool {
            op: HeightOp::Smooth,
            strength: 64,
            falloff: Falloff::Flat,
            flatten_to: None,
        };
        let out = apply(&tool, (0, 0), 0, BrushShape::Square, 0, 1, &lookup);
        assert_eq!(out.len(), 1);
        assert!(out[0].1 > -400 && out[0].1 <= 0);
    }

    #[test]
    fn roughening_is_deterministic_per_seed() {
        let tool = HeightTool {
            op: HeightOp::Roughen,
            strength: 64,
            falloff: Falloff::Flat,
            flatten_to: None,
        };
        let a = apply(&tool, (5, 5), 3, BrushShape::Square, 0, 7, &flat(0));
        let b = apply(&tool, (5, 5), 3, BrushShape::Square, 0, 7, &flat(0));
        let c = apply(&tool, (5, 5), 3, BrushShape::Square, 0, 8, &flat(0));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.iter().all(|(_, h)| h % 8 == 0 && h.abs() <= 64));
    }

    #[test]
    fn encoded_heights_round_trip_and_skip_the_quirky_one() {
        for base in [0, -240, -1000] {
            for step in [0, 2, 3, 40, 255] {
                let height = base - step * 8;
                assert_eq!(decode_height(base, encode_height(base, height)), height);
            }
        }
        // One step is not storable: the client reads a stored 1 as 0.
        assert_eq!(decode_height(0, encode_height(0, -8)), 0);
        // Out of range clamps.
        assert_eq!(encode_height(0, 500), 0);
        assert_eq!(encode_height(0, -99_999), 255);
    }
}
