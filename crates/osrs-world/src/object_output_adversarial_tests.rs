use crate::{ObjectOutputComparison, ObjectOutputCompleteness, ObjectOutputSnapshot};
use osrs_core::coords::RegionCoord;
use osrs_render::{SceneGeometry, ZoneGeometry, ZoneGroup};
use std::collections::BTreeSet;

fn geometry(entries: &[((i32, i32), u8)]) -> SceneGeometry {
    SceneGeometry {
        zones: entries
            .iter()
            .map(|&(zone, marker)| ZoneGeometry {
                zone_x: zone.0,
                zone_z: zone.1,
                groups: vec![ZoneGroup {
                    level: marker,
                    ..ZoneGroup::default()
                }],
            })
            .collect(),
    }
}

fn full(region: RegionCoord, entries: &[((i32, i32), u8)]) -> ObjectOutputSnapshot {
    ObjectOutputSnapshot::full(region, &geometry(entries))
        .unwrap_or_else(|error| unreachable!("valid full producer snapshot: {error:?}"))
}

fn exact(before: &ObjectOutputSnapshot, after: &ObjectOutputSnapshot) -> BTreeSet<(i32, i32)> {
    match before
        .compare(after)
        .unwrap_or_else(|error| unreachable!("same producer comparison: {error:?}"))
    {
        ObjectOutputComparison::Exact(zones) => zones,
        ObjectOutputComparison::Incomplete => {
            unreachable!("full producer snapshots must compare exactly")
        }
    }
}

#[test]
fn object_output_change_can_span_an_eight_tile_zone_boundary() {
    let region = RegionCoord::new(0, 0);
    let before = full(region, &[((0, 0), 1), ((1, 0), 1)]);
    let after = full(region, &[((0, 0), 2), ((1, 0), 3)]);

    assert_eq!(exact(&before, &after), BTreeSet::from([(0, 0), (1, 0)]));
}

#[test]
fn object_output_change_can_span_a_sixty_four_tile_region_boundary() {
    let left = RegionCoord::new(0, 0);
    let right = RegionCoord::new(1, 0);

    let left_before = full(left, &[((7, 0), 1)]);
    let left_after = full(left, &[((7, 0), 2)]);
    let right_before = full(right, &[((8, 0), 1)]);
    let right_after = full(right, &[((8, 0), 3)]);

    assert_eq!(
        exact(&left_before, &left_after),
        BTreeSet::from([(7, 0)])
    );
    assert_eq!(
        exact(&right_before, &right_after),
        BTreeSet::from([(8, 0)])
    );
}

#[test]
fn fixed_slot_replacement_or_reveal_is_detected_in_place() {
    let region = RegionCoord::new(0, 0);
    let before = full(region, &[((2, 2), 1)]);
    let after = full(region, &[((2, 2), 2)]);

    assert_eq!(exact(&before, &after), BTreeSet::from([(2, 2)]));
}

#[test]
fn wall_decoration_host_change_is_detected_even_when_ownership_zone_is_unchanged() {
    let region = RegionCoord::new(0, 0);
    let before = full(region, &[((3, 3), 4)]);
    let after = full(region, &[((3, 3), 5)]);

    assert_eq!(exact(&before, &after), BTreeSet::from([(3, 3)]));
}

#[test]
fn capacity_removal_revealing_a_later_game_object_is_detected_as_added_output() {
    let region = RegionCoord::new(0, 0);
    let before = full(region, &[]);
    let after = full(region, &[((4, 4), 1)]);

    assert_eq!(exact(&before, &after), BTreeSet::from([(4, 4)]));
}

#[test]
fn overlapping_capacity_chain_can_change_multiple_output_zones() {
    let region = RegionCoord::new(0, 0);
    let before = full(region, &[((4, 4), 1), ((5, 4), 1)]);
    let after = full(region, &[((5, 4), 2), ((6, 4), 1)]);

    assert_eq!(
        exact(&before, &after),
        BTreeSet::from([(4, 4), (5, 4), (6, 4)])
    );
}

#[test]
fn clipped_wall_shadow_can_change_neighboring_terrain_output_without_anchor_zone_change() {
    let region = RegionCoord::new(0, 0);
    let before = full(region, &[((0, 0), 7), ((1, 0), 1)]);
    let after = full(region, &[((0, 0), 7), ((1, 0), 2)]);

    assert_eq!(exact(&before, &after), BTreeSet::from([(1, 0)]));
}

#[test]
fn type_ten_or_eleven_shadow_can_change_terrain_across_a_zone_edge() {
    let region = RegionCoord::new(0, 0);
    let before = full(region, &[((2, 2), 1), ((3, 2), 1)]);
    let after = full(region, &[((2, 2), 2), ((3, 2), 3)]);

    assert_eq!(exact(&before, &after), BTreeSet::from([(2, 2), (3, 2)]));
}

#[test]
fn model_data_normal_reconciliation_can_change_neighbor_object_output() {
    let region = RegionCoord::new(0, 0);
    let before = full(region, &[((5, 5), 9), ((6, 5), 1)]);
    let after = full(region, &[((5, 5), 9), ((6, 5), 2)]);

    assert_eq!(exact(&before, &after), BTreeSet::from([(6, 5)]));
}

#[test]
fn dynamic_morphed_or_omitted_case_fails_closed_without_complete_candidate_output() {
    let region = RegionCoord::new(0, 0);
    let before = full(region, &[((1, 1), 1)]);
    let incomplete = ObjectOutputSnapshot::scoped(region, [(1, 1)], &geometry(&[((1, 1), 1)]))
        .unwrap_or_else(|error| unreachable!("valid scoped candidate: {error:?}"));

    assert_eq!(
        incomplete.completeness(),
        ObjectOutputCompleteness::Incomplete
    );
    assert_eq!(
        before
            .compare(&incomplete)
            .unwrap_or_else(|error| unreachable!("same producer comparison: {error:?}")),
        ObjectOutputComparison::Incomplete
    );
}

#[test]
fn complete_output_can_prove_a_dynamic_only_edit_has_no_static_diff() {
    let region = RegionCoord::new(0, 0);
    let before = full(region, &[((1, 1), 1)]);
    let after = full(region, &[((1, 1), 1)]);

    assert_eq!(exact(&before, &after), BTreeSet::new());
}
