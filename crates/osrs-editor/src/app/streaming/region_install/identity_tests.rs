//! Accepted artifact provenance follows production installation, retention and cleanup paths.
#![cfg(not(target_arch = "wasm32"))]
use super::*;
use crate::app::world_runtime::test_support::*;
use osrs_core::coords::MapTile;
use osrs_world::RegionRenderPath;
use std::collections::BTreeSet;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const FIRST: MapTile = MapTile::new(3200, 3200);
const SECOND: MapTile = MapTile::new(3208, 3200);

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

fn projection() -> RegionGeometry {
    let mut candidate = candidate();
    let mut other = candidate.geometry.zones[0].clone();
    other.zone_x += 1;
    candidate.geometry.zones.push(other);
    candidate
}

fn current_static(app: &EditorApp, tile: MapTile) -> RegionArtifactIdentity {
    app.renderer
        .static_identity(tile)
        .unwrap_or_else(|| panic!("static artifact"))
        .clone()
}

#[test]
fn gpu_partial_refresh_keeps_untouched_identity_and_shares_new_identity_with_picker() -> TestResult
{
    let Some(mut app) = app()? else { return Ok(()) };
    let candidate = projection();
    app.world.set_semantic_revision(7);
    request(&mut app.world, RegionBuildScope::Full);
    // Unrelated hydration can advance the session without invalidating the producer ticket.
    app.world.set_semantic_revision(9);
    install_region(&mut app, candidate.clone());
    let first = current_static(&app, FIRST);
    assert!(std::ptr::eq(
        app.renderer
            .static_identity(FIRST)
            .ok_or("static identity")?,
        app.world.info_identity(FIRST).ok_or("picker identity")?,
    ));
    let lines = first.describe().join("\n");
    assert!(
        lines.contains("build 0/1; issued revision 7; epoch 0; presentation 1"),
        "{lines}"
    );
    assert!(lines.contains(candidate.target.profile_id()));
    assert!(lines.contains(&candidate.target.profile_digest().to_string()));
    assert!(lines.contains(&candidate.target.cache_fingerprint().to_string()));
    assert!(lines.contains("extraction Reference;"));
    assert_eq!(current_static(&app, SECOND), first);

    app.world.set_semantic_revision(10);
    request(
        &mut app.world,
        RegionBuildScope::Zones(BTreeSet::from([(400, 400)])),
    );
    let mut changed = candidate.clone();
    changed.geometry.zones[0].groups[0].geometry.opaque[0].abhsl = 3;
    install_region(&mut app, changed.clone());
    let second = current_static(&app, FIRST);
    assert_ne!(second, first);
    assert!(second.describe()[0].contains("build 0/2; issued revision 10"));
    assert_eq!(current_static(&app, SECOND), first);
    assert!(std::ptr::eq(
        app.renderer
            .static_identity(FIRST)
            .ok_or("static identity")?,
        app.world
            .info_identity(SECOND)
            .ok_or("complete picker identity")?,
    ));

    // A full Object proof may retain every static zone while refreshing complete picker data.
    begin_object_proof(&mut app.world);
    request(&mut app.world, RegionBuildScope::Full);
    install_region(&mut app, changed);
    assert_eq!(current_static(&app, FIRST), second);
    assert_eq!(current_static(&app, SECOND), first);
    assert!(app.world.info_identity(FIRST).ok_or("picker")?.describe()[0].contains("build 0/3;"));

    // Successful empty scoped output retires that zone's resources and identity only.
    request(
        &mut app.world,
        RegionBuildScope::Zones(BTreeSet::from([(400, 400)])),
    );
    let mut empty = candidate;
    empty.geometry = SceneGeometry::default();
    install_region(&mut app, empty);
    assert!(app.renderer.static_identity(FIRST).is_none());
    assert_eq!(current_static(&app, SECOND), first);
    assert!(app.world.info_identity(FIRST).is_some());
    eprintln!(
        "GPU artifact identity exercised full, partial, Object-proof and empty-zone installs"
    );
    Ok(())
}

#[test]
fn gpu_stale_failure_rebuild_and_replacement_keep_or_remove_exact_artifact_identities() -> TestResult
{
    let Some(mut app) = app()? else { return Ok(()) };
    request(&mut app.world, RegionBuildScope::Full);
    install_region(&mut app, projection());
    let first = current_static(&app, FIRST);
    let picker = app.world.info_identity(FIRST).ok_or("picker")?.clone();
    let session = app.session.clone();
    // An unsolicited Region has no current ticket and cannot install even a different path.
    let mut unsolicited = projection();
    unsolicited.render_path = RegionRenderPath::ReferenceFallback;
    unsolicited.presentation.hd_geometry = true;
    install_region(&mut app, unsolicited);
    assert_eq!(current_static(&app, FIRST), first);
    assert_eq!(app.world.info_identity(FIRST), Some(&picker));
    assert_eq!(app.session, session);

    // A semantic rejection removes metadata together with the real static/picker projections.
    request(&mut app.world, RegionBuildScope::Full);
    install_region(&mut app, invalid_candidate());
    assert!(app.renderer.static_identity(FIRST).is_none());
    assert!(app.world.info_identity(FIRST).is_none());
    assert_eq!(app.renderer.resident_vertex_bytes(), 0);
    assert_eq!(app.session, session);

    app.rebuild_all();
    let mut next = projection();
    next.generation = 2;
    request(&mut app.world, RegionBuildScope::Full);
    install_region(&mut app, next.clone());
    let second = current_static(&app, FIRST);
    assert!(second.describe()[0].contains("presentation 2"));
    request(&mut app.world, RegionBuildScope::Full);
    app.rebuild_all();
    assert!(app.renderer.static_identity(FIRST).is_none());
    assert!(app.world.info_identity(FIRST).is_none());
    request(&mut app.world, RegionBuildScope::Full);
    // The old FIFO result cannot restore identity or consume the newer request.
    install_region(&mut app, next);
    assert!(app.renderer.static_identity(FIRST).is_none());
    assert!(app.world.info_identity(FIRST).is_none());
    assert_eq!(app.world.in_flight_count(), 1);
    let mut current = projection();
    current.generation = 3;
    install_region(&mut app, current);
    let third = current_static(&app, FIRST);
    assert_ne!(third, second);
    assert!(third.describe()[0].contains("presentation 3"));

    // A current terminal result removes identity; source replacement uses a new instance stamp.
    request(&mut app.world, RegionBuildScope::Full);
    finish_unavailable_region(&mut app, candidate().region, Some("missing data"));
    assert!(app.renderer.static_identity(FIRST).is_none());
    assert!(app.world.info_identity(FIRST).is_none());
    drop(
        app.world
            .replace_streamer(WorldRuntime::spawn_streamer(source())),
    );
    app.settings.prefs.render_profile = ProfileChoice::Hd;
    app.rebuild_all();
    let mut replacement = projection();
    replacement.generation = 4;
    replacement.render_path = RegionRenderPath::ReferenceFallback;
    replacement.presentation.hd_geometry = true;
    request(&mut app.world, RegionBuildScope::Full);
    install_region(&mut app, replacement);
    let replaced = current_static(&app, FIRST);
    assert!(replaced.describe()[0].contains("build 1/"));
    assert!(replaced.describe()[3].contains("ReferenceFallback"));
    assert!(replaced.describe()[3].contains("hd_geometry: true"));
    app.renderer.remove_region(KEY);
    app.world.remove_info(KEY);
    assert!(app.renderer.static_identity(FIRST).is_none());
    assert!(app.world.info_identity(FIRST).is_none());
    eprintln!(
        "GPU artifact identities followed stale, rejection, rebuild, terminal and source lifecycles"
    );
    Ok(())
}

#[test]
fn gpu_artifact_lookup_uses_euclidean_zone_coordinates() -> TestResult {
    let Some(mut app) = app()? else { return Ok(()) };
    let mut negative = projection();
    negative.region = RegionCoord::new(-1, -1);
    negative.geometry.zones[0].zone_x = -1;
    negative.geometry.zones[0].zone_z = -1;
    negative.geometry.zones.truncate(1);
    let identity = identity(&app.world, &negative);
    app.renderer
        .upload_region(Arc::clone(&identity), &negative.geometry);
    assert_eq!(
        app.renderer.static_identity(MapTile::new(-1, -1)),
        Some(identity.as_ref())
    );
    assert_eq!(
        app.renderer.static_identity(MapTile::new(-8, -8)),
        Some(identity.as_ref())
    );
    assert!(app.renderer.static_identity(MapTile::new(0, 0)).is_none());
    app.renderer.remove_region((-1, -1));
    assert!(app.renderer.static_identity(MapTile::new(-1, -1)).is_none());
    eprintln!("GPU artifact lookup exercised negative region and zone coordinates");
    Ok(())
}
