use super::walls::{WallKit, emit_walls, generate_walls, rasterize_band};
use std::collections::HashSet;

fn dense_polyline(points: &[(f32, f32)], step: f32) -> Vec<(f32, f32)> {
    let mut line = Vec::new();
    for (segment_index, pair) in points.windows(2).enumerate() {
        let a = pair[0];
        let b = pair[1];
        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        let distance = (dx * dx + dy * dy).sqrt();
        let samples = (distance / step).ceil().max(1.0) as usize;
        for sample in 0..=samples {
            if segment_index > 0 && sample == 0 {
                continue;
            }
            let t = sample as f32 / samples as f32;
            line.push((a.0 + dx * t, a.1 + dy * t));
        }
    }
    line
}

#[test]
fn wide_winding_wall_band_is_deterministic_unique_and_bounded() {
    let points = [
        (4.5, 6.5),
        (36.5, 21.5),
        (68.5, 5.5),
        (100.5, 22.5),
        (132.5, 7.5),
    ];
    let line = dense_polyline(&points, 0.25);

    let first = generate_walls(&line, &points, 11.0, 7.0);
    let second = generate_walls(&line, &points, 11.0, 7.0);

    assert_eq!(first.tiles, second.tiles);
    assert_eq!(first.tangents, second.tangents);
    assert!(!first.tiles.is_empty());
    assert!(first.tiles.len() < 4_000);
    assert!(first.tiles.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(first.tangents.values().all(|tangent| {
        tangent.0.is_finite()
            && tangent.1.is_finite()
            && tangent.0.abs() <= 1.0
            && tangent.1.abs() <= 1.0
    }));

    let set: HashSet<_> = first.tiles.iter().copied().collect();
    let kit = WallKit {
        side: 100,
        top: 101,
        door: 0,
        diagonal: 200,
    };
    let specs = emit_walls(&set, &first.tangents, &kit, 3, 1);
    let repeat = emit_walls(&set, &first.tangents, &kit, 3, 1);

    assert_eq!(specs, repeat);
    assert!(!specs.is_empty());
    assert!(specs.iter().all(|spec| (1..=3).contains(&spec.plane)));
    assert!(specs.iter().all(|spec| spec.loc_type == 0 || spec.loc_type == 9));
    assert!(specs
        .iter()
        .all(|spec| matches!(spec.object_id, 100 | 101 | 200)));

    let mut unique = HashSet::new();
    assert!(specs.iter().all(|spec| unique.insert((
        spec.object_id,
        spec.tile,
        spec.plane,
        spec.loc_type,
        spec.orientation,
    ))));
}

#[test]
fn even_width_straight_band_keeps_exact_cross_section() {
    let line = dense_polyline(&[(4.5, 10.5), (60.5, 10.5)], 0.25);
    let band = rasterize_band(&line, 12.0);
    let ys: HashSet<_> = band.tiles.iter().map(|tile| tile.1).collect();

    assert_eq!(ys.len(), 12);
    assert_eq!(ys.iter().copied().min(), Some(4));
    assert_eq!(ys.iter().copied().max(), Some(15));
    assert!(band
        .tangents
        .values()
        .all(|tangent| tangent.0 > 0.99 && tangent.1.abs() < 1.0e-4));
}
