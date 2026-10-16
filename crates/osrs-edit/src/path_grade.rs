//! Smoothing the ground along a laid path. Real paths follow the ground and are not levelled across
//! (see `docs/verification/PATH-CENSUS.md`), so this keeps the ground's cross-slope and only removes
//! bumps and too-steep stretches *along* the path: the whole cross-section moves together.
//!
//! Pure and deterministic. The result depends only on the centre line, the spec and the heights
//! the caller supplies, so the editor's preview and the committed patch agree exactly when they
//! read the same heights. Heights follow the map convention: client units, up is negative.
//!
//! Method: sample the terrain along the centre line (every half tile), smooth that profile, limit
//! its slope with a Lipschitz envelope (the average of the upper and lower envelopes, so it stays
//! close to the smoothed profile), then shift every vertex within `half-width + blend` of the line
//! by the correction (smoothed minus raw) at its nearest point on the line, fading to nothing
//! across the blend.

use std::collections::BTreeMap;

/// Distance between profile samples along the centre line, in tiles.
const STEP: f32 = 0.5;
/// How many times the box filter runs; three passes approximate a Gaussian.
const BLUR_PASSES: usize = 3;
/// Limits of each parameter.
pub const MAX_SMOOTHING_TILES: f32 = 40.0;
pub const MAX_BLEND_TILES: f32 = 30.0;
pub const MIN_SLOPE: f32 = 4.0;
pub const MAX_SLOPE: f32 = 128.0;

/// A point of the path's centre line and the half-width of the path there, in tiles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradePoint {
    pub x: f32,
    pub y: f32,
    pub half: f32,
}

/// How the ground is smoothed along a path. The defaults come from the real map: its paths have a
/// steepest-10% slope of about 40 units per tile (128 is a whole tile), so that is not "too steep".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathGradeSpec {
    /// Length over which the profile along the path is smoothed (`0` keeps it as it is).
    pub smoothing_tiles: f32,
    /// Steepest allowed change along the path, in client units per tile.
    pub max_slope: f32,
    /// Distance beyond the path edge over which the correction fades out.
    pub blend_tiles: f32,
}

impl Default for PathGradeSpec {
    fn default() -> Self {
        Self {
            smoothing_tiles: 8.0,
            max_slope: 40.0,
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

/// The line resampled every [`STEP`] tiles, with the half-width interpolated.
fn resample(line: &[GradePoint]) -> Vec<GradePoint> {
    let mut out = vec![line[0]];
    let mut carry = 0.0_f32;
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
        if length <= f32::EPSILON {
            continue;
        }
        let mut at = STEP - carry;
        while at <= length {
            let t = at / length;
            out.push(GradePoint {
                x: a.x + (b.x - a.x) * t,
                y: a.y + (b.y - a.y) * t,
                half: a.half + (b.half - a.half) * t,
            });
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

/// Nearest point on the sampled line: its distance, the interpolated correction and half-width.
fn nearest(samples: &[GradePoint], correction: &[f32], point: (f32, f32)) -> (f32, f32, f32) {
    if samples.len() == 1 {
        let d = ((point.0 - samples[0].x).powi(2) + (point.1 - samples[0].y).powi(2)).sqrt();
        return (d, correction[0], samples[0].half);
    }
    let mut best = (f32::INFINITY, correction[0], samples[0].half);
    for (i, pair) in samples.windows(2).enumerate() {
        let (a, b) = (pair[0], pair[1]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let length_squared = dx * dx + dy * dy;
        let t = if length_squared <= f32::EPSILON {
            0.0
        } else {
            (((point.0 - a.x) * dx + (point.1 - a.y) * dy) / length_squared).clamp(0.0, 1.0)
        };
        let (cx, cy) = (a.x + dx * t, a.y + dy * t);
        let d = ((point.0 - cx).powi(2) + (point.1 - cy).powi(2)).sqrt();
        if d < best.0 {
            best = (
                d,
                correction[i] + (correction[i + 1] - correction[i]) * t,
                a.half + (b.half - a.half) * t,
            );
        }
    }
    best
}

/// The new height of every vertex that smoothing the ground along a path would change, sorted by
/// vertex.
///
/// `height` gives a vertex's current height (`None` where unknown). If any profile sample's ground
/// is unknown the result is empty: this never guesses a height. Only vertices whose height changes
/// are returned.
pub fn plan_path_grade(
    line: &[GradePoint],
    spec: PathGradeSpec,
    height: &dyn Fn((i32, i32)) -> Option<i32>,
) -> Vec<((i32, i32), i32)> {
    if line.is_empty()
        || line
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite() || !p.half.is_finite() || p.half <= 0.0)
    {
        return Vec::new();
    }
    let spec = spec.clamped();
    let samples = resample(line);
    let mut raw = Vec::with_capacity(samples.len());
    for sample in &samples {
        let Some(ground) = terrain_at((sample.x, sample.y), height) else {
            return Vec::new();
        };
        raw.push(ground);
    }
    let mut bed = raw.clone();
    blur(
        &mut bed,
        (spec.smoothing_tiles / STEP / 2.0).round() as usize,
    );
    limit_slope(&mut bed, spec.max_slope * STEP);
    let correction: Vec<f32> = bed.iter().zip(&raw).map(|(bed, raw)| bed - raw).collect();

    let widest = samples.iter().map(|p| p.half).fold(0.0_f32, f32::max);
    let reach = widest + spec.blend_tiles;
    let (mut min, mut max) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for p in &samples {
        min = (min.0.min(p.x), min.1.min(p.y));
        max = (max.0.max(p.x), max.1.max(p.y));
    }
    let mut changes = BTreeMap::new();
    for x in (min.0 - reach).floor() as i32..=(max.0 + reach).ceil() as i32 {
        for y in (min.1 - reach).floor() as i32..=(max.1 + reach).ceil() as i32 {
            let Some(current) = height((x, y)) else {
                continue;
            };
            let (distance, shift, half) = nearest(&samples, &correction, (x as f32, y as f32));
            let weight = if distance <= half {
                1.0
            } else if spec.blend_tiles <= 0.0 || distance >= half + spec.blend_tiles {
                continue;
            } else {
                1.0 - smoothstep((distance - half) / spec.blend_tiles)
            };
            let new = round8(current as f32 + shift * weight);
            if new != current {
                changes.insert((x, y), new);
            }
        }
    }
    changes.into_iter().collect()
}

#[cfg(test)]
mod tests;
