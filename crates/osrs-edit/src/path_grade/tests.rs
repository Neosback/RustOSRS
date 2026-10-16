use super::*;

type Heights = BTreeMap<(i32, i32), i32>;

/// A field of heights `f(x, y)` over a square, `None` outside it.
fn field(size: i32, f: impl Fn(i32, i32) -> i32) -> Heights {
    let mut map = Heights::new();
    for x in -size..=size {
        for y in -size..=size {
            map.insert((x, y), f(x, y));
        }
    }
    map
}

fn grade(line: &[(f32, f32)], width: f32, spec: PathGradeSpec, heights: &Heights) -> Heights {
    plan_path_grade(line, width, spec, &|v| heights.get(&v).copied())
        .into_iter()
        .collect()
}

fn apply(heights: &Heights, changes: &Heights) -> Heights {
    let mut out = heights.clone();
    out.extend(changes.iter().map(|(k, v)| (*k, *v)));
    out
}

const LINE: [(f32, f32); 2] = [(-20.0, 0.0), (20.0, 0.0)];

#[test]
fn flat_ground_needs_no_grading() {
    let heights = field(40, |_, _| -64);
    assert!(grade(&LINE, 3.0, PathGradeSpec::default(), &heights).is_empty());
}

#[test]
fn a_road_across_a_hillside_is_flat_across_its_width() {
    // The ground rises 16 units per tile toward +y: across the road the bed must be level.
    let heights = field(40, |_, y| -16 * y);
    let changes = grade(&LINE, 5.0, PathGradeSpec::default(), &heights);
    let after = apply(&heights, &changes);
    let across: Vec<i32> = (-2..=2).map(|y| after[&(0, y)]).collect();
    assert!(across.iter().all(|h| *h == across[0]), "{across:?}");
    // Outside the blend the terrain is untouched.
    assert_eq!(after[&(0, 20)], heights[&(0, 20)]);
    assert_eq!(after[&(0, -20)], heights[&(0, -20)]);
}

#[test]
fn the_bed_never_climbs_faster_than_the_slope_limit() {
    // A steep ramp along the path: 100 units per tile, limit 24.
    let heights = field(40, |x, _| -100 * (x / 2));
    let spec = PathGradeSpec {
        smoothing_tiles: 0.0,
        max_slope: 24.0,
        blend_tiles: 3.0,
    };
    let after = apply(&heights, &grade(&LINE, 3.0, spec, &heights));
    for x in -19..=19 {
        let step = (after[&(x + 1, 0)] - after[&(x, 0)]).abs();
        // 24 per tile plus the 8-unit quantisation on each end.
        assert!(step <= 24 + 16, "x {x}: step {step}");
    }
}

#[test]
fn the_falloff_is_monotonic_and_reaches_zero_at_the_blend_distance() {
    let heights = field(40, |_, y| if y < 0 { 0 } else { -400 });
    // The path runs along y = 0 on a step of terrain: the bed sits between the two levels.
    let spec = PathGradeSpec {
        smoothing_tiles: 0.0,
        max_slope: MAX_SLOPE,
        blend_tiles: 6.0,
    };
    let changes = grade(&LINE, 2.0, spec, &heights);
    let after = apply(&heights, &changes);
    // Beyond width/2 + blend nothing changes.
    for y in 8..=12 {
        assert_eq!(after[&(0, y)], heights[&(0, y)], "y {y}");
    }
    // Moving away from the path the change shrinks (never grows) on each side.
    let delta = |y: i32| (after[&(0, y)] - heights[&(0, y)]).abs();
    for y in 1..7 {
        assert!(
            delta(y + 1) <= delta(y) + 8,
            "y {y}: {} then {}",
            delta(y),
            delta(y + 1)
        );
    }
}

#[test]
fn result_is_deterministic_sorted_and_quantised() {
    let heights = field(40, |x, y| -((x * 7 + y * 13) % 90));
    let a = plan_path_grade(&LINE, 4.0, PathGradeSpec::default(), &|v| {
        heights.get(&v).copied()
    });
    let b = plan_path_grade(&LINE, 4.0, PathGradeSpec::default(), &|v| {
        heights.get(&v).copied()
    });
    assert_eq!(a, b);
    assert!(a.windows(2).all(|pair| pair[0].0 < pair[1].0));
    assert!(a.iter().all(|(_, h)| h % 8 == 0));
}

#[test]
fn unknown_ground_under_the_path_grades_nothing() {
    let mut heights = field(40, |_, y| -16 * y);
    heights.remove(&(0, 0));
    heights.remove(&(1, 0));
    heights.remove(&(0, 1));
    heights.remove(&(1, 1));
    assert!(grade(&LINE, 3.0, PathGradeSpec::default(), &heights).is_empty());
}

#[test]
fn degenerate_input_is_rejected_without_panicking() {
    let heights = field(10, |_, _| 0);
    let spec = PathGradeSpec::default();
    let go = |line: &[(f32, f32)], width: f32| {
        plan_path_grade(line, width, spec, &|v| heights.get(&v).copied())
    };
    assert!(go(&[], 3.0).is_empty());
    assert!(go(&LINE, 0.0).is_empty());
    assert!(go(&LINE, f32::NAN).is_empty());
    assert!(go(&[(f32::NAN, 0.0), (1.0, 1.0)], 3.0).is_empty());
    // A single point is a valid, tiny path.
    let _ = go(&[(0.0, 0.0)], 3.0);
}

#[test]
fn smoothing_flattens_bumps_along_the_path() {
    let bumpy = field(40, |x, _| if x % 6 == 0 { -96 } else { 0 });
    let rough = PathGradeSpec {
        smoothing_tiles: 0.0,
        max_slope: MAX_SLOPE,
        blend_tiles: 2.0,
    };
    let smooth = PathGradeSpec {
        smoothing_tiles: 12.0,
        ..rough
    };
    let variation = |spec| {
        let after = apply(&bumpy, &grade(&LINE, 3.0, spec, &bumpy));
        let values: Vec<i32> = (-15..=15).map(|x| after[&(x, 0)]).collect();
        values.iter().max().copied().unwrap_or(0) - values.iter().min().copied().unwrap_or(0)
    };
    assert!(variation(smooth) < variation(rough));
}
