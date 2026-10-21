use super::*;
use std::collections::BTreeMap;

const REAL: &str = include_str!("real_roofs.json");

fn plane() -> SourcePlane {
    SourcePlane::new(1).unwrap_or_else(|| unreachable!("plane one is valid"))
}

fn assets() -> HipRoofAssets {
    HipRoofAssets::new(ObjectId::new(1924), ObjectId::new(1790))
}

fn rect(w: i32, h: i32) -> Vec<MapTile> {
    (0..h)
        .flat_map(|y| (0..w).map(move |x| MapTile::new(x, y)))
        .collect()
}

fn by_tile(pieces: Vec<ObjectPlacement>) -> BTreeMap<(i32, i32), (u32, u8, u8)> {
    pieces
        .into_iter()
        .map(|p| {
            (
                (p.position.tile.x, p.position.tile.y),
                (p.object_id.get(), p.kind.get(), p.orientation.get()),
            )
        })
        .collect()
}

/// A real roof from the fixture: the footprint it stands on and every piece's kind and
/// orientation by tile (south-west corner at the origin).
struct Real {
    name: String,
    label: String,
    choice: RoofChoice,
    assets: HipRoofAssets,
    footprint: Vec<MapTile>,
    pieces: BTreeMap<(i32, i32), (u8, u8)>,
}

fn real_roofs() -> Vec<Real> {
    let doc: serde_json::Value =
        serde_json::from_str(REAL).unwrap_or_else(|e| panic!("fixture: {e}"));
    let text = |v: &serde_json::Value| v.as_str().unwrap_or_default().to_owned();
    doc["roofs"]
        .as_array()
        .unwrap_or_else(|| panic!("roofs"))
        .iter()
        .map(|roof| {
            let rows: Vec<String> = roof["rows"]
                .as_array()
                .unwrap_or_else(|| panic!("rows"))
                .iter()
                .map(&text)
                .collect();
            let height = rows.len() as i32;
            let mut pieces = BTreeMap::new();
            for (row, line) in rows.iter().enumerate() {
                for (x, token) in line.split_whitespace().enumerate() {
                    if let Some((kind, orientation)) = token.split_once('/') {
                        let parse = |s: &str| s.parse::<u8>().unwrap_or_else(|_| panic!("{s}"));
                        pieces.insert(
                            (x as i32, height - 1 - row as i32),
                            (parse(kind), parse(orientation)),
                        );
                    }
                }
            }
            let kinds: Vec<u8> = roof["kinds"]
                .as_array()
                .unwrap_or_else(|| panic!("kinds"))
                .iter()
                .map(|k| k.as_u64().unwrap_or(0) as u8)
                .collect();
            let label = text(&roof["type"]);
            let ridge = match text(&roof["ridge"]).as_str() {
                "EW" => RoofAxis::EastWest,
                _ => RoofAxis::NorthSouth,
            };
            let corner = match text(&roof["corner"]).as_str() {
                "rows" => GableCorner::Rows,
                "cols" => GableCorner::Columns,
                _ => GableCorner::Outer,
            };
            let choice = if label.starts_with("gable") {
                RoofChoice::Gable { ridge, corner }
            } else {
                RoofChoice::Hip
            };
            // A hip roofs the whole outline: a courtyard has no pieces but is inside it.
            let footprint = fill(&pieces.keys().copied().collect());
            Real {
                name: text(&roof["name"]),
                label,
                choice,
                assets: HipRoofAssets {
                    kinds: crate::RoofKinds {
                        outer_edge: kinds[0],
                        outer_slope: kinds[1],
                        inner_slope: kinds[2],
                    },
                    trim: roof["type"] != "hip_notrim",
                    gable_percent: 0,
                    ..assets()
                },
                footprint: footprint
                    .into_iter()
                    .map(|(x, y)| MapTile::new(x, y))
                    .collect(),
                pieces,
            }
        })
        .collect()
}

/// The tiles inside the outline of `tiles`: the set plus every tile that cannot reach outside.
fn fill(tiles: &BTreeSet<(i32, i32)>) -> BTreeSet<(i32, i32)> {
    let (x0, x1) = (
        tiles.iter().map(|t| t.0).min().unwrap_or(0) - 1,
        tiles.iter().map(|t| t.0).max().unwrap_or(0) + 1,
    );
    let (y0, y1) = (
        tiles.iter().map(|t| t.1).min().unwrap_or(0) - 1,
        tiles.iter().map(|t| t.1).max().unwrap_or(0) + 1,
    );
    let mut outside = BTreeSet::from([(x0, y0)]);
    let mut stack = vec![(x0, y0)];
    while let Some((x, y)) = stack.pop() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let next = (x + dx, y + dy);
            if (x0..=x1).contains(&next.0)
                && (y0..=y1).contains(&next.1)
                && !tiles.contains(&next)
                && outside.insert(next)
            {
                stack.push(next);
            }
        }
    }
    (x0..=x1)
        .flat_map(|x| (y0..=y1).map(move |y| (x, y)))
        .filter(|t| !outside.contains(t))
        .collect()
}

/// The kind a piece is, whichever of its two numbers it has.
fn alike(kind: u8) -> u8 {
    match kind {
        21 => 19,
        13 => 16,
        14 => 15,
        k => k,
    }
}

fn roof_mixes_corner_numbers(pieces: &BTreeMap<(i32, i32), (u8, u8)>) -> bool {
    let has = |k: u8| pieces.values().any(|&(kind, _)| kind == k);
    (has(19) && has(21)) || (has(16) && has(13)) || (has(15) && has(14))
}

/// The point of the whole module: a real roof, regenerated from its outline and type alone, is
/// the real roof, piece for piece. A flat top's orientation does not matter (it is not visible);
/// everything else, including which of the two kind numbers a corner has, must match.
#[test]
fn real_roofs_are_reproduced_piece_for_piece() {
    let roofs = real_roofs();
    assert!(roofs.len() >= 150, "{} roofs in the fixture", roofs.len());
    let mut by_label: BTreeMap<String, usize> = BTreeMap::new();
    let (mut compared, mut bare, mut bare_not_flat, mut mixed_roofs) =
        (0_usize, 0_usize, 0_usize, 0_usize);
    for roof in &roofs {
        let made = by_tile(roof_placements(
            &roof.footprint,
            roof.choice,
            roof.assets,
            plane(),
        ));
        // About one real roof in ten uses both numbers of a corner pair (19 and 21, 16 and 13,
        // 15 and 14) in the same roof; those are compared by what the piece is, not its number.
        let mixed = roof_mixes_corner_numbers(&roof.pieces);
        let same = |a: u8, b: u8| a == b || (mixed && alike(a) == alike(b));
        mixed_roofs += usize::from(mixed);
        for (tile, &(kind, orientation)) in &roof.pieces {
            let Some(&(_, got_kind, got_orientation)) = made.get(tile) else {
                panic!("{} ({}): no piece at {tile:?}", roof.name, roof.label);
            };
            assert!(
                same(got_kind, kind) && (kind == 17 || got_orientation == orientation),
                "{} ({}): at {tile:?} real {kind}/{orientation}, made {got_kind}/{got_orientation}",
                roof.name,
                roof.label
            );
        }
        // Tiles the real roof leaves bare are courtyards, or a gap for a chimney or skylight.
        // Ours has flat top in a courtyard; elsewhere it may lay a piece the original left out.
        for (tile, &(_, got_kind, _)) in &made {
            if !roof.pieces.contains_key(tile) {
                bare += 1;
                bare_not_flat += usize::from(got_kind != 17);
            }
        }
        compared += roof.pieces.len();
        *by_label.entry(roof.label.clone()).or_default() += 1;
    }
    assert!(
        mixed_roofs * 5 <= roofs.len(),
        "{mixed_roofs} roofs mix corner numbers"
    );
    // Gaps in real roofs are rare next to the pieces compared.
    assert!(
        bare_not_flat * 100 <= compared,
        "{bare_not_flat} of {bare} bare tiles, {compared} compared"
    );
    // Every kind of roof the generator makes is in the fixture.
    for label in ["hip", "hip_notrim", "gable", "gable_flat"] {
        assert!(
            by_label.get(label).copied().unwrap_or(0) >= 4,
            "{by_label:?}"
        );
    }
}

#[test]
fn a_hip_without_trim_has_no_edge_pieces() {
    let mut a = assets();
    a.trim = false;
    let made = by_tile(hip_roof_placements(&rect(8, 6), a, plane()));
    assert!(made.values().all(|&(_, kind, _)| kind != 18 && kind != 19));
    // The outermost ring is the slope ring.
    assert_eq!(made[&(3, 0)].1, 12);
    assert_eq!(made[&(0, 0)].1, 16);
}

#[test]
fn the_other_corner_numbers_change_only_the_kind() {
    let standard = by_tile(hip_roof_placements(&rect(9, 7), assets(), plane()));
    let mut other = assets();
    other.kinds = crate::RoofKinds {
        outer_edge: 21,
        outer_slope: 13,
        inner_slope: 14,
    };
    let swapped = by_tile(hip_roof_placements(&rect(9, 7), other, plane()));
    for (tile, (_, kind, orientation)) in &standard {
        let (_, other_kind, other_orientation) = swapped[tile];
        let expected = match kind {
            19 => 21,
            16 => 13,
            15 => 14,
            k => *k,
        };
        assert_eq!((other_kind, other_orientation), (expected, *orientation));
    }
}

#[test]
fn a_pointed_gable_has_two_equal_slopes_and_no_flat_piece() {
    let pieces = by_tile(
        gable_roof_placements(
            &rect(10, 8),
            RoofAxis::EastWest,
            GableCorner::Outer,
            assets(),
            plane(),
        )
        .unwrap_or_else(|| unreachable!("a rectangle takes a gable")),
    );
    assert_eq!(pieces.len(), 80);
    let count = |kind: u8, orientation: u8| {
        pieces
            .values()
            .filter(|&&(_, k, o)| k == kind && o == orientation)
            .count()
    };
    // Six rows inside the trim: three face south, three face north, eight tiles long.
    assert_eq!(count(12, 1), 24);
    assert_eq!(count(12, 3), 24);
    assert_eq!(count(17, 1), 0);
    // Trim: the long runs, the short runs, four corners.
    assert_eq!(count(18, 1), 8);
    assert_eq!(count(18, 3), 8);
    assert_eq!(count(18, 2), 6);
    assert_eq!(count(18, 0), 6);
    assert_eq!(pieces[&(0, 0)], (1790, 19, 1));
    assert_eq!(pieces[&(9, 7)], (1790, 19, 3));
    // The slope facing south is the southern half.
    assert_eq!(pieces[&(4, 1)].2, 1);
    assert_eq!(pieces[&(4, 6)].2, 3);
}

#[test]
fn an_odd_span_gets_a_flat_ridge_between_one_row_of_slope_each_side() {
    let pieces = by_tile(
        gable_roof_placements(
            &rect(7, 7),
            RoofAxis::NorthSouth,
            GableCorner::Columns,
            assets(),
            plane(),
        )
        .unwrap_or_else(|| unreachable!("a rectangle takes a gable")),
    );
    // Five columns inside the trim: west slope, three flat, east slope.
    assert_eq!((pieces[&(1, 3)].1, pieces[&(1, 3)].2), (12, 2));
    assert_eq!(pieces[&(2, 3)].1, 17);
    assert_eq!(pieces[&(4, 3)].1, 17);
    assert_eq!((pieces[&(5, 3)].1, pieces[&(5, 3)].2), (12, 0));
    // Corners take the west and east trim.
    assert_eq!((pieces[&(0, 0)].1, pieces[&(0, 0)].2), (18, 2));
    assert_eq!((pieces[&(6, 6)].1, pieces[&(6, 6)].2), (18, 0));
}

#[test]
fn a_gable_that_cannot_be_laid_falls_back_to_a_hip() {
    let l: Vec<MapTile> = rect(9, 9)
        .into_iter()
        .filter(|t| !(t.x >= 5 && t.y >= 5))
        .collect();
    let gable = RoofChoice::Gable {
        ridge: RoofAxis::EastWest,
        corner: GableCorner::Outer,
    };
    assert!(
        gable_roof_placements(
            &l,
            RoofAxis::EastWest,
            GableCorner::Outer,
            assets(),
            plane()
        )
        .is_none()
    );
    assert_eq!(
        by_tile(roof_placements(&l, gable, assets(), plane())),
        by_tile(hip_roof_placements(&l, assets(), plane()))
    );
    let mut bare = assets();
    bare.trim = false;
    assert!(
        gable_roof_placements(
            &rect(8, 6),
            RoofAxis::EastWest,
            GableCorner::Rows,
            bare,
            plane()
        )
        .is_none()
    );
}

#[test]
fn the_choice_follows_the_kit_and_the_outline() {
    let l: Vec<MapTile> = rect(9, 9)
        .into_iter()
        .filter(|t| !(t.x >= 5 && t.y >= 5))
        .collect();
    let mut always = assets();
    always.gable_percent = 100;
    let mut never = assets();
    never.gable_percent = 0;
    // An L is always a hip; a kit can ask for all hips or all gables.
    assert_eq!(plan_roof_choice(&l, always, 1, 1), RoofChoice::Hip);
    assert_eq!(plan_roof_choice(&rect(10, 8), never, 1, 1), RoofChoice::Hip);
    assert!(matches!(
        plan_roof_choice(&rect(10, 8), always, 1, 1),
        RoofChoice::Gable { .. }
    ));
    // The ridge runs along the longer side.
    for seed in 0..20 {
        assert!(matches!(
            plan_roof_choice(&rect(10, 7), always, seed, 0),
            RoofChoice::Gable {
                ridge: RoofAxis::EastWest,
                ..
            }
        ));
        assert!(matches!(
            plan_roof_choice(&rect(6, 11), always, seed, 0),
            RoofChoice::Gable {
                ridge: RoofAxis::NorthSouth,
                ..
            }
        ));
    }
    // Too small to hold a ridge inside the trim, or no trim: a hip.
    assert_eq!(plan_roof_choice(&rect(5, 3), always, 1, 1), RoofChoice::Hip);
    let mut bare = always;
    bare.trim = false;
    assert_eq!(plan_roof_choice(&rect(10, 8), bare, 1, 1), RoofChoice::Hip);
}

#[test]
fn about_a_fifth_of_rectangles_are_gables_as_in_the_real_map() {
    let gables = (0..1000)
        .filter(|seed| {
            matches!(
                plan_roof_choice(&rect(10, 8), assets(), *seed, 7),
                RoofChoice::Gable { .. }
            )
        })
        .count();
    assert!((150..=250).contains(&gables), "{gables} of 1000");
    // All three corner fillings occur, and the choice repeats for the same seed.
    let corners: BTreeSet<String> = (0..300)
        .filter_map(|seed| {
            match plan_roof_choice(
                &rect(10, 8),
                crate::HipRoofAssets {
                    gable_percent: 100,
                    ..assets()
                },
                seed,
                3,
            ) {
                RoofChoice::Gable { corner, .. } => Some(format!("{corner:?}")),
                RoofChoice::Hip => None,
            }
        })
        .collect();
    assert_eq!(corners.len(), 3);
    assert_eq!(
        plan_roof_choice(&rect(10, 8), assets(), 9, 4),
        plan_roof_choice(&rect(10, 8), assets(), 9, 4)
    );
}
