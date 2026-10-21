//! Accept current region candidates or hide unavailable projections without changing authored state.
use super::*;
use crate::app::{
    edit_runtime::EditRuntime,
    world_runtime::{RegionArtifactIdentity, RegionBuildScope, WorldRuntime},
};
use crate::session::{RegionAdaptError, adapt_region_map};
use osrs_render::SceneGeometry;
use osrs_world::RegionGeometry;

/// Composition only: semantic adaptation and freshness precede every candidate install.
pub(super) fn install_region(app: &mut EditorApp, region: RegionGeometry) {
    let install_started = Instant::now();
    app.diagnostics
        .record_build(&region.timings, region.build_ms);
    let key = (region.region.x, region.region.y);
    let (scope, identity) = match prepare_region_install(&mut app.world, &mut app.session, &region)
    {
        Ok(Some(prepared)) => prepared,
        Ok(None) => return,
        Err(error) => {
            hide_region(app, key);
            app.ui.set_status(format!(
                "semantic region {},{} failed: {error:?}",
                key.0, key.1
            ));
            return;
        }
    };
    match &scope {
        world_runtime::RegionBuildScope::Full => {
            app.renderer
                .upload_region(Arc::clone(&identity), &region.geometry);
        }
        world_runtime::RegionBuildScope::Zones(zones) => {
            app.renderer
                .upload_region_zones(Arc::clone(&identity), &region.geometry, zones);
        }
    }
    // One immutable accepted payload supplies picker, dependency, animation and status projections.
    app.world.region_installed(identity, &scope, region);
    if matches!(scope, world_runtime::RegionBuildScope::Full) {
        app.apply_project_region(RegionCoord::new(key.0, key.1));
    }
    app.diagnostics.event(
        "build.install",
        install_started.elapsed().as_secs_f32() * 1000.0,
    );
}

/// Empty means missing region source, including for a scoped request. Neither Empty nor Failed
/// authorizes semantic eviction or consumption of pending project restoration work.
pub(super) fn finish_unavailable_region(
    app: &mut EditorApp,
    region: RegionCoord,
    error: Option<&str>,
) {
    let key = (region.x, region.y);
    if app.world.build_finished(key).is_none() {
        return;
    }
    app.world.region_unavailable(key);
    hide_region(app, key);
    // Missing cache regions are ordinary residency outcomes. Report only current worker errors.
    if let Some(error) = error {
        app.ui
            .set_status(format!("region {},{} failed: {error}", key.0, key.1));
    }
}

/// Static zones have per-region ownership; animated locs share a batch. Hide that batch until
/// the worker rebuilds surviving membership, and invalidate interaction with removed picker data.
fn hide_region(app: &mut EditorApp, key: (i32, i32)) {
    app.renderer.remove_region(key);
    app.renderer
        .upload_transient(ANIMATED_KEY, &SceneGeometry::default());
    app.clear_selection();
}

/// Retire the FIFO ticket even for stale results. Only accepted candidates may hydrate semantics
/// or consume a pending Object comparison. Failure cleanup touches disposable world state only.
fn prepare_region_install(
    world: &mut WorldRuntime,
    session: &mut EditRuntime,
    candidate: &RegionGeometry,
) -> Result<Option<(RegionBuildScope, Arc<RegionArtifactIdentity>)>, RegionAdaptError> {
    let key = (candidate.region.x, candidate.region.y);
    let build = world.build_finished(key);
    if !world.is_current_generation(candidate.generation) {
        return Ok(None);
    }
    let Some(build) = build else { return Ok(None) };
    let snapshot = match adapt_region_map(&candidate.source) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            world.region_unavailable(key);
            return Err(error);
        }
    };
    let identity = build.artifact_identity(candidate);
    let scope = world.resolve_install_scope(key, build.into_scope(), &candidate.geometry);
    let _ = session.ingest_region(&snapshot);
    Ok(Some((scope, identity)))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod terminal_tests;

#[cfg(test)]
mod identity_tests;
