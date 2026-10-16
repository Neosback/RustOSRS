use super::*;

fn id(n: u32) -> ObjectId {
    ObjectId::new(n)
}
fn wall(n: u32, x: i32, y: i32, orientation: u8) -> Placed {
    placed(n, 0, x, y, orientation)
}
fn placed(n: u32, kind: u8, x: i32, y: i32, orientation: u8) -> Placed {
    Placed {
        id: id(n),
        kind,
        orientation,
        tile: (x, y),
        plane: 0,
    }
}
fn census(placements: &[Placed]) -> Census {
    let mut c = Census::default();
    c.add(placements);
    c
}
fn count(c: &Census, r: Relation) -> u32 {
    c.relations().get(&r).copied().unwrap_or(0)
}

/// A north wall (edge 1) along y = 10 from x = 0 to 3, ids 1,1,1,1.
fn run(ids: &[u32]) -> Vec<Placed> {
    ids.iter()
        .enumerate()
        .map(|(i, n)| wall(*n, i as i32, 10, 1))
        .collect()
}

#[test]
fn consecutive_walls_on_one_edge_line_continue_each_other_once_per_pair() {
    let c = census(&run(&[1, 1, 2]));
    assert_eq!(count(&c, Relation::Continues(id(1), id(1))), 1);
    assert_eq!(count(&c, Relation::Continues(id(1), id(2))), 1);
    assert_eq!(
        count(&c, Relation::Continues(id(2), id(1))),
        0,
        "pairs are ordered"
    );
}

#[test]
fn walls_on_different_edges_or_planes_do_not_continue() {
    let c = census(&[wall(1, 0, 10, 1), wall(1, 1, 10, 3)]);
    assert!(
        c.relations()
            .keys()
            .all(|r| !matches!(r, Relation::Continues(..)))
    );
    let mut other_plane = wall(1, 1, 10, 1);
    other_plane.plane = 1;
    let c = census(&[wall(1, 0, 10, 1), other_plane]);
    assert!(
        c.relations()
            .keys()
            .all(|r| !matches!(r, Relation::Continues(..)))
    );
}

#[test]
fn an_unconnected_end_is_open_and_a_perpendicular_wall_on_the_same_tile_caps_it() {
    let c = census(&run(&[1, 1]));
    assert_eq!(
        count(
            &c,
            Relation::RunEnd {
                wall: id(1),
                end: End::Open
            }
        ),
        2
    );
    // A west wall (edge 0) on the first tile meets the run at its west end.
    let mut placements = run(&[1, 1]);
    placements.push(wall(7, 0, 10, 0));
    let c = census(&placements);
    assert_eq!(
        count(
            &c,
            Relation::RunEnd {
                wall: id(1),
                end: End::Perpendicular(id(7))
            }
        ),
        1
    );
    assert_eq!(
        count(
            &c,
            Relation::RunEnd {
                wall: id(1),
                end: End::Open
            }
        ),
        1
    );
}

#[test]
fn a_pillar_on_the_end_vertex_caps_the_run() {
    // Pillar orientation 3 owns the vertex between the south and west edges; the north run's
    // west end is the north-west vertex, which is orientation 0.
    let mut placements = run(&[1, 1]);
    placements.push(placed(9, 1, 0, 10, 0));
    let c = census(&placements);
    assert_eq!(
        count(
            &c,
            Relation::RunEnd {
                wall: id(1),
                end: End::Pillar(id(9))
            }
        ),
        1
    );
    // The wrong vertex does not count.
    let mut placements = run(&[1, 1]);
    placements.push(placed(9, 1, 0, 10, 3));
    let c = census(&placements);
    assert_eq!(
        count(
            &c,
            Relation::RunEnd {
                wall: id(1),
                end: End::Open
            }
        ),
        2
    );
}

#[test]
fn a_corner_at_either_end_of_a_run_is_joined_to_the_run() {
    // Corner type 2, orientation 1 owns the north and east edges of its tile: a north-east
    // corner closing the run's east end on x = 2, and the run goes west from there.
    let mut placements = run(&[1, 1]);
    placements.push(placed(5, 2, 2, 10, 1));
    let c = census(&placements);
    assert_eq!(
        count(
            &c,
            Relation::CornerArm {
                corner: id(5),
                arm: id(1)
            }
        ),
        1
    );
    // A corner before the run (west of x = 0) is found from the other side.
    let mut placements = run(&[1, 1]);
    placements.push(placed(6, 2, -1, 10, 1));
    let c = census(&placements);
    assert_eq!(
        count(
            &c,
            Relation::CornerArm {
                corner: id(6),
                arm: id(1)
            }
        ),
        1
    );
}

#[test]
fn a_different_object_between_two_walls_is_an_inset_of_each_host() {
    let c = census(&run(&[1, 8, 1]));
    assert_eq!(
        count(
            &c,
            Relation::Inset {
                host: id(1),
                inset: id(8)
            }
        ),
        1,
        "one host, counted once"
    );
    let c = census(&run(&[1, 8, 2]));
    assert_eq!(
        count(
            &c,
            Relation::Inset {
                host: id(1),
                inset: id(8)
            }
        ),
        1
    );
    assert_eq!(
        count(
            &c,
            Relation::Inset {
                host: id(2),
                inset: id(8)
            }
        ),
        1
    );
    // At the end of a run it is not between two walls.
    let c = census(&run(&[1, 1, 8]));
    assert!(
        c.relations()
            .keys()
            .all(|r| !matches!(r, Relation::Inset { .. }))
    );
}

#[test]
fn decorations_attach_to_walls_on_the_same_tile_edge() {
    let mut placements = run(&[1]);
    placements.push(placed(30, 4, 0, 10, 1));
    placements.push(placed(31, 4, 0, 10, 3));
    let c = census(&placements);
    assert_eq!(
        count(
            &c,
            Relation::Decorates {
                decoration: id(30),
                host: id(1)
            }
        ),
        1
    );
    assert_eq!(
        count(
            &c,
            Relation::Decorates {
                decoration: id(31),
                host: id(1)
            }
        ),
        0
    );
}

#[test]
fn analysis_does_not_depend_on_placement_order_and_regions_merge() {
    let placements = run(&[1, 2, 1, 2, 1]);
    let mut reversed = placements.clone();
    reversed.reverse();
    assert_eq!(
        census(&placements).relations(),
        census(&reversed).relations()
    );
    let (a, b) = placements.split_at(2);
    let mut merged = census(a);
    merged.merge(&census(b));
    assert_eq!(merged.occurrences(id(1)), 3);
    // Splitting loses the relation across the cut, which is why regions must be adjacency-closed.
    assert!(merged.relations().len() <= census(&placements).relations().len() + 4);
}
