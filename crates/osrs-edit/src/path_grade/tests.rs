use super::*;

type Heights = BTreeMap<(i32, i32), i32>;

/// A field of heights `f(x, y)` over a square.
fn field(size: i32, f: impl Fn(i32, i32) -> i32) -> Heights {
    let mut map = Heights::new();
    for x in -size..=size {
        for y in -size..=size {
            map.insert((x, y), f(x, y));
        }
    }
    map
}

fn straight(half: f32) -> Vec<GradePoint> {
    vec![
        GradePoint {
            x: -20.0,
            y: 0.0,
            half,
            weight: 1.0,
        },
        GradePoint {
            x: 20.0,
            y: 0.0,
            half,
            weight: 1.0,
        },
    ]
}

fn grade(line: &[GradePoint], spec: PathGradeSpec, heights: &Heights) -> Heights {
    plan_path_grade(line, spec, &|v| heights.get(&v).copied())
        .into_iter()
        .collect()
}

fn apply(heights: &Heights, changes: &Heights) -> Heights {
    let mut out = heights.clone();
    out.extend(changes.iter().map(|(k, v)| (*k, *v)));
    out
}

const NO_BLEND: PathGradeSpec = PathGradeSpec {
    smoothing_tiles: 8.0,
    max_slope: MAX_SLOPE,
    blend_tiles: 0.0,
};

#[test]
fn ground_that_is_already_smooth_is_left_exactly_alone() {
    let flat = field(40, |_, _| -64);
    assert!(grade(&straight(1.5), PathGradeSpec::default(), &flat).is_empty());
    // A constant slope along the path is a straight profile: nothing to smooth or limit.
    let ramp = field(40, |x, _| -8 * (x / 2) * 2);
    let after = apply(
        &ramp,
        &grade(&straight(1.5), PathGradeSpec::default(), &ramp),
    );
    for x in -15..=15 {
        assert!((after[&(x, 0)] - ramp[&(x, 0)]).abs() <= 8, "x {x}");
    }
}

#[test]
fn the_cross_slope_is_preserved_while_bumps_along_the_path_are_removed() {
    // Ground rises 16 units per tile toward +y (a cross-slope), with a bump every 6 tiles along x.
    let heights = field(40, |x, y| -16 * y + if x % 6 == 0 { -96 } else { 0 });
    let spec = PathGradeSpec {
        smoothing_tiles: 12.0,
        max_slope: MAX_SLOPE,
        blend_tiles: 3.0,
    };
    let after = apply(&heights, &grade(&straight(2.5), spec, &heights));
    // Along the centre the bumps are much smaller than before.
    let spread = |h: &Heights| {
        let values: Vec<i32> = (-15..=15).map(|x| h[&(x, 0)]).collect();
        values.iter().max().copied().unwrap_or(0) - values.iter().min().copied().unwrap_or(0)
    };
    assert!(
        spread(&after) < spread(&heights) / 2,
        "{} vs {}",
        spread(&after),
        spread(&heights)
    );
    // Across the path the slope is unchanged (within the 8-unit quantisation): the road is not levelled.
    for x in [-9, -3, 0, 4, 11] {
        let before = heights[&(x, 2)] - heights[&(x, -2)];
        let now = after[&(x, 2)] - after[&(x, -2)];
        assert!((now - before).abs() <= 16, "x {x}: {before} -> {now}");
    }
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
    let after = apply(&heights, &grade(&straight(1.5), spec, &heights));
    for x in -19..=19 {
        let step = (after[&(x + 1, 0)] - after[&(x, 0)]).abs();
        // 24 per tile plus the 8-unit quantisation on each end.
        assert!(step <= 24 + 16, "x {x}: step {step}");
    }
}

#[test]
fn the_correction_fades_to_nothing_across_the_blend_and_is_zero_beyond_it() {
    let heights = field(40, |x, _| if x % 6 == 0 { -200 } else { 0 });
    let spec = PathGradeSpec {
        smoothing_tiles: 12.0,
        max_slope: MAX_SLOPE,
        blend_tiles: 6.0,
    };
    let after = apply(&heights, &grade(&straight(1.0), spec, &heights));
    for y in 8..=12 {
        for x in -10..=10 {
            assert_eq!(
                after[&(x, y)],
                heights[&(x, y)],
                "({x}, {y}) is past half + blend"
            );
        }
    }
    // The change shrinks with distance from the path (monotonic up to quantisation).
    let change = |y: i32| {
        (-10..=10)
            .map(|x| (after[&(x, y)] - heights[&(x, y)]).abs())
            .max()
            .unwrap_or(0)
    };
    for y in 1..7 {
        assert!(
            change(y + 1) <= change(y) + 8,
            "y {y}: {} then {}",
            change(y),
            change(y + 1)
        );
    }
}

#[test]
fn a_wider_stretch_reaches_further_than_a_narrow_one() {
    let heights = field(40, |x, _| if x % 5 == 0 { -160 } else { 0 });
    let spec = PathGradeSpec {
        smoothing_tiles: 10.0,
        max_slope: MAX_SLOPE,
        blend_tiles: 0.0,
    };
    // Narrow (half 0.5) on the left half, wide (half 4) on the right half.
    let line = vec![
        GradePoint {
            x: -20.0,
            y: 0.0,
            half: 0.5,
            weight: 1.0,
        },
        GradePoint {
            x: -1.0,
            y: 0.0,
            half: 0.5,
            weight: 1.0,
        },
        GradePoint {
            x: 1.0,
            y: 0.0,
            half: 4.0,
            weight: 1.0,
        },
        GradePoint {
            x: 20.0,
            y: 0.0,
            half: 4.0,
            weight: 1.0,
        },
    ];
    let changes = grade(&line, spec, &heights);
    let reach = |range: std::ops::RangeInclusive<i32>| {
        changes
            .keys()
            .filter(|(x, _)| range.contains(x))
            .map(|(_, y)| y.abs())
            .max()
            .unwrap_or(0)
    };
    assert!(
        reach(5..=15) > reach(-15..=-5) + 2,
        "{} vs {}",
        reach(5..=15),
        reach(-15..=-5)
    );
}

#[test]
fn result_is_deterministic_sorted_and_quantised() {
    let heights = field(40, |x, y| -((x * 7 + y * 13) % 90));
    let go = || {
        plan_path_grade(&straight(2.0), PathGradeSpec::default(), &|v| {
            heights.get(&v).copied()
        })
    };
    let (a, b) = (go(), go());
    assert_eq!(a, b);
    assert!(a.windows(2).all(|pair| pair[0].0 < pair[1].0));
    assert!(a.iter().all(|(_, h)| h % 8 == 0));
}

#[test]
fn unknown_ground_under_the_path_changes_nothing() {
    let mut heights = field(40, |x, _| if x % 6 == 0 { -96 } else { 0 });
    for v in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        heights.remove(&v);
    }
    assert!(grade(&straight(1.5), NO_BLEND, &heights).is_empty());
}

#[test]
fn degenerate_input_is_rejected_without_panicking() {
    let heights = field(10, |_, _| 0);
    let go = |line: &[GradePoint]| {
        plan_path_grade(line, PathGradeSpec::default(), &|v| {
            heights.get(&v).copied()
        })
    };
    assert!(go(&[]).is_empty());
    assert!(go(&straight(0.0)).is_empty());
    assert!(go(&straight(f32::NAN)).is_empty());
    assert!(
        go(&[GradePoint {
            x: f32::NAN,
            y: 0.0,
            half: 1.0,
            weight: 1.0,
        }])
        .is_empty()
    );
    let _ = go(&[GradePoint {
        x: 0.0,
        y: 0.0,
        half: 1.0,
        weight: 1.0,
    }]);
}

#[test]
fn weight_zero_leaves_the_ground_and_a_partial_weight_is_in_between() {
    let bumpy = field(40, |x, _| if x % 6 == 0 { -96 } else { 0 });
    let mut line = straight(1.5);
    for point in &mut line {
        point.weight = 0.0;
    }
    assert!(grade(&line, NO_BLEND, &bumpy).is_empty());
    let full = grade(&straight(1.5), NO_BLEND, &bumpy);
    for point in &mut line {
        point.weight = 0.5;
    }
    let half = grade(&line, NO_BLEND, &bumpy);
    let size = |changes: &Heights| -> i64 {
        changes
            .iter()
            .map(|(v, h)| i64::from((h - bumpy[v]).abs()))
            .sum()
    };
    assert!(size(&full) > 0);
    let (full_size, half_size) = (size(&full), size(&half));
    assert!(
        half_size * 3 > full_size && half_size * 3 < full_size * 2 + full_size / 2,
        "half {half_size} of full {full_size}"
    );
}

#[test]
fn a_stretch_with_weight_zero_is_only_gradually_smoothed_at_its_edge() {
    let bumpy = field(40, |x, _| if x % 6 == 0 { -96 } else { 0 });
    let mut line = straight(1.5);
    line[0].weight = 0.0; // follow the ground at the start, smooth by the end
    let changes = grade(&line, NO_BLEND, &bumpy);
    let near_start: i64 = changes
        .iter()
        .filter(|(v, _)| v.0 < -14)
        .map(|(v, h)| i64::from((h - bumpy[v]).abs()))
        .sum();
    let near_end: i64 = changes
        .iter()
        .filter(|(v, _)| v.0 > 14)
        .map(|(v, h)| i64::from((h - bumpy[v]).abs()))
        .sum();
    assert!(near_start < near_end, "start {near_start} end {near_end}");
}
