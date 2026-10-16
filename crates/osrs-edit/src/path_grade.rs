//! Terrain grading under a laid path: a road bed that follows the ground along the path and is
//! flat across it, with a smooth falloff back to the original terrain.
//!
//! Pure and deterministic. The result depends only on the centre line, the width, the spec and the
//! heights the caller supplies, so the editor's preview and the committed patch agree exactly when
//! they read the same heights. Heights follow the map convention: client units, up is negative.
//!
//! Method: sample the terrain along the centre line (every half tile), smooth that profile, limit
//! its slope with a Lipschitz envelope (the average of the upper and lower envelopes, so it stays
//! close to the smoothed profile), then move every vertex within `width / 2 + blend` of the line
//! toward the bed height of its nearest point on the line.

use std::collections::BTreeMap;

/// Distance between profile samples along the centre line, in tiles.
const STEP: f32 = 0.5;
/// How many times the box filter runs; three passes approximate a Gaussian.
const BLUR_PASSES: usize = 3;
/// Limits of each parameter, shared by the planner and the editor's settings validation.
pub const MAX_SMOOTHING_TILES: f32 = 40.0;
pub const MAX_BLEND_TILES: f32 = 30.0;
pub const MIN_SLOPE: f32 = 4.0;
pub const MAX_SLOPE: f32 = 128.0;

/// How a path grades the terrain beneath it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathGradeSpec {
    /// Length over which the bed profile is smoothed along the path (`0` follows the ground).
    pub smoothing_tiles: f32,
    /// Steepest allowed change of the bed along the path, in client units per tile.
    pub max_slope: f32,
    /// Distance beyond the path edge over which the bed fades back into the terrain.
    pub blend_tiles: f32,
}

impl Default for PathGradeSpec {
    fn default() -> Self {
        Self {
            smoothing_tiles: 8.0,
            max_slope: 32.0,
            blend_tiles: 4.0,
        }
    }
}

impl PathGradeSpec {
    fn clamped(self) -> Self {
        let finite = |value: f32, fallback: f32| if value.is_finite() { value } else { fallback };
        Self {
            smoothing_tiles: finite(self.smoothing_tiles, 0.0).clamp(0.0, MAX_SMOOTHING_TILES),
            max_slope: finite(self.max_slope, MAX_SLOPE).clamp(MIN_SLOPE, MAX_SLOPE),
            blend_tiles: finite(self.blend_tiles, 0.0).clamp(0.0, MAX_BLEND_TILES),
        }
    }
}

/// The nearest multiple of 8, the Height tool's step.
fn round8(value: f32) -> i32 {
    ((value / 8.0).round() as i32) * 8
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The line resampled every [`STEP`] tiles, with each sample's position.
fn resample(line: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut out = vec![line[0]];
    let mut carry = 0.0_f32;
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        if length <= f32::EPSILON {
            continue;
        }
        let mut at = STEP - carry;
        while at <= length {
            let t = at / length;
            out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
            at += STEP;
        }
        carry = length - (at - STEP);
    }
    if let Some(&last) = line.last()
        && out.last() != Some(&last)
    {
        out.push(last);
    }
    out
}

/// Bilinear terrain height at a point in tile coordinates; `None` when a corner is unknown.
fn terrain_at(point: (f32, f32), height: &dyn Fn((i32, i32)) -> Option<i32>) -> Option<f32> {
    let (x0, y0) = (point.0.floor(), point.1.floor());
    let (u, v) = (point.0 - x0, point.1 - y0);
    let (x0, y0) = (x0 as i32, y0 as i32);
    let corner = |dx, dy| height((x0 + dx, y0 + dy)).map(|h| h as f32);
    let (sw, se, ne, nw) = (corner(0, 0)?, corner(1, 0)?, corner(1, 1)?, corner(0, 1)?);
    Some(sw * (1.0 - u) * (1.0 - v) + se * u * (1.0 - v) + ne * u * v + nw * (1.0 - u) * v)
}

/// Box-blur the profile `passes` times with a window of `radius` samples each side; the ends
/// repeat their value so the profile does not sag toward zero there.
fn blur(profile: &mut [f32], radius: usize) {
    if radius == 0 || profile.len() < 3 {
        return;
    }
    for _ in 0..BLUR_PASSES {
        let source = profile.to_vec();
        let last = source.len() - 1;
        for (i, out) in profile.iter_mut().enumerate() {
            let mut sum = 0.0;
            for k in 0..=2 * radius {
                sum += source[(i + k).saturating_sub(radius).min(last)];
            }
            *out = sum / (2 * radius + 1) as f32;
        }
    }
}

/// The mean of the upper and lower `max_step`-Lipschitz envelopes of `profile`. It is itself
/// `max_step`-Lipschitz and stays near the input where the input is already gentle.
fn limit_slope(profile: &mut [f32], max_step: f32) {
    if profile.len() < 2 {
        return;
    }
    let n = profile.len();
    let mut upper = profile.to_vec();
    let mut lower = profile.to_vec();
    for i in 1..n {
        upper[i] = upper[i].min(upper[i - 1] + max_step);
        lower[i] = lower[i].max(lower[i - 1] - max_step);
    }
    for i in (0..n - 1).rev() {
        upper[i] = upper[i].min(upper[i + 1] + max_step);
        lower[i] = lower[i].max(lower[i + 1] - max_step);
    }
    for i in 0..n {
        profile[i] = (upper[i] + lower[i]) / 2.0;
    }
}

/// Nearest point on the sampled line: its distance and the interpolated bed height there.
fn nearest(samples: &[(f32, f32)], bed: &[f32], point: (f32, f32)) -> (f32, f32) {
    if samples.len() == 1 {
        let d = ((point.0 - samples[0].0).powi(2) + (point.1 - samples[0].1).powi(2)).sqrt();
        return (d, bed[0]);
    }
    let mut best = (f32::INFINITY, bed[0]);
    for (i, pair) in samples.windows(2).enumerate() {
        let (a, b) = (pair[0], pair[1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length_squared = dx * dx + dy * dy;
        let t = if length_squared <= f32::EPSILON {
            0.0
        } else {
            (((point.0 - a.0) * dx + (point.1 - a.1) * dy) / length_squared).clamp(0.0, 1.0)
        };
        let (cx, cy) = (a.0 + dx * t, a.1 + dy * t);
        let d = ((point.0 - cx).powi(2) + (point.1 - cy).powi(2)).sqrt();
        if d < best.0 {
            best = (d, bed[i] + (bed[i + 1] - bed[i]) * t);
        }
    }
    best
}

/// The new height of every vertex a path of `width` tiles along `line` grades, sorted by vertex.
///
/// `height` gives a vertex's current height (`None` where unknown). If any profile sample's ground
/// is unknown the result is empty: grading never guesses a height. Only vertices whose height
/// changes are returned.
pub fn plan_path_grade(
    line: &[(f32, f32)],
    width: f32,
    spec: PathGradeSpec,
    height: &dyn Fn((i32, i32)) -> Option<i32>,
) -> Vec<((i32, i32), i32)> {
    if line.is_empty()
        || !width.is_finite()
        || width <= 0.0
        || line.iter().any(|p| !p.0.is_finite() || !p.1.is_finite())
    {
        return Vec::new();
    }
    let spec = spec.clamped();
    let samples = resample(line);
    let mut profile = Vec::with_capacity(samples.len());
    for sample in &samples {
        let Some(ground) = terrain_at(*sample, height) else {
            return Vec::new();
        };
        profile.push(ground);
    }
    blur(
        &mut profile,
        (spec.smoothing_tiles / STEP / 2.0).round() as usize,
    );
    limit_slope(&mut profile, spec.max_slope * STEP);

    let half = width / 2.0;
    let reach = half + spec.blend_tiles;
    let (mut min, mut max) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for p in &samples {
        min = (min.0.min(p.0), min.1.min(p.1));
        max = (max.0.max(p.0), max.1.max(p.1));
    }
    let mut changes = BTreeMap::new();
    for x in (min.0 - reach).floor() as i32..=(max.0 + reach).ceil() as i32 {
        for y in (min.1 - reach).floor() as i32..=(max.1 + reach).ceil() as i32 {
            let Some(current) = height((x, y)) else {
                continue;
            };
            let (distance, bed) = nearest(&samples, &profile, (x as f32, y as f32));
            let weight = if distance <= half {
                1.0
            } else if spec.blend_tiles <= 0.0 || distance >= reach {
                continue;
            } else {
                1.0 - smoothstep((distance - half) / spec.blend_tiles)
            };
            let target = current as f32 + (bed - current as f32) * weight;
            let new = round8(target);
            if new != current {
                changes.insert((x, y), new);
            }
        }
    }
    changes.into_iter().collect()
}

#[cfg(test)]
mod tests;
