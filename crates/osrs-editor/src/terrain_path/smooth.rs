//! The centre line of a path: control points with their own width and a corner flag, turned into a
//! dense polyline that carries the local half-width.

/// One control point of a path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPoint {
    pub x: f32,
    pub y: f32,
    /// Full width of the path at this point, in tiles.
    pub width: f32,
    /// A corner keeps the path from curving through this point: the line bends sharply here.
    pub corner: bool,
}

impl ControlPoint {
    pub const fn new(x: f32, y: f32, width: f32) -> Self {
        Self {
            x,
            y,
            width,
            corner: false,
        }
    }
}

/// A point of the dense centre line with the half-width of the path there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathSample {
    pub x: f32,
    pub y: f32,
    pub half: f32,
}

const STEPS: usize = 12;
const MIN_HALF_WIDTH: f32 = 0.25;

/// The smooth centre line through `points`: a Catmull-Rom curve (clamped ends, twelve samples per
/// segment) in each run between corners, with the width interpolated along it. Two points are a
/// straight segment; corners join runs without rounding. Non-finite widths fall back to one tile.
pub fn smooth_path(points: &[ControlPoint]) -> Vec<PathSample> {
    let half = |width: f32| {
        if width.is_finite() {
            (width / 2.0).max(MIN_HALF_WIDTH)
        } else {
            0.5
        }
    };
    if points.len() == 1 {
        let p = points[0];
        return vec![PathSample {
            x: p.x,
            y: p.y,
            half: half(p.width),
        }];
    }
    let mut out = Vec::with_capacity((points.len().saturating_sub(1)) * STEPS + 1);
    let mut start = 0;
    for end in 1..points.len() {
        let run_ends = end == points.len() - 1 || points[end].corner;
        if !run_ends {
            continue;
        }
        run(&points[start..=end], &mut out, half);
        out.pop(); // the next run starts at this point
        start = end;
    }
    let last = points[points.len() - 1];
    out.push(PathSample {
        x: last.x,
        y: last.y,
        half: half(last.width),
    });
    out
}

/// One corner-free run: samples from its first point up to, not including, its last.
fn run(points: &[ControlPoint], out: &mut Vec<PathSample>, half: impl Fn(f32) -> f32) {
    for i in 0..points.len() - 1 {
        let p0 = points[i.saturating_sub(1)];
        let (p1, p2) = (points[i], points[i + 1]);
        let p3 = points[(i + 2).min(points.len() - 1)];
        let (h1, h2) = (half(p1.width), half(p2.width));
        for step in 0..STEPS {
            let t = step as f32 / STEPS as f32;
            let (t2, t3) = (t * t, t * t * t);
            out.push(PathSample {
                x: catmull(p0.x, p1.x, p2.x, p3.x, t, t2, t3),
                y: catmull(p0.y, p1.y, p2.y, p3.y, t, t2, t3),
                half: h1 + (h2 - h1) * t,
            });
        }
    }
    let last = points[points.len() - 1];
    out.push(PathSample {
        x: last.x,
        y: last.y,
        half: half(last.width),
    });
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
}
