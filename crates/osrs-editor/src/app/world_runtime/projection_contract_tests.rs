//! Cross-owner lifecycle checks for the accepted FIFO projection contract.
use super::{test_support::*, *};
use crate::app::edit_runtime::{EditExecution, EditReceipt, EditRuntime};
use osrs_core::coords::MapTile;
use osrs_edit::{MapOp, MapPatch, MapPosition, TerrainTile};
use std::time::Instant;

fn terrain_receipt(tile: MapTile, revision: u64) -> EditReceipt {
    let mut session = EditRuntime::default();
    let patch = MapPatch::new([MapOp::SetTerrain {
        coord: MapPosition::new(
            tile,
            osrs_core::coords::SourcePlane::new(0).unwrap_or_else(|| panic!("valid plane")),
        ),
        before: Default::default(),
        after: TerrainTile {
            underlay: Some(osrs_core::ids::FloorUnderlayId::new(1)),
            ..Default::default()
        },
    }]);
    let EditExecution::Committed { mut receipt, .. } = session
        .execute(patch, |_| Ok::<_, ()>(()))
        .unwrap_or_else(|error| panic!("{error:?}"))
    else {
        panic!("nonempty terrain edit")
    };
    receipt.revision = revision;
    receipt
}

#[test]
fn animation_membership_changes_discard_the_older_in_flight_batch() {
    for transition in ["replace", "rebuild", "evict"] {
        let mut world = world();
        let candidate = candidate();
        seed_installed(&mut world, &candidate);
        world.animations.force_rebuild();
        assert!(world.animations.request(0, (3200, 3200), 80));
        match transition {
            "replace" => world.region_installed(
                identity(&world, &candidate),
                &RegionBuildScope::Full,
                candidate.clone(),
            ),
            "rebuild" => {
                world.begin_rebuild_all(false, true);
            }
            "evict" => {
                world.unload_stale((60, 60), 0);
            }
            _ => unreachable!(),
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while !world.animations.is_idle() {
            assert!(
                world.poll_animation_geometry().is_none(),
                "{transition} must not publish a pre-transition aggregate"
            );
            assert!(Instant::now() < deadline, "animation worker stalled");
            std::thread::yield_now();
        }
        world.schedule_animation(true, 0.1, (3200, 3200));
        loop {
            if world.poll_animation_geometry().is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "animation rebuild stalled");
            std::thread::yield_now();
        }
    }
}

#[test]
fn animation_settings_and_source_changes_discard_the_older_in_flight_batch() {
    for transition in [
        "smoothing",
        "freeze",
        "source",
        "smooth roundtrip",
        "freeze roundtrip",
    ] {
        let mut world = world();
        seed_installed(
            &mut world,
            &animated_candidate().unwrap_or_else(|error| panic!("{error:?}")),
        );
        world.schedule_animation(true, 0.1, (3200, 3200));
        match transition {
            "smoothing" => world.set_animation_smoothing(true),
            "freeze" => world.schedule_animation(false, 0.1, (3200, 3200)),
            "source" => {
                drop(world.replace_streamer(WorldRuntime::spawn_streamer(source())));
            }
            "smooth roundtrip" => {
                world.set_animation_smoothing(true);
                world.set_animation_smoothing(false);
            }
            "freeze roundtrip" => {
                world.schedule_animation(false, 0.1, (3200, 3200));
                world.schedule_animation(true, 0.1, (3200, 3200));
            }
            _ => unreachable!(),
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while !world.animations.is_idle() {
            assert!(
                world.poll_animation_geometry().is_none(),
                "{transition} must not publish pre-transition geometry"
            );
            assert!(Instant::now() < deadline, "animation worker stalled");
            std::thread::yield_now();
        }
    }
}

#[test]
fn collision_is_unavailable_during_dirty_in_flight_and_failed_scoped_work() {
    let mut world = world();
    let candidate = candidate();
    seed_installed(&mut world, &candidate);
    let tile = MapTile::new(3204, 3204);
    assert!(matches!(
        world.collision_inspection(tile),
        CollisionInspection::Ready(_)
    ));
    world.invalidate_semantic_edit(&terrain_receipt(tile, 1));
    assert!(matches!(
        world.collision_inspection(tile),
        CollisionInspection::Updating
    ));
    world.flush_dirty_regions();
    assert!(matches!(
        world.collision_inspection(tile),
        CollisionInspection::Updating
    ));
    let scope = world
        .build_finished(KEY)
        .map(|build| build.into_scope())
        .unwrap_or_else(|| panic!("current build"));
    assert!(matches!(scope, RegionBuildScope::Zones(_)));
    world.region_unavailable(KEY);
    // A terminal partial failure hides the whole producer, including navigation evidence.
    assert_eq!(world.resident_regions(), 0);
    assert!(!world.infos().contains_key(&KEY));
    assert!(matches!(
        world.collision_inspection(tile),
        CollisionInspection::Unavailable
    ));
    world.invalidate_semantic_edit(&terrain_receipt(tile, 2));
    world.flush_dirty_regions();
    let scope = world
        .build_finished(KEY)
        .map(|build| build.into_scope())
        .unwrap_or_else(|| panic!("retry build"));
    assert_eq!(
        scope,
        RegionBuildScope::Full,
        "hidden regions need full recovery"
    );
    world.region_installed(identity(&world, &candidate), &scope, candidate.clone());
    assert!(matches!(
        world.collision_inspection(tile),
        CollisionInspection::Ready(_)
    ));
}

#[test]
fn retained_inspection_from_another_source_or_presentation_cannot_certify_collision() {
    for transition in ["source", "presentation"] {
        let mut world = world();
        seed_installed(&mut world, &candidate());
        let tile = MapTile::new(3204, 3204);
        let identity = world
            .info_identity(tile)
            .unwrap_or_else(|| panic!("identity"))
            .clone();
        assert!(matches!(
            world.collision_inspection(tile),
            CollisionInspection::Ready(_)
        ));
        match transition {
            "source" => {
                drop(world.replace_streamer(WorldRuntime::spawn_streamer(source())));
            }
            "presentation" => {
                world.begin_rebuild_all(false, true);
            }
            _ => unreachable!(),
        }
        // The caller has yet to remove its old image/picker data. A zero epoch after source reset
        // must not make that old producer's collision appear current in the replacement source.
        assert!(world.infos().contains_key(&KEY));
        assert_eq!(world.info_identity(tile), Some(&identity));
        assert!(
            matches!(
                world.collision_inspection(tile),
                CollisionInspection::Unavailable
            ),
            "{transition}"
        );
    }
}

#[test]
fn dirty_zone_work_coalesces_and_full_work_subsumes_it() {
    let mut world = world();
    let candidate = candidate();
    seed_installed(&mut world, &candidate);
    for revision in 1..=3 {
        world.invalidate_semantic_edit(&terrain_receipt(MapTile::new(3204, 3204), revision));
    }
    assert_eq!(world.in_flight_count(), 0);
    let expected = world.dirty_zones.get(&KEY).cloned().unwrap_or_default();
    assert!(!expected.is_empty());
    world.flush_dirty_regions();
    assert_eq!(world.in_flight_count(), 1);
    assert_eq!(
        world.build_finished(KEY).map(|build| build.into_scope()),
        Some(RegionBuildScope::Zones(expected))
    );
    world.invalidate_semantic_edit(&terrain_receipt(MapTile::new(3204, 3204), 4));
    world.mark_dirty(KEY);
    assert!(!world.dirty_zones.contains_key(&KEY));
    world.flush_dirty_regions();
    assert_eq!(world.in_flight_count(), 1);
    assert_eq!(
        world.build_finished(KEY).map(|build| build.into_scope()),
        Some(RegionBuildScope::Full)
    );
}

#[test]
fn later_edit_retires_old_scope_and_schedules_the_pending_scope() {
    let mut world = world();
    let candidate = candidate();
    seed_installed(&mut world, &candidate);
    world.invalidate_semantic_edit(&terrain_receipt(MapTile::new(3204, 3204), 1));
    let old_scope = world.dirty_zones.get(&KEY).cloned().unwrap_or_default();
    world.flush_dirty_regions();
    world.invalidate_semantic_edit(&terrain_receipt(MapTile::new(3220, 3220), 2));
    let mut pending = world.dirty_zones.get(&KEY).cloned().unwrap_or_default();
    pending.extend(old_scope);
    world.flush_dirty_regions();
    assert_eq!(world.in_flight_count(), 1);
    assert_eq!(
        world.build_finished(KEY).map(|build| build.into_scope()),
        None
    );
    assert_eq!(world.in_flight_count(), 0);
    assert_eq!(world.dirty_zones.get(&KEY), Some(&pending));
    world.flush_dirty_regions();
    assert_eq!(
        world.build_finished(KEY).map(|build| build.into_scope()),
        Some(RegionBuildScope::Zones(pending))
    );
}

#[test]
fn unrelated_hydration_revision_does_not_reject_a_current_region_ticket() {
    let mut world = world();
    request(&mut world, RegionBuildScope::Full);
    world.set_semantic_revision(999);
    assert_eq!(
        world.build_finished(KEY).map(|build| build.into_scope()),
        Some(RegionBuildScope::Full)
    );
}

#[test]
fn full_build_keeps_full_obligation_after_a_newer_zone_edit() {
    let mut world = world();
    let candidate = candidate();
    seed_installed(&mut world, &candidate);
    world.mark_dirty(KEY);
    world.flush_dirty_regions();
    world.invalidate_semantic_edit(&terrain_receipt(MapTile::new(3220, 3220), 2));
    assert_eq!(
        world.build_finished(KEY).map(|build| build.into_scope()),
        None
    );
    assert!(world.dirty_regions.contains(&KEY));
    assert!(!world.dirty_zones.contains_key(&KEY));
    world.flush_dirty_regions();
    assert_eq!(
        world.build_finished(KEY).map(|build| build.into_scope()),
        Some(RegionBuildScope::Full)
    );
    assert!(!world.dirty_regions.contains(&KEY));
}
