//! Accepted animated artifacts retain dispatch provenance through the real GPU upload lifecycle.
use super::*;
use crate::app::world_runtime::{RegionBuildScope, WorldRuntime, test_support::*};
use osrs_core::coords::MapTile;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const TILE: MapTile = MapTile::new(3200, 3200);

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

#[test]
fn gpu_animated_identity_follows_upload_noop_and_empty_replacement() -> TestResult {
    let Some(mut app) = app()? else {
        return Ok(());
    };
    request(&mut app.world, RegionBuildScope::Full);
    region_install::install_region(&mut app, animated_candidate()?);
    let session = app.session.clone();
    let static_bytes = app.renderer.resident_vertex_bytes();
    app.world.schedule_animation(true, 0.1, (3200, 3200));
    let first = finish_animation(&mut app.world).ok_or("first artifact")?;
    assert_eq!(first.geometry.vertex_count(), 3);
    app.renderer.upload_animation(&first);
    assert!(std::ptr::eq(
        app.renderer.animation_identity().ok_or("GPU identity")?,
        first.identity.as_ref(),
    ));
    assert!(app.renderer.resident_vertex_bytes() > static_bytes);
    let description = first.identity.describe(TILE);
    assert!(description.iter().any(|line| line.contains("build 0/1;")));
    // A worker no-op does not replace or relabel the last uploaded image.
    app.world.schedule_animation(true, 0.2, (3200, 3200));
    assert!(finish_animation(&mut app.world).is_none());
    assert_eq!(
        app.renderer.animation_identity(),
        Some(first.identity.as_ref())
    );
    assert_eq!(
        app.renderer
            .animation_identity()
            .ok_or("retained")?
            .describe(TILE),
        description
    );
    assert_eq!(app.session, session);
    app.world.region_unavailable(KEY);
    app.world.schedule_animation(true, 0.3, (3200, 3200));
    let empty = finish_animation(&mut app.world).ok_or("empty artifact")?;
    app.renderer.upload_animation(&empty);
    assert!(app.renderer.animation_identity().is_none());
    assert_eq!(app.renderer.resident_vertex_bytes(), static_bytes);
    assert_eq!(app.session, session);
    eprintln!(
        "GPU animation identity exercised nonempty upload, no-op retention and empty retirement"
    );
    Ok(())
}

#[test]
fn gpu_animation_identity_is_cleared_by_terminal_rebuild_and_source_cleanup() -> TestResult {
    for transition in ["terminal", "rebuild", "source"] {
        let Some(mut app) = app()? else {
            return Ok(());
        };
        request(&mut app.world, RegionBuildScope::Full);
        region_install::install_region(&mut app, animated_candidate()?);
        app.world.schedule_animation(true, 0.1, (3200, 3200));
        let first = finish_animation(&mut app.world).ok_or("first")?;
        app.renderer.upload_animation(&first);
        assert!(app.renderer.animation_identity().is_some());
        let session = app.session.clone();
        match transition {
            "terminal" => {
                request(&mut app.world, RegionBuildScope::Full);
                region_install::finish_unavailable_region(
                    &mut app,
                    candidate().region,
                    Some("fixture"),
                );
            }
            "rebuild" => app.rebuild_all(),
            "source" => {
                drop(
                    app.world
                        .replace_streamer(WorldRuntime::spawn_streamer(source())),
                );
                app.rebuild_all();
            }
            _ => unreachable!(),
        }
        assert!(app.renderer.animation_identity().is_none(), "{transition}");
        assert_eq!(app.renderer.resident_vertex_bytes(), 0, "{transition}");
        assert_eq!(app.session, session, "{transition}");
    }
    eprintln!("GPU animation identity retired with terminal, presentation and source cleanup");
    Ok(())
}

#[test]
fn gpu_frame_reconciles_freeze_before_accepting_an_arrived_animation() -> TestResult {
    let Some(mut app) = app()? else {
        return Ok(());
    };
    request(&mut app.world, RegionBuildScope::Full);
    region_install::install_region(&mut app, animated_candidate()?);
    app.world.schedule_animation(true, 0.0, (3200, 3200));
    app.settings.prefs.animate_objects = false;
    // This production frame must reject the previous request before it can publish. The FIFO
    // replacement is frozen, regardless of whether the old completion arrived before this call.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        app.update_animations();
        if let Some(identity) = app.renderer.animation_identity() {
            assert!(identity.describe(TILE)[1].contains("frozen true"));
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "animation frame stalled"
        );
        std::thread::yield_now();
    }
    assert!(app.renderer.resident_vertex_bytes() > 0);
    eprintln!("GPU frame rejected pre-freeze animation and published the frozen replacement");
    Ok(())
}
