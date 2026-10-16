//! The centre line of a path: control points with their own width and a corner flag, turned into a
//! dense polyline that carries the local half-width.

use super::wander::{Wander, wander_segment};

/// One control point of a path. Its width, wander and ground flag describe the path *from this
/// point to the next* (width is also interpolated toward the next point's width).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPoint {
    pub x: f32,
    pub y: f32,
    /// Full width of the path at this point, in tiles.
    pub width: f32,
    /// A corner keeps the path from curving through this point: the line bends sharply here.
    pub corner: bool,
    /// How the stretch after this point wanders.
    pub wander: Wander,
    /// Identity of this point's wander: it keeps its drift while other points move.
    pub seed: u32,
    /// Whether the ground is smoothed along the stretch after this point.
    pub smooth_ground: bool,
}

impl ControlPoint {
    pub const fn new(x: f32, y: f32, width: f32) -> Self {
        Self {
            x,
            y,
            width,
            corner: false,
            wander: Wander::Off,
            seed: 0,
            smooth_ground: false,
        }
    }
}

/// A point of the dense centre line with the half-width of the path there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathSample {
    pub x: f32,
    pub y: f32,
    pub half: f32,
    /// Whether the ground is smoothed here.
    pub smooth: bool,
}

const STEPS: usize = 12;
const MIN_HALF_WIDTH: f32 = 0.25;

fn half_of(width: f32) -> f32 {
    if width.is_finite() {
        (width / 2.0).max(MIN_HALF_WIDTH)
    } else {
        0.5
    }
}

fn sample_at(p: &ControlPoint) -> PathSample {
    PathSample {
        x: p.x,
        y: p.y,
        half: half_of(p.width),
        smooth: p.smooth_ground,
    }
}

/// The smooth centre line through `points`: a Catmull-Rom curve (clamped ends, twelve samples per
/// segment) in each run between corners, with the width interpolated along it. Two points are a
/// straight segment; corners join runs without rounding. Each segment then wanders as its start
/// point says. Non-finite widths fall back to one tile.
pub fn smooth_path(points: &[ControlPoint]) -> Vec<PathSample> {
    if points.len() == 1 {
        return vec![sample_at(&points[0])];
    }
    // One sample list per segment (from its start point, up to but not including its end).
    let mut segments: Vec<Vec<PathSample>> = vec![Vec::new(); points.len() - 1];
    let mut start = 0;
    for end in 1..points.len() {
        if end == points.len() - 1 || points[end].corner {
            run(&points[start..=end], &mut segments[start..end]);
            start = end;
        }
    }
    let mut out = Vec::with_capacity(points.len() * STEPS + 1);
    for (k, samples) in segments.into_iter().enumerate() {
        if points[k].wander == Wander::Off {
            out.extend(samples);
        } else {
            let mut stretch = samples;
            stretch.push(sample_at(&points[k + 1]));
            out.extend(wander_segment(&stretch, points[k].wander, points[k].seed));
        }
    }
    let mut last = sample_at(&points[points.len() - 1]);
    // The final point carries the last stretch's ground flag, so it is not a one-sample island.
    last.smooth = points[points.len() - 2].smooth_ground;
    out.push(last);
    out
}

/// One corner-free run: fills each of its segments with samples from the segment's start point up
/// to, not including, its end.
fn run(points: &[ControlPoint], segments: &mut [Vec<PathSample>]) {
    for i in 0..points.len() - 1 {
        let p0 = points[i.saturating_sub(1)];
        let (p1, p2) = (points[i], points[i + 1]);
        let p3 = points[(i + 2).min(points.len() - 1)];
        let (h1, h2) = (half_of(p1.width), half_of(p2.width));
        for step in 0..STEPS {
            let t = step as f32 / STEPS as f32;
            let (t2, t3) = (t * t, t * t * t);
            segments[i].push(PathSample {
                x: catmull(p0.x, p1.x, p2.x, p3.x, t, t2, t3),
                y: catmull(p0.y, p1.y, p2.y, p3.y, t, t2, t3),
                half: h1 + (h2 - h1) * t,
                smooth: p1.smooth_ground,
            });
        }
    }
}

fn catmull(p0: f32, p1: f32, p2: f32, p3: f32, t: f32, t2: f32, t3: f32) -> f32 {
    0.5 * (2.0 * p1
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(list: &[(f32, f32)], width: f32) -> Vec<ControlPoint> {
        list.iter()
            .map(|&(x, y)| ControlPoint::new(x, y, width))
            .collect()
    }

    #[test]
    fn two_points_are_a_straight_segment_with_interpolated_width() {
        let line = smooth_path(&[
            ControlPoint::new(0.0, 0.0, 2.0),
            ControlPoint::new(10.0, 0.0, 6.0),
        ]);
        assert_eq!(
            line.first().map(|s| (s.x, s.y, s.half)),
            Some((0.0, 0.0, 1.0))
        );
        assert_eq!(
            line.last().map(|s| (s.x, s.y, s.half)),
            Some((10.0, 0.0, 3.0))
        );
        assert!(line.iter().all(|s| s.y == 0.0));
        assert!(line.windows(2).all(|w| w[0].half <= w[1].half));
        assert_eq!(line.len(), STEPS + 1);
    }

    #[test]
    fn a_smooth_path_keeps_its_endpoints_and_sample_count() {
        let line = smooth_path(&pts(&[(0.0, 0.0), (2.0, 3.0), (6.0, 4.0)], 3.0));
        assert_eq!(line.len(), 2 * STEPS + 1);
        assert_eq!((line[0].x, line[0].y), (0.0, 0.0));
        let last = line[line.len() - 1];
        assert_eq!((last.x, last.y), (6.0, 4.0));
        // The curve passes through the middle control point.
        assert_eq!((line[STEPS].x, line[STEPS].y), (2.0, 3.0));
    }

    #[test]
    fn a_corner_bends_sharply_while_a_smooth_point_curves() {
        let mut points = pts(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0)], 1.0);
        let smooth = smooth_path(&points);
        points[1].corner = true;
        let corner = smooth_path(&points);
        // With a corner the line is two straight runs meeting at (4, 0): every sample of the
        // first run has y = 0 and every sample of the second has x = 4.
        assert!(corner[..=STEPS].iter().all(|s| s.y == 0.0));
        assert!(corner[STEPS..].iter().all(|s| s.x == 4.0));
        // Smooth overshoots the corner region instead.
        assert!(smooth[..=STEPS].iter().any(|s| s.y != 0.0));
        assert_eq!(corner.len(), smooth.len());
    }

    #[test]
    fn corner_flags_on_the_ends_change_nothing() {
        let mut points = pts(&[(0.0, 0.0), (4.0, 1.0), (8.0, 0.0)], 2.0);
        let plain = smooth_path(&points);
        points[0].corner = true;
        points[2].corner = true;
        assert_eq!(smooth_path(&points), plain);
    }

    #[test]
    fn degenerate_widths_fall_back_and_a_single_point_is_one_sample() {
        let line = smooth_path(&[
            ControlPoint::new(1.0, 1.0, f32::NAN),
            ControlPoint::new(3.0, 1.0, 0.0),
        ]);
        assert_eq!(line[0].half, 0.5);
        assert_eq!(line[line.len() - 1].half, MIN_HALF_WIDTH);
        assert_eq!(smooth_path(&[ControlPoint::new(2.0, 2.0, 3.0)]).len(), 1);
    }

    #[test]
    fn a_wandering_stretch_leaves_other_stretches_and_the_control_points_alone() {
        let mut points = pts(&[(0.0, 0.0), (30.0, 0.0), (60.0, 0.0)], 3.0);
        let plain = smooth_path(&points);
        points[0].wander = Wander::Natural;
        points[0].seed = 5;
        let wandered = smooth_path(&points);
        // The control points themselves do not move.
        let find = |line: &[PathSample], x: f32| line.iter().any(|s| s.x == x && s.y == 0.0);
        assert!(find(&wandered, 0.0) && find(&wandered, 30.0) && find(&wandered, 60.0));
        // The first stretch drifts; the second (which starts at point 1, not wandering) does not.
        assert!(
            wandered
                .iter()
                .take_while(|s| s.x < 30.0)
                .any(|s| s.y.abs() > 0.3)
        );
        let second: Vec<_> = wandered.iter().filter(|s| s.x > 30.0).collect();
        assert!(second.iter().all(|s| s.y == 0.0));
        // And the second stretch's samples are unchanged from the plain path.
        let plain_second: Vec<_> = plain.iter().filter(|s| s.x > 30.0).collect();
        assert_eq!(second.len(), plain_second.len());
    }

    #[test]
    fn the_ground_flag_follows_each_stretch() {
        let mut points = pts(&[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)], 3.0);
        points[0].smooth_ground = true;
        let line = smooth_path(&points);
        assert!(line.iter().filter(|s| s.x < 10.0).all(|s| s.smooth));
        assert!(line.iter().filter(|s| s.x >= 10.0).all(|s| !s.smooth));
    }
}
