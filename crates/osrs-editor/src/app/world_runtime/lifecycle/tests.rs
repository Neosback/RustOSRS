use super::*;
use crate::app::world_runtime::{test_support::*, *};
use std::time::Instant;

#[test]
fn rejection_discards_all_world_projections_and_pending_proofs() {
    for scope in [
        RegionBuildScope::Full,
        RegionBuildScope::Zones(BTreeSet::from([(400, 400)])),
    ] {
        let mut world = world();
        let candidate = candidate();
        seed_installed(&mut world, &candidate);
        request(&mut world, scope.clone());
        assert_eq!(
            world.build_finished(KEY).map(|build| build.into_scope()),
            Some(scope)
        );
        world.dirty_regions.insert(KEY);
        world.dirty_zones.insert(KEY, BTreeSet::from([(400, 400)]));
        let stamp = height_dependencies::HeightDependencyStamp::new(0, 1);
        let vertex = osrs_edit::HeightVertexCoord::new(
            osrs_core::coords::MapTile::new(3200, 3200),
            osrs_core::coords::SourcePlane::new(0).unwrap_or_else(|| panic!("valid plane")),
        );
        assert!(
            world
                .height_dependencies
                .proven_zones(&[(KEY, stamp)], &[vertex])
                .is_some()
        );
        let version = world.infos_version();
        world.region_unavailable(KEY);
        assert!(matches!(world.loaded.get(&KEY), Some(None)));
        assert!(world.infos().is_empty());
        assert!(world.infos_version() > version);
        assert!(
            world
                .height_dependencies
                .proven_zones(&[(KEY, stamp)], &[vertex])
                .is_none()
        );
        assert_eq!(
            world.resolve_install_scope(KEY, RegionBuildScope::Full, &candidate.geometry),
            RegionBuildScope::Full
        );
        assert!(!world.dirty_regions.contains(&KEY));
        assert!(!world.dirty_zones.contains_key(&KEY));
        assert_eq!(world.resident_regions(), 0);
        assert!(world.animation.dirty);
    }
}

#[test]
fn unavailable_region_does_not_loop_and_each_existing_reset_can_retry() {
    let mut world = world();
    world.region_unavailable(KEY);
    for _ in 0..3 {
        world.request_missing(KEY, 0);
        world.flush_dirty_regions();
    }
    assert_eq!(world.in_flight_count(), 0);
    world.begin_rebuild_all(false, false);
    world.request_missing(KEY, 0);
    assert_eq!(world.in_flight_count(), 1);
    let _ = world.build_finished(KEY).map(|build| build.into_scope());
    world.region_unavailable(KEY);
    assert_eq!(world.unload_stale((60, 60), 0), vec![KEY]);
    world.request_missing(KEY, 0);
    assert_eq!(world.in_flight_count(), 1);
    let _ = world.build_finished(KEY).map(|build| build.into_scope());
    world.region_unavailable(KEY);
    let replacement = WorldRuntime::spawn_streamer(source());
    drop(world.replace_streamer(replacement));
    world.begin_rebuild_all(false, false);
    world.request_missing(KEY, 0);
    assert_eq!(world.in_flight_count(), 1);
}

#[test]
fn tile_invalidation_after_rejection_requests_full_recovery() {
    let mut world = world();
    world.region_unavailable(KEY);
    let mut session = crate::app::edit_runtime::EditRuntime::default();
    let tile = osrs_core::coords::MapTile::new(3200, 3200);
    let coord = osrs_edit::MapPosition::new(
        tile,
        osrs_core::coords::SourcePlane::new(0).unwrap_or_else(|| panic!("valid plane")),
    );
    let patch = osrs_edit::MapPatch::new([osrs_edit::MapOp::SetTerrain {
        coord,
        before: Default::default(),
        after: osrs_edit::TerrainTile {
            underlay: Some(osrs_core::ids::FloorUnderlayId::new(7)),
            ..Default::default()
        },
    }]);
    let crate::app::edit_runtime::EditExecution::Committed { receipt, .. } = session
        .execute(patch, |_| Ok::<_, ()>(()))
        .unwrap_or_else(|error| panic!("{error:?}"))
    else {
        panic!("edit should commit")
    };
    world.invalidate_semantic_edit(&receipt);
    world.flush_dirty_regions();
    assert_eq!(
        world.build_finished(KEY).map(|build| build.into_scope()),
        Some(RegionBuildScope::Full)
    );
    let candidate = candidate();
    world.region_installed(
        identity(&world, &candidate),
        &RegionBuildScope::Full,
        candidate.clone(),
    );
    assert_eq!(world.resident_regions(), 1);
}

#[test]
fn rejection_discards_in_flight_animation_then_rebuilds_survivors() {
    let mut world = world();
    world.animations.force_rebuild();
    assert!(world.animations.request(0, (3200, 3200), 80));
    world.region_unavailable(KEY);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !world.animations.is_idle() {
        assert!(world.poll_animation_geometry().is_none());
        assert!(Instant::now() < deadline, "animation worker did not finish");
        std::thread::yield_now();
    }
    assert!(!world.animation.discard_in_flight);
    world.schedule_animation(true, 0.1, (3200, 3200));
    let geometry = loop {
        if let Some(artifact) = world.poll_animation_geometry() {
            break artifact.geometry;
        }
        assert!(
            Instant::now() < deadline,
            "replacement animation did not finish"
        );
        std::thread::yield_now();
    };
    assert!(geometry.zones.is_empty());
}
