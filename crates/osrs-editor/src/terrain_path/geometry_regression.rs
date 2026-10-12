use super::model::PathPoint;
use super::resample::smooth_and_resample;

fn all_finite(points: &[PathPoint]) -> bool {
    points
        .iter()
        .all(|point| point.x.is_finite() && point.y.is_finite())
}

#[test]
fn near_coincident_control_points_do_not_create_runaway_samples() {
    let points = [
        PathPoint::new(0.0, 0.0),
        PathPoint::new(5.0e-10, 0.0),
        PathPoint::new(4.0, 0.0),
    ];

    let samples = smooth_and_resample(&points, 1.0, 0.1);

    assert_eq!(samples.first().copied(), Some(points[0]));
    assert_eq!(samples.last().copied(), Some(points[2]));
    assert!(all_finite(&samples));
    assert!(samples.len() < 100);
}

#[test]
fn exact_backtracking_control_points_remain_finite_and_bounded() {
    let points = [
        PathPoint::new(0.0, 0.0),
        PathPoint::new(4.0, 0.0),
        PathPoint::new(0.0, 0.0),
        PathPoint::new(4.0, 4.0),
    ];

    let samples = smooth_and_resample(&points, 1.0, 0.1);

    assert_eq!(samples.first().copied(), Some(points[0]));
    assert_eq!(samples.last().copied(), Some(points[3]));
    assert!(all_finite(&samples));
    assert!(samples.len() < 250);
}

#[test]
fn bow_tie_self_intersection_is_deterministic_and_bounded() {
    let points = [
        PathPoint::new(0.0, 0.0),
        PathPoint::new(4.0, 4.0),
        PathPoint::new(0.0, 4.0),
        PathPoint::new(4.0, 0.0),
    ];

    let first = smooth_and_resample(&points, 0.85, 0.1);
    let second = smooth_and_resample(&points, 0.85, 0.1);

    assert_eq!(first, second);
    assert_eq!(first.first().copied(), Some(points[0]));
    assert_eq!(first.last().copied(), Some(points[3]));
    assert!(all_finite(&first));
    assert!(first.len() < 250);
}
