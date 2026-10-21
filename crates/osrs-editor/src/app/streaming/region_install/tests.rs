use super::*;
use crate::app::world_runtime::test_support::*;
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::FloorUnderlayId,
};
use osrs_edit::{MapOp, MapPatch, MapPosition, TerrainTile};
use std::collections::BTreeSet;

fn prepare(
    world: &mut WorldRuntime,
    session: &mut EditRuntime,
    candidate: &RegionGeometry,
) -> Result<Option<RegionBuildScope>, RegionAdaptError> {
    prepare_region_install(world, session, candidate)
        .map(|prepared| prepared.map(|(scope, _)| scope))
}

fn position() -> MapPosition {
    MapPosition::new(
        MapTile::new(3200, 3200),
        SourcePlane::new(0).unwrap_or_else(|| panic!("valid plane")),
    )
}

fn edit(session: &mut EditRuntime, underlay: u32) {
    let coord = position();
    session
        .commit(MapPatch::new([MapOp::SetTerrain {
            coord,
            before: session.document().terrain_at(coord),
            after: TerrainTile {
                underlay: Some(FloorUnderlayId::new(underlay)),
                ..Default::default()
            },
        }]))
        .unwrap_or_else(|error| panic!("{error:?}"));
}

#[test]
fn invalid_full_and_scoped_candidates_preserve_the_complete_semantic_session() {
    for scope in [
        RegionBuildScope::Full,
        RegionBuildScope::Zones(BTreeSet::from([(400, 400)])),
    ] {
        let mut world = world();
        let mut session = EditRuntime::default();
        let valid = candidate();
        session.ingest_region(
            &adapt_region_map(&valid.source).unwrap_or_else(|error| panic!("{error:?}")),
        );
        seed_installed(&mut world, &valid);
        edit(&mut session, 2);
        edit(&mut session, 3);
        session.undo().unwrap_or_else(|error| panic!("{error:?}"));
        let before = session.clone();
        assert!(session.can_undo() && session.can_redo());
        request(&mut world, scope);
        assert!(matches!(
            prepare(&mut world, &mut session, &invalid_candidate()),
            Err(RegionAdaptError::InvalidLocKind { kind: 23, .. })
        ));
        assert_eq!(session, before);
        assert!(session.can_undo() && session.can_redo());
        assert_eq!(world.resident_regions(), 0);
        assert!(world.infos().is_empty());
        assert_eq!(world.in_flight_count(), 0);
        world.request_missing(KEY, 0);
        assert_eq!(world.in_flight_count(), 0);
    }
}

#[test]
fn valid_candidates_hydrate_once_and_preserve_authored_changes() {
    let mut world = world();
    let mut session = EditRuntime::default();
    let candidate = candidate();
    request(&mut world, RegionBuildScope::Full);
    assert_eq!(
        prepare(&mut world, &mut session, &candidate),
        Ok(Some(RegionBuildScope::Full))
    );
    assert_eq!(
        session.document().terrain_at(position()).underlay,
        Some(FloorUnderlayId::new(1))
    );
    assert!(!session.can_undo());
    let hydrated = session.clone();
    request(&mut world, RegionBuildScope::Full);
    assert!(
        prepare(&mut world, &mut session, &candidate)
            .unwrap_or_else(|error| panic!("{error:?}"))
            .is_some()
    );
    assert_eq!(session, hydrated);
    edit(&mut session, 7);
    let authored = session.clone();
    request(&mut world, RegionBuildScope::Full);
    assert!(
        prepare(&mut world, &mut session, &candidate)
            .unwrap_or_else(|error| panic!("{error:?}"))
            .is_some()
    );
    assert_eq!(session, authored);
}

#[test]
fn invalid_stale_completions_cannot_reject_a_newer_installation() {
    let mut world = world();
    let mut session = EditRuntime::default();
    let valid = candidate();
    request(&mut world, RegionBuildScope::Full);
    world.begin_rebuild_all(false, false);
    request(&mut world, RegionBuildScope::Full);
    seed_installed(&mut world, &valid);
    let before = session.clone();
    assert_eq!(
        prepare(&mut world, &mut session, &invalid_candidate()),
        Ok(None)
    );
    assert_eq!(world.resident_regions(), 1);
    assert_eq!(world.in_flight_count(), 1);
    assert!(world.infos().contains_key(&KEY));
    assert_eq!(session, before);
    // Stale input must also leave the newer Object comparison intact.
    let mut new = valid;
    new.generation = 2;
    assert_eq!(
        prepare(&mut world, &mut session, &new),
        Ok(Some(RegionBuildScope::Zones(BTreeSet::new())))
    );
}

#[test]
fn valid_object_proof_still_narrows_and_rejected_region_can_recover() {
    let mut world = world();
    let mut session = EditRuntime::default();
    let candidate = candidate();
    seed_installed(&mut world, &candidate);
    request(&mut world, RegionBuildScope::Full);
    assert_eq!(
        prepare(&mut world, &mut session, &candidate),
        Ok(Some(RegionBuildScope::Zones(BTreeSet::new())))
    );
    request(&mut world, RegionBuildScope::Full);
    assert!(prepare(&mut world, &mut session, &invalid_candidate()).is_err());
    world.begin_rebuild_all(false, false);
    let mut candidate = candidate;
    candidate.generation = 2;
    request(&mut world, RegionBuildScope::Full);
    assert_eq!(
        prepare(&mut world, &mut session, &candidate),
        Ok(Some(RegionBuildScope::Full))
    );
    world.region_installed(
        identity(&world, &candidate),
        &RegionBuildScope::Full,
        candidate.clone(),
    );
    assert_eq!(world.resident_regions(), 1);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn gpu_rejection_hides_existing_geometry_without_consuming_project_work()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::session::RegionDiff;
    use eframe::{egui_wgpu, wgpu};
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let state = match pollster::block_on(egui_wgpu::RenderState::create(
        &egui_wgpu::WgpuConfiguration::default(),
        &instance,
        None,
        egui_wgpu::RendererOptions::default(),
    )) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("GPU region rejection test unavailable: {error}");
            return Ok(());
        }
    };
    let mut app = EditorApp::from_source(
        &egui::Context::default(),
        state,
        source(),
        PathBuf::new(),
        3136,
        3136,
        false,
    )?;
    let candidate = candidate();
    for scope in [
        RegionBuildScope::Full,
        RegionBuildScope::Zones(BTreeSet::from([(400, 400)])),
    ] {
        let diff = RegionDiff {
            region: candidate.region,
            tiles: vec![],
            heights: vec![],
            objects_added: vec![],
            objects_removed: vec![],
        };
        app.project.prepare_replacement(
            &mut app.session,
            PathBuf::from("pending.json"),
            vec![diff],
        );
        app.session.ingest_region(
            &adapt_region_map(&candidate.source).unwrap_or_else(|error| panic!("{error:?}")),
        );
        edit(&mut app.session, 2);
        edit(&mut app.session, 3);
        app.session
            .undo()
            .unwrap_or_else(|error| panic!("{error:?}"));
        let before = app.session.clone();
        let pending = app.project.export_pending();
        seed_installed(&mut app.world, &candidate);
        app.renderer
            .upload_region(identity(&app.world, &candidate), &candidate.geometry);
        app.renderer
            .upload_transient(ANIMATED_KEY, &candidate.geometry);
        assert!(app.renderer.resident_vertex_bytes() > 0);
        request(&mut app.world, scope);
        install_region(&mut app, invalid_candidate());
        assert_eq!(app.renderer.resident_vertex_bytes(), 0);
        assert_eq!(app.session, before);
        assert_eq!(app.project.export_pending(), pending);
        assert!(app.world.infos().is_empty());
        assert_eq!(app.world.resident_regions(), 0);
        assert!(
            app.ui
                .status()
                .unwrap_or("")
                .contains("semantic region 50,50 failed")
        );
    }
    seed_installed(&mut app.world, &candidate);
    app.renderer
        .upload_region(identity(&app.world, &candidate), &candidate.geometry);
    app.renderer
        .upload_transient(ANIMATED_KEY, &candidate.geometry);
    let before = app.session.clone();
    app.rebuild_all();
    assert_eq!(app.renderer.resident_vertex_bytes(), 0);
    assert!(app.world.infos().is_empty());
    assert_eq!(app.session, before);
    eprintln!("GPU presentation rebuild cleared static and aggregate animated geometry");
    eprintln!("GPU rejection exercised full/scoped static and animated removal");
    Ok(())
}
