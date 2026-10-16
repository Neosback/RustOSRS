//! A route that wanders: smooth, seeded, deterministic sideways drift along a stretch of a path, so
//! a trail through a field is not ruler-straight. The method is the editor's `meander` idea (two
//! octaves of value noise along the arc, pushed perpendicular to the line, ends held in place)
//! taken from the EikenB00m map editor's coast shaper, applied per stretch between control points.

use super::smooth::PathSample;

/// How much a stretch between two control points wanders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Wander {
    #[default]
    Off,
    /// A slight, natural drift (about a tile).
    Gentle,
    /// A visible meander (about two tiles).
    Natural,
}

impl Wander {
    pub const fn amplitude(self) -> f32 {
        match self {
            Self::Off => 0.0,
            Self::Gentle => 0.9,
            Self::Natural => 1.8,
        }
    }

    /// The next setting when cycling Off, Gentle, Natural.
    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::Gentle,
            Self::Gentle => Self::Natural,
            Self::Natural => Self::Off,
        }
    }

    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Off => "Straight",
            Self::Gentle => "Gentle",
            Self::Natural => "Natural",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Straight",
            Self::Gentle => "Gentle wander",
            Self::Natural => "Natural wander",
        }
    }
}

/// Spacing of the resampled points the noise is applied to, in tiles.
const SPACING: f32 = 1.0;
/// Tiles over which the drift fades in from, and out to, the control points.
const FADE: f32 = 4.0;
/// Segments shorter than this get proportionally less drift, so a short hop does not zig-zag.
const FULL_AMPLITUDE_LENGTH: f32 = 12.0;

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn hash(i: i32, seed: u32) -> f32 {
    let mut h = (i as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ u64::from(seed).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    ((h & 0x00FF_FFFF) as f32 / 0x00FF_FFFF as f32) * 2.0 - 1.0
}

/// Smooth one-dimensional value noise in `[-1, 1]`.
fn noise(x: f32, seed: u32) -> f32 {
    let i = x.floor();
    let f = x - i;
    let (a, b) = (hash(i as i32, seed), hash(i as i32 + 1, seed));
    a + (b - a) * smoothstep(f)
}

/// The polyline `segment` (control point to control point, both included) with sideways drift, as
/// points from the start up to, not including, the end. The ends do not move. The drift depends
/// only on the arc length from the start and `seed`, so editing other stretches never changes it.
pub fn wander_segment(segment: &[PathSample], wander: Wander, seed: u32) -> Vec<PathSample> {
    let amplitude = wander.amplitude();
    let (Some(&first), Some(&last)) = (segment.first(), segment.last()) else {
        return Vec::new();
    };
    let lengths: Vec<f32> = segment
        .windows(2)
        .map(|w| ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt())
        .collect();
    let total: f32 = lengths.iter().sum();
    if amplitude <= 0.0 || total < 2.0 * SPACING || segment.len() < 2 {
        return segment[..segment.len().saturating_sub(1)].to_vec();
    }
    let amplitude = amplitude * (total / FULL_AMPLITUDE_LENGTH).min(1.0);
    // Resample at one-tile spacing, interpolating the half-width and the ground flag.
    let count = (total / SPACING).round().max(2.0) as usize;
    let at = |s: f32| -> (PathSample, (f32, f32)) {
        let mut remaining = s;
        for (i, length) in lengths.iter().enumerate() {
            if remaining <= *length || i == lengths.len() - 1 {
                let t = if *length > f32::EPSILON {
                    (remaining / length).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let (a, b) = (segment[i], segment[i + 1]);
                let direction = if *length > f32::EPSILON {
                    ((b.x - a.x) / length, (b.y - a.y) / length)
                } else {
                    (1.0, 0.0)
                };
                return (
                    PathSample {
                        x: a.x + (b.x - a.x) * t,
                        y: a.y + (b.y - a.y) * t,
                        half: a.half + (b.half - a.half) * t,
                        smooth: a.smooth,
                    },
                    direction,
                );
            }
            remaining -= length;
        }
        (last, (1.0, 0.0))
    };
    let mut out = Vec::with_capacity(count);
    out.push(first);
    for k in 1..count {
        let s = total * k as f32 / count as f32;
        let (mut sample, direction) = at(s);
        let fade = smoothstep(s.min(total - s) / FADE);
        let drift = (noise(s / 14.0, seed) * 0.7 + noise(s / 5.0, seed.wrapping_add(101)) * 0.3)
            * amplitude
            * fade;
        // Perpendicular to the local direction (left of travel).
        sample.x -= direction.1 * drift;
        sample.y += direction.0 * drift;
        out.push(sample);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn straight(length: f32) -> Vec<PathSample> {
        vec![
            PathSample {
                x: 0.0,
                y: 0.0,
                half: 1.5,
                smooth: false,
            },
            PathSample {
                x: length,
                y: 0.0,
                half: 1.5,
                smooth: false,
            },
        ]
    }

    #[test]
    fn off_changes_nothing() {
        let line = straight(30.0);
        assert_eq!(wander_segment(&line, Wander::Off, 7), vec![line[0]]);
    }

    #[test]
    fn the_ends_stay_put_and_the_drift_is_bounded_and_deterministic() {
        let line = straight(40.0);
        let a = wander_segment(&line, Wander::Natural, 3);
        assert_eq!(a, wander_segment(&line, Wander::Natural, 3));
        assert_eq!((a[0].x, a[0].y), (0.0, 0.0));
        // Near the start the drift has faded in; everywhere it stays within the amplitude.
        assert!(a[1].y.abs() < 0.5, "{}", a[1].y);
        assert!(
            a.iter()
                .all(|s| s.y.abs() <= Wander::Natural.amplitude() + 1e-3)
        );
        // It actually wanders somewhere in the middle.
        assert!(a.iter().any(|s| s.y.abs() > 0.4));
        // The last point returned is the one before the end, a tile away or less.
        let last = a[a.len() - 1];
        assert!(last.x > 38.0 && last.x < 40.0);
    }

    #[test]
    fn different_seeds_wander_differently_and_natural_wanders_more_than_gentle() {
        let line = straight(40.0);
        let reach = |wander, seed| {
            wander_segment(&line, wander, seed)
                .iter()
                .map(|s| s.y.abs())
                .fold(0.0_f32, f32::max)
        };
        assert_ne!(
            wander_segment(&line, Wander::Natural, 1),
            wander_segment(&line, Wander::Natural, 2)
        );
        assert!(reach(Wander::Natural, 5) > reach(Wander::Gentle, 5));
    }

    #[test]
    fn short_hops_barely_wander() {
        let a = wander_segment(&straight(3.0), Wander::Natural, 9);
        assert!(a.iter().all(|s| s.y.abs() < 0.5), "{a:?}");
        // Under two tiles there is nothing to resample.
        assert_eq!(wander_segment(&straight(1.5), Wander::Natural, 9).len(), 1);
    }

    #[test]
    fn the_drift_depends_only_on_arc_length_and_seed() {
        // The same stretch seen from a longer or shorter line drifts the same way where it overlaps.
        let a = wander_segment(&straight(40.0), Wander::Natural, 4);
        let b = wander_segment(&straight(44.0), Wander::Natural, 4);
        for k in 8..14 {
            assert!(
                (a[k].y - b[k].y).abs() < 0.6,
                "k {k}: {} vs {}",
                a[k].y,
                b[k].y
            );
        }
    }

    #[test]
    fn a_wandering_line_follows_the_direction_of_a_diagonal() {
        let line = vec![
            PathSample {
                x: 0.0,
                y: 0.0,
                half: 1.0,
                smooth: false,
            },
            PathSample {
                x: 30.0,
                y: 30.0,
                half: 1.0,
                smooth: false,
            },
        ];
        let a = wander_segment(&line, Wander::Natural, 6);
        // Drift is perpendicular: points stay within the amplitude of the diagonal y = x.
        assert!(a.iter().all(|s| ((s.x - s.y).abs() / 2.0_f32.sqrt()) <= Wander::Natural.amplitude() + 1e-3));
    }

    #[test]
    fn wander_cycles_and_names_every_setting() {
        assert_eq!(Wander::Off.next().next().next(), Wander::Off);
        let names: std::collections::BTreeSet<_> = [Wander::Off, Wander::Gentle, Wander::Natural]
            .map(Wander::label)
            .into();
        assert_eq!(names.len(), 3);
    }
}
