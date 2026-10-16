//! Differential cases produced by the real SceneGraph.smoothPathHeights body, with UI/history
//! calls stubbed out. This tests the public semantic planner and patch builder, not a test-only port.
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{
    HeightVertexCoord, MapDocument, PathHeightSpec, build_path_height_patch,
    plan_path_height_smoothing,
};

fn vertex(x: i32, y: i32, z: u8) -> HeightVertexCoord {
    HeightVertexCoord::new(
        MapTile::new(x, y),
        SourcePlane::new(z).unwrap_or_else(|| unreachable!("valid fixture")),
    )
}

#[test]
fn eiken_smoothing_matches_every_height_on_every_plane() {
    let mut count = 0;
    let mut document = MapDocument::new();
    let mut original = document.clone();
    let mut line = Vec::new();
    let mut expected = Vec::new();
    let mut size = (0, 0);
    let mut plane = 0;
    let mut spec = PathHeightSpec {
        width: 3.0,
        passes: 5,
        blend: 4.0,
    };
    for row in include_str!("fixtures/eiken_heights.txt").lines() {
        let words: Vec<_> = row.split_whitespace().collect();
        match words[0] {
            "case" => {
                size = (
                    words[2]
                        .parse::<i32>()
                        .unwrap_or_else(|_| unreachable!("valid fixture")),
                    words[3]
                        .parse::<i32>()
                        .unwrap_or_else(|_| unreachable!("valid fixture")),
                );
                plane = words[4]
                    .parse()
                    .unwrap_or_else(|_| unreachable!("valid fixture"));
                spec = PathHeightSpec {
                    width: words[5]
                        .parse()
                        .unwrap_or_else(|_| unreachable!("valid fixture")),
                    passes: words[6]
                        .parse()
                        .unwrap_or_else(|_| unreachable!("valid fixture")),
                    blend: words[7]
                        .parse()
                        .unwrap_or_else(|_| unreachable!("valid fixture")),
                };
                let mut heights = Vec::new();
                for z in 0..4 {
                    for x in 0..size.0 {
                        for y in 0..size.1 {
                            heights.push((
                                vertex(x, y, z),
                                -40 - (x * 19 + y * 31) % 110 - i32::from(z) * 160,
                            ));
                        }
                    }
                }
                document = MapDocument::from_parts([], heights, []);
                original = document.clone();
                line.clear();
                expected.clear();
            }
            "point" => line.push((
                words[1]
                    .parse()
                    .unwrap_or_else(|_| unreachable!("valid fixture")),
                words[2]
                    .parse()
                    .unwrap_or_else(|_| unreachable!("valid fixture")),
            )),
            "height" => expected.push((
                vertex(
                    words[2]
                        .parse()
                        .unwrap_or_else(|_| unreachable!("valid fixture")),
                    words[3]
                        .parse()
                        .unwrap_or_else(|_| unreachable!("valid fixture")),
                    words[1]
                        .parse()
                        .unwrap_or_else(|_| unreachable!("valid fixture")),
                ),
                words[4]
                    .parse::<i32>()
                    .unwrap_or_else(|_| unreachable!("valid fixture")),
            )),
            "end" => {
                let smoothed = plan_path_height_smoothing(&line, spec, &|p| {
                    (p.0 >= 0 && p.1 >= 0 && p.0 < size.0 && p.1 < size.1)
                        .then(|| document.height_at(vertex(p.0, p.1, plane)))
                        .flatten()
                });
                let patch = build_path_height_patch(
                    &document,
                    SourcePlane::new(plane).unwrap_or_else(|| unreachable!("valid fixture")),
                    &smoothed,
                )
                .unwrap_or_else(|_| unreachable!("valid fixture"));
                document
                    .apply_patch(&patch)
                    .unwrap_or_else(|_| unreachable!("valid fixture"));
                for &(coord, height) in &expected {
                    assert_eq!(
                        document.height_at(coord),
                        Some(height),
                        "case {count}, {coord:?}"
                    );
                }
                document
                    .apply_patch(&patch.inverse())
                    .unwrap_or_else(|_| unreachable!("valid fixture"));
                assert_eq!(document, original, "case {count} undo");
                count += 1;
            }
            _ => panic!("unknown oracle row"),
        }
    }
    assert_eq!(count, 16);
}
