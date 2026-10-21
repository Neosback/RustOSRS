//! Synthetic region inputs shared by candidate acceptance and lifecycle regression tests.
pub(in crate::app) use super::animation::tests::{animated_candidate, finish as finish_animation};
use super::*;
use osrs_cache::{decode::DecodedLocations, source::CacheSource};
use osrs_core::coords::MapTile;
use osrs_render::PackedVertex;
use osrs_scene::{
    SceneGrid, SceneReferenceFinalizer,
    collision::CollisionSnapshotBuilder,
    terrain_load::{RegionTerrain, RegionTerrainTile, RegionTileHeight, TerrainLoadGrid},
};
use osrs_world::{RegionGeometry, RegionMap, WorldScene};
use std::sync::Arc;

pub(in crate::app) const KEY: RegionKey = (50, 50);

pub(in crate::app) fn source() -> CacheSource {
    CacheSource::Memory(Arc::new(Default::default()))
}

pub(in crate::app) fn world() -> WorldRuntime {
    WorldRuntime::new(WorldRuntime::spawn_streamer(source()))
}

pub(in crate::app) fn request(world: &mut WorldRuntime, scope: RegionBuildScope) {
    world
        .builds
        .request(KEY, scope, world.presentation_generation);
}

pub(in crate::app) fn candidate() -> RegionGeometry {
    let region = RegionCoord::new(KEY.0, KEY.1);
    let window = osrs_world::region_window(region);
    let world = WorldScene {
        collision: Arc::new(
            CollisionSnapshotBuilder::reference(MapTile::new(window.base_x, window.base_y)).build(),
        ),
        window,
        load: TerrainLoadGrid::reference(),
        scene: SceneGrid::new(104, 104, 4).unwrap_or_else(|error| panic!("{error:?}")),
        finalizer: SceneReferenceFinalizer::new(104, 104, 4)
            .unwrap_or_else(|error| panic!("{error:?}")),
        lit: vec![],
        animated: vec![],
        locs: vec![],
        loc_stats: Default::default(),
        merge_report: Default::default(),
    };
    let info = osrs_world::extract_region_info(&world, region, ((16, 16), (80, 80)), &mut |_| None);
    let source = RegionMap {
        terrain: RegionTerrain::new(vec![
            RegionTerrainTile {
                height: RegionTileHeight::Explicit(0),
                overlay: None,
                settings: None,
                underlay: 1,
            };
            4 * 64 * 64
        ])
        .unwrap_or_else(|error| panic!("{error:?}")),
        locations: DecodedLocations::with_locations(region, vec![]),
    };
    let mut geometry = SceneGeometry::default();
    geometry.zone_mut(400, 400).group_mut(0, 0).opaque.extend(
        [PackedVertex {
            position: [0; 4],
            abhsl: 0,
            texture: 0,
            uv: [0; 2],
            pad: 0,
        }; 3],
    );
    RegionGeometry {
        region,
        target: fixture_target(),
        presentation: WorldRuntime::presentation(false, false),
        source: Arc::new(source),
        geometry,
        info: Arc::new(info),
        height_dependencies: Default::default(),
        render_path: osrs_world::RegionRenderPath::Reference,
        hd_stats: None,
        animations: vec![],
        build_ms: 0.0,
        timings: Default::default(),
        generation: 1,
    }
}

pub(in crate::app) fn actor_context() -> osrs_world::npc::ActorGeometryContext {
    osrs_world::npc::ActorGeometryContext {
        target: fixture_target(),
        presentation_generation: 1,
        render_path: osrs_world::RegionRenderPath::Reference,
    }
}

fn fixture_target() -> Arc<osrs_core::provenance::TargetProvenance> {
    let profiles =
        osrs_cache::registry::bundled_profiles().unwrap_or_else(|error| panic!("{error:?}"));
    let profile = profiles
        .first()
        .unwrap_or_else(|| panic!("bundled profile"));
    let context = osrs_cache::decode::DecoderContext::from_profile(profile)
        .unwrap_or_else(|error| panic!("{error:?}"));
    Arc::new(context.target_provenance().clone())
}

/// Synthetic seed identity only; integration tests issue real FIFO tickets through `request`.
pub(in crate::app) fn identity(
    world: &WorldRuntime,
    candidate: &RegionGeometry,
) -> Arc<RegionArtifactIdentity> {
    let stamp = builds::BuildStamp {
        request_id: 0,
        revision: 0,
        source: world.builds.source_generation(),
        projection_epoch: world
            .builds
            .projection_epoch((candidate.region.x, candidate.region.y)),
        presentation_generation: world.presentation_generation,
    };
    Arc::new(RegionArtifactIdentity::new(stamp, candidate))
}

pub(in crate::app) fn invalid_candidate() -> RegionGeometry {
    use osrs_cache::decode::DecodedLocation;
    use osrs_core::{
        coords::{RegionTile, SourcePlane},
        ids::ObjectId,
    };
    let mut candidate = candidate();
    Arc::get_mut(&mut candidate.source)
        .unwrap_or_else(|| panic!("unique source"))
        .locations = DecodedLocations::with_locations(
        candidate.region,
        vec![DecodedLocation {
            object_id: ObjectId::new(100),
            tile: RegionTile::new(0, 0).unwrap_or_else(|| panic!("valid tile")),
            source_plane: SourcePlane::new(0).unwrap_or_else(|| panic!("valid plane")),
            loc_type: 23,
            orientation: 0,
        }],
    );
    candidate
}

pub(in crate::app) fn seed_installed(world: &mut WorldRuntime, candidate: &RegionGeometry) {
    world.region_installed(
        identity(world, candidate),
        &RegionBuildScope::Full,
        candidate.clone(),
    );
    begin_object_proof(world);
}

pub(in crate::app) fn begin_object_proof(world: &mut WorldRuntime) {
    let stamp = ObjectOutputStamp::new(
        world.builds.projection_epoch(KEY),
        world.presentation_generation,
    );
    world.object_outputs.begin_proof(KEY, stamp);
}

/// Optional native adapter for lifecycle tests that must observe real transient GPU ownership.
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::app) fn render_state() -> Option<eframe::egui_wgpu::RenderState> {
    use eframe::{egui_wgpu, wgpu};
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    match pollster::block_on(egui_wgpu::RenderState::create(
        &egui_wgpu::WgpuConfiguration::default(),
        &instance,
        None,
        egui_wgpu::RendererOptions::default(),
    )) {
        Ok(state) => Some(state),
        Err(error) => {
            eprintln!("GPU projection test unavailable: {error}");
            None
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(in crate::app) fn actor_renderer() -> Option<crate::app::render_runtime::RenderRuntime> {
    render_state().map(|state| crate::app::render_runtime::RenderRuntime::new(state, (0, 0)))
}

/// Install only synthetic ground/inspection data for actor lifecycle tests.
pub(in crate::app) fn seed_info(world: &mut WorldRuntime, candidate: &RegionGeometry) {
    world.install_info(identity(world, candidate), Arc::clone(&candidate.info));
}
