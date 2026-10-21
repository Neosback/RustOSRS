//! Terminal absence/failure is projection removal, never semantic eviction.
#![cfg(not(target_arch = "wasm32"))]
use super::*;
use crate::app::world_runtime::{CollisionInspection, test_support::*};
use crate::session::RegionDiff;
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::FloorUnderlayId,
};
use osrs_edit::{MapOp, MapPatch, MapPosition, TerrainTile};
use std::collections::BTreeSet;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn app() -> Result<Option<EditorApp>, Box<dyn std::error::Error>> {
    let Some(state) = render_state() else {
        return Ok(None);
    };
    Ok(Some(EditorApp::from_source(
        &egui::Context::default(),
        state,
        source(),
        PathBuf::new(),
        3136,
        3136,
        false,
    )?))
}

fn position() -> MapPosition {
    MapPosition::new(
        MapTile::new(3200, 3200),
        SourcePlane::new(0).unwrap_or_else(|| panic!("plane")),
    )
}

fn tile(underlay: u32) -> TerrainTile {
    TerrainTile {
        underlay: Some(FloorUnderlayId::new(underlay)),
        ..Default::default()
    }
}

fn projection() -> RegionGeometry {
    let mut candidate = candidate();
    let mut other = candidate.geometry.zones[0].clone();
    other.zone_x += 1;
    candidate.geometry.zones.push(other);
    candidate
}

fn seed(app: &mut EditorApp, dirty: bool) {
    let candidate = projection();
    let diff = RegionDiff {
        region: candidate.region,
        tiles: vec![(position(), tile(9))],
        heights: vec![],
        objects_added: vec![],
        objects_removed: vec![],
    };
    app.project
        .prepare_replacement(&mut app.session, PathBuf::from("pending.json"), vec![diff]);
    app.session.ingest_region(
        &adapt_region_map(&candidate.source).unwrap_or_else(|error| panic!("{error:?}")),
    );
    if dirty {
        for underlay in [2, 3] {
            app.session
                .commit(MapPatch::new([MapOp::SetTerrain {
                    coord: position(),
                    before: app.session.document().terrain_at(position()),
                    after: tile(underlay),
                }]))
                .unwrap_or_else(|error| panic!("{error:?}"));
        }
        app.session
            .undo()
            .unwrap_or_else(|error| panic!("{error:?}"));
        assert!(app.session.can_undo() && app.session.can_redo());
    }
    seed_installed(&mut app.world, &candidate);
    app.renderer
        .upload_region(identity(&app.world, &candidate), &candidate.geometry);
    app.renderer
        .upload_transient(ANIMATED_KEY, &candidate.geometry);
    app.interaction.replace_tiles([(3200, 3200)]);
    app.ui.set_status("previous notice".into());
}

fn scopes() -> [RegionBuildScope; 2] {
    [
        RegionBuildScope::Full,
        RegionBuildScope::Zones(BTreeSet::from([(400, 400)])),
    ]
}

#[test]
fn gpu_current_empty_and_failed_hide_full_region_preserve_session_and_retry() -> TestResult {
    let Some(mut app) = app()? else {
        return Ok(());
    };
    let mut generation = 1;
    for error in [None, Some("decode failed")] {
        for scope in scopes() {
            for dirty in [false, true] {
                seed(&mut app, dirty);
                let before = app.session.clone();
                let pending = app.project.export_pending();
                request(&mut app.world, scope.clone());
                finish_unavailable_region(&mut app, candidate().region, error);
                assert_eq!(app.renderer.resident_vertex_bytes(), 0);
                assert!(app.world.infos().is_empty());
                assert!(matches!(
                    app.world.collision_inspection(MapTile::new(3200, 3200)),
                    CollisionInspection::Unavailable
                ));
                assert_eq!(app.world.resident_regions(), 0);
                assert_eq!(app.world.in_flight_count(), 0);
                assert!(app.interaction.selection_tiles().is_empty());
                assert_eq!(
                    app.session, before,
                    "clean and dirty sessions must survive both terminal outcomes"
                );
                assert_eq!(app.project.export_pending(), pending);
                assert_eq!(
                    app.ui.status(),
                    Some(if error.is_some() {
                        "region 50,50 failed: decode failed"
                    } else {
                        "previous notice"
                    })
                );
                assert_eq!(
                    app.world.resolve_install_scope(
                        KEY,
                        RegionBuildScope::Full,
                        &projection().geometry
                    ),
                    RegionBuildScope::Full,
                    "terminal removal clears pending Object comparison"
                );
                for _ in 0..3 {
                    app.world.request_missing(KEY, 0);
                    app.world.flush_dirty_regions();
                }
                assert_eq!(app.world.in_flight_count(), 0, "no per-frame retry loop");
                app.rebuild_all();
                request(&mut app.world, RegionBuildScope::Full);
                let mut recovery = candidate();
                // Synthetic candidates follow the existing presentation reset, as the worker does.
                generation += 1;
                recovery.generation = generation;
                install_region(&mut app, recovery);
                assert!(
                    app.renderer.resident_vertex_bytes() > 0,
                    "valid recovery installs"
                );
                assert!(app.world.infos().contains_key(&KEY));
                assert!(
                    app.project.export_pending().is_empty(),
                    "only valid recovery consumes restoration"
                );
                assert_eq!(app.session.document().terrain_at(position()), tile(9));
            }
        }
    }
    // A successful zero-triangle scoped extraction is not an Empty terminal outcome.
    seed(&mut app, true);
    app.renderer
        .upload_transient(ANIMATED_KEY, &SceneGeometry::default());
    let before = app.session.clone();
    let pending = app.project.export_pending();
    let bytes = app.renderer.resident_vertex_bytes();
    request(&mut app.world, scopes()[1].clone());
    let mut empty_zone = candidate();
    empty_zone.generation = generation;
    empty_zone.geometry = SceneGeometry::default();
    install_region(&mut app, empty_zone);
    assert!(
        app.renderer.resident_vertex_bytes() > 0,
        "other static zone survives"
    );
    assert!(
        app.renderer.resident_vertex_bytes() < bytes,
        "empty requested zone removed"
    );
    assert_eq!(app.world.resident_regions(), 1);
    assert!(app.world.infos().contains_key(&KEY));
    assert_eq!(app.session, before);
    assert_eq!(
        app.project.export_pending(),
        pending,
        "scoped success does not consume full restoration"
    );
    eprintln!(
        "GPU terminal Empty/Failed full/scoped clean/dirty preservation and recovery exercised"
    );
    Ok(())
}

#[test]
fn gpu_stale_empty_and_failed_preserve_newer_projection_request_and_notice() -> TestResult {
    let Some(mut app) = app()? else {
        return Ok(());
    };
    for error in [None, Some("obsolete decode error")] {
        for scope in scopes() {
            seed(&mut app, true);
            request(&mut app.world, scope);
            app.rebuild_all();
            request(&mut app.world, RegionBuildScope::Full);
            // Newer projection state and pending comparison must survive the old FIFO completion.
            seed(&mut app, true);
            let before = app.session.clone();
            let pending = app.project.export_pending();
            let bytes = app.renderer.resident_vertex_bytes();
            let info_version = app.world.infos_version();
            finish_unavailable_region(&mut app, candidate().region, error);
            assert_eq!(app.world.in_flight_count(), 1);
            assert_eq!(app.renderer.resident_vertex_bytes(), bytes);
            assert_eq!(app.world.infos_version(), info_version);
            assert_eq!(app.world.resident_regions(), 1);
            assert_eq!(app.interaction.selection_tiles().len(), 1);
            assert_eq!(app.ui.status(), Some("previous notice"));
            assert_eq!(app.session, before);
            assert_eq!(app.project.export_pending(), pending);
            assert_eq!(
                app.world
                    .build_finished(KEY)
                    .map(|build| build.into_scope()),
                Some(RegionBuildScope::Full)
            );
            assert_eq!(
                app.world.resolve_install_scope(
                    KEY,
                    RegionBuildScope::Full,
                    &projection().geometry
                ),
                RegionBuildScope::Zones(BTreeSet::new()),
                "old failure leaves newer comparison intact"
            );
            // An unsolicited/duplicate terminal completion has no authority either.
            finish_unavailable_region(&mut app, candidate().region, error);
            assert_eq!(app.renderer.resident_vertex_bytes(), bytes);
            assert_eq!(app.world.infos_version(), info_version);
            assert_eq!(app.ui.status(), Some("previous notice"));
        }
    }
    // A newer edit can stale the original ticket without a replacement request yet.
    for error in [None, Some("obsolete edit failure")] {
        seed(&mut app, true);
        request(&mut app.world, RegionBuildScope::Full);
        let crate::app::edit_runtime::EditExecution::Committed { receipt, .. } = app
            .session
            .execute(
                MapPatch::new([MapOp::SetTerrain {
                    coord: position(),
                    before: app.session.document().terrain_at(position()),
                    after: tile(4),
                }]),
                |_| Ok::<_, ()>(()),
            )
            .unwrap_or_else(|error| panic!("{error:?}"))
        else {
            panic!("nonempty edit");
        };
        app.world.invalidate_semantic_edit(&receipt);
        let before = app.session.clone();
        let pending = app.project.export_pending();
        let bytes = app.renderer.resident_vertex_bytes();
        let version = app.world.infos_version();
        finish_unavailable_region(&mut app, candidate().region, error);
        assert_eq!(app.world.in_flight_count(), 0, "old physical ticket drains");
        assert_eq!(app.renderer.resident_vertex_bytes(), bytes);
        assert_eq!(app.world.infos_version(), version);
        assert_eq!(app.session, before);
        assert_eq!(app.project.export_pending(), pending);
        assert_eq!(app.ui.status(), Some("previous notice"));
        app.world.flush_dirty_regions();
        assert_eq!(
            app.world.in_flight_count(),
            1,
            "newer dirty obligation survives"
        );
        assert!(
            app.world
                .build_finished(KEY)
                .map(|build| build.into_scope())
                .is_some()
        );
    }
    eprintln!("GPU stale Empty/Failed preserved newer installation and FIFO slot");
    Ok(())
}
