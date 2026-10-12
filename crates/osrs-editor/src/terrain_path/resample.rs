//! Smoothing and resampling a drawn line into evenly spaced samples, and cubic Bezier sampling.

use super::model::PathPoint;

const MIN_DISTANCE: f64 = 1.0e-9;

fn lerp(a: PathPoint, b: PathPoint, t: f64) -> PathPoint {
    PathPoint::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// `point` with consecutive duplicates removed.
fn dedupe(points: &[PathPoint]) -> Vec<PathPoint> {
    let mut out: Vec<PathPoint> = Vec::with_capacity(points.len());
    for point in points {
        if out
            .last()
            .is_none_or(|last| last.distance_to(*point) > MIN_DISTANCE)
        {
            out.push(*point);
        }
    }
    out
}

fn reflect(a: PathPoint, b: PathPoint) -> PathPoint {
    PathPoint::new(a.x * 2.0 - b.x, a.y * 2.0 - b.y)
}

/// Centripetal parameter step: the square root of the distance.
fn parameter_step(a: PathPoint, b: PathPoint) -> f64 {
    a.distance_to(b).sqrt().max(1.0e-9)
}

fn blend(a: PathPoint, b: PathPoint, ta: f64, tb: f64, t: f64) -> PathPoint {
    if (tb - ta).abs() < 1.0e-9 {
        a
    } else {
        lerp(a, b, (t - ta) / (tb - ta))
    }
}

const SHARP_START: f64 = std::f64::consts::PI / 6.0;
const SHARP_END: f64 = std::f64::consts::PI * 2.0 / 3.0;

/// Round off the drawn corners (`smoothness` 0 to 1; corners sharper than 120 degrees stay sharp)
/// with a centripetal Catmull-Rom curve, then space the samples `spacing` apart.
pub fn smooth_and_resample(points: &[PathPoint], smoothness: f64, spacing: f64) -> Vec<PathPoint> {
    if points.is_empty() {
        return Vec::new();
    }
    if points.len() == 1 {
        return vec![points[0]];
    }
    let smoothness = smoothness.clamp(0.0, 1.0);
    let spacing = spacing.max(1.0e-6);
    let pts = dedupe(points);
    if pts.len() == 1 {
        return pts;
    }
    if smoothness == 0.0 {
        return resample(&pts, spacing);
    }
    // How much of the curve each point takes: less at sharp corners.
    let mut alpha = vec![smoothness; pts.len()];
    for i in 1..pts.len() - 1 {
        let (a, b, c) = (pts[i - 1], pts[i], pts[i + 1]);
        let (ux, uy) = (b.x - a.x, b.y - a.y);
        let (vx, vy) = (c.x - b.x, c.y - b.y);
        let norm = ux.hypot(uy) * vx.hypot(vy);
        if norm < 1.0e-9 {
            continue;
        }
        let cos = ((ux * vx + uy * vy) / norm).clamp(-1.0, 1.0);
        let turn = cos.acos();
        let sharp = if turn <= SHARP_START {
            0.0
        } else if turn >= SHARP_END {
            1.0
        } else {
            let t = (turn - SHARP_START) / (SHARP_END - SHARP_START);
            t * t * (3.0 - t * 2.0)
        };
        alpha[i] = smoothness * (1.0 - sharp);
    }
    let mut out = vec![pts[0]];
    for i in 0..pts.len() - 1 {
        let (p1, p2) = (pts[i], pts[i + 1]);
        let p0 = if i == 0 { reflect(p1, p2) } else { pts[i - 1] };
        let p3 = if i + 2 >= pts.len() {
            reflect(p2, p1)
        } else {
            pts[i + 2]
        };
        let steps = ((p1.distance_to(p2) / spacing.min(0.25)).ceil() as usize).max(4);
        for k in 1..=steps {
            let u = k as f64 / steps as f64;
            let straight = lerp(p1, p2, u);
            let t1 = parameter_step(p0, p1);
            let t2 = t1 + parameter_step(p1, p2);
            let t3 = t2 + parameter_step(p2, p3);
            let t = t1 + u * (t2 - t1);
            let a1 = blend(p0, p1, 0.0, t1, t);
            let a2 = blend(p1, p2, t1, t2, t);
            let a3 = blend(p2, p3, t2, t3, t);
            let b1 = blend(a1, a2, 0.0, t2, t);
            let b2 = blend(a2, a3, t1, t3, t);
            let curve = blend(b1, b2, t1, t2, t);
            out.push(lerp(
                straight,
                curve,
                alpha[i] + (alpha[i + 1] - alpha[i]) * u,
            ));
        }
    }
    resample(&out, spacing)
}

/// The line through `points`, sampled every `spacing` along its length (the last point kept).
pub fn resample(points: &[PathPoint], spacing: f64) -> Vec<PathPoint> {
    let spacing = spacing.max(1.0e-6);
    let pts = dedupe(points);
    if pts.len() < 2 {
        return pts;
    }
    let mut out = vec![pts[0]];
    let mut remaining = spacing;
    let mut current = pts[0];
    for target in &pts[1..] {
        let mut distance = current.distance_to(*target);
        while distance > 1.0e-9 && distance + 1.0e-9 >= remaining {
            current = lerp(current, *target, remaining / distance);
            out.push(current);
            distance = current.distance_to(*target);
            remaining = spacing;
        }
        remaining -= distance;
        current = *target;
    }
    let last = pts[pts.len() - 1];
    if out[out.len() - 1].distance_to(last) > 1.0e-9 {
        out.push(last);
    }
    out
}

/// A point on the cubic Bezier through `p0`, `p1`, `p2`, `p3` at `t` (0 to 1).
pub fn bezier_point(
    p0: PathPoint,
    p1: PathPoint,
    p2: PathPoint,
    p3: PathPoint,
    t: f64,
) -> PathPoint {
    let t = t.clamp(0.0, 1.0);
    let u = 1.0 - t;
    let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    PathPoint::new(
        w0 * p0.x + w1 * p1.x + w2 * p2.x + w3 * p3.x,
        w0 * p0.y + w1 * p1.y + w2 * p2.y + w3 * p3.y,
    )
}

/// Samples of a cubic Bezier about `spacing` apart (the start point included when asked).
#[allow(dead_code, reason = "the Bezier knots of the spline path mode")]
pub fn bezier_sample(
    p0: PathPoint,
    p1: PathPoint,
    p2: PathPoint,
    p3: PathPoint,
    spacing: f64,
    include_start: bool,
) -> Vec<PathPoint> {
    let length = p0.distance_to(p1) + p1.distance_to(p2) + p2.distance_to(p3);
    let steps = ((length / spacing.max(1.0e-6)).ceil() as usize).clamp(6, 512);
    let mut out = Vec::with_capacity(steps + 1);
    if include_start {
        out.push(p0);
    }
    for k in 1..=steps {
        out.push(bezier_point(p0, p1, p2, p3, k as f64 / steps as f64));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_length(points: &[PathPoint]) -> f64 {
        points.windows(2).map(|w| w[0].distance_to(w[1])).sum()
    }

    #[test]
    fn resampling_spaces_the_samples_and_keeps_the_ends() {
        let line = [PathPoint::new(0.0, 0.0), PathPoint::new(10.0, 0.0)];
        let samples = resample(&line, 0.25);
        assert_eq!(samples.len(), 41);
        assert_eq!(samples[0], line[0]);
        assert_eq!(samples[samples.len() - 1], line[1]);
        assert!(
            samples
                .windows(2)
                .all(|w| (w[0].distance_to(w[1]) - 0.25).abs() < 1e-9)
        );
    }

    #[test]
    fn smoothing_keeps_the_ends_and_passes_through_the_points() {
        let line = [
            PathPoint::new(0.0, 0.0),
            PathPoint::new(5.0, 0.0),
            PathPoint::new(9.0, 3.0),
        ];
        let smooth = smooth_and_resample(&line, 0.65, 0.25);
        assert_eq!(smooth[0], line[0]);
        assert!(smooth[smooth.len() - 1].distance_to(line[2]) < 1e-9);
        // A Catmull-Rom curve passes through the points it is drawn through, evenly spaced.
        assert!(smooth.iter().any(|p| p.distance_to(line[1]) < 0.15));
        assert!(line_length(&smooth) >= line[0].distance_to(line[2]));
    }

    #[test]
    fn a_right_angle_stays_sharp() {
        let line = [
            PathPoint::new(0.0, 0.0),
            PathPoint::new(5.0, 0.0),
            PathPoint::new(5.0, 5.0),
        ];
        let smooth = smooth_and_resample(&line, 1.0, 0.25);
        let near_corner = smooth
            .iter()
            .any(|p| p.distance_to(PathPoint::new(5.0, 0.0)) < 0.3);
        assert!(near_corner);
    }

    #[test]
    fn bezier_ends_are_the_end_points() {
        let (a, b, c, d) = (
            PathPoint::new(0.0, 0.0),
            PathPoint::new(1.0, 2.0),
            PathPoint::new(3.0, 2.0),
            PathPoint::new(4.0, 0.0),
        );
        assert_eq!(bezier_point(a, b, c, d, 0.0), a);
        assert_eq!(bezier_point(a, b, c, d, 1.0), d);
    }
}
