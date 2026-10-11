//! Region streaming: one scene window per map region, built on a worker thread.
//!
//! The reference scene is a fixed 104x104 window, and terrain blending and normal merging need
//! tile context around every tile. To stream an unbounded world, each map region is built from
//! its own window (the region plus [`REGION_WINDOW_MARGIN`] tiles before it and 24 after) and
//! only the region's own 64x64 tiles are kept, so every kept tile has at least 16 tiles of
//! context in every direction.

use crate::{
    AnimatedInstance, OwnedTiles, RegionInfo, SceneWindow, TerrainPresentation, WorldDefinitions,
    WorldError, build_world_scene, extract_animated_instances, extract_owned_geometry,
    extract_region_info,
    hd::{HdContext, extract::extract_owned_geometry_hd, gpu::HdGpuData},
    texture_layers,
};
use osrs_core::coords::RegionCoord;
use osrs_render::{HD_FLAG_TERRAIN, HD_MATERIAL_INDEX_MASK, SceneGeometry, gpu::TextureLayer};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::Arc,
    sync::mpsc::{Receiver, Sender, channel},
    thread::JoinHandle,
    time::Instant,
};

/// Tiles of context before a region inside its window.
pub const REGION_WINDOW_MARGIN: i32 = 16;
const REGION_TILES: u32 = 64;

/// Which extraction path actually produced a region's geometry.
///
/// `ReferenceFallback` is deliberately distinct from `Reference`: it means the caller requested
/// HD geometry but no HD context was available, so the region would otherwise look like a valid
/// Reference build and hide the real failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionRenderPath {
    Reference,
    Hd,
    ReferenceFallback,
}

impl RegionRenderPath {
    const fn select(hd_requested: bool, hd_available: bool) -> Self {
        match (hd_requested, hd_available) {
            (false, _) => Self::Reference,
            (true, true) => Self::Hd,
            (true, false) => Self::ReferenceFallback,
        }
    }
}

/// Material coverage of one region after HD extraction.
///
/// Each HD triangle repeats the three corner material indices on all three vertices. Statistics
/// therefore sample the first vertex of each triangle instead of triple-counting the face.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HdRegionStats {
    pub faces: usize,
    pub opaque_faces: usize,
    pub alpha_faces: usize,
    pub terrain_faces: usize,
    pub model_faces: usize,
    /// Faces where at least one corner carries a non-`NONE` material index.
    pub materialized_faces: usize,
    /// Faces whose three material indices are all `NONE`.
    pub none_material_faces: usize,
    pub unique_materials: usize,
    pub max_material_index: u16,
}

impl HdRegionStats {
    fn from_geometry(geometry: &SceneGeometry) -> Self {
        let mut stats = Self::default();
        let mut unique = BTreeSet::new();
        for zone in &geometry.zones {
            for group in &zone.groups {
                accumulate_hd_faces(&mut stats, &mut unique, &group.geometry.hd_opaque, false);
                accumulate_hd_faces(&mut stats, &mut unique, &group.geometry.hd_alpha, true);
            }
        }
        stats.unique_materials = unique.len();
        stats
    }
}

fn accumulate_hd_faces(
    stats: &mut HdRegionStats,
    unique: &mut BTreeSet<u16>,
    vertices: &[osrs_render::HdVertex],
    alpha: bool,
) {
    for face in vertices.chunks_exact(3) {
        let vertex = &face[0];
        stats.faces += 1;
        if alpha {
            stats.alpha_faces += 1;
        } else {
            stats.opaque_faces += 1;
        }
        if vertex.hsl_flags[3] & HD_FLAG_TERRAIN != 0 {
            stats.terrain_faces += 1;
        } else {
            stats.model_faces += 1;
        }

        let mut materialized = false;
        for packed in &vertex.material_water[..3] {
            let index = *packed & HD_MATERIAL_INDEX_MASK;
            if index != 0 {
                materialized = true;
                unique.insert(index);
                stats.max_material_index = stats.max_material_index.max(index);
            }
        }
        if materialized {
            stats.materialized_faces += 1;
        } else {
            stats.none_material_faces += 1;
        }
    }
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

/// The scene window used to build `region`.
pub fn region_window(region: RegionCoord) -> SceneWindow {
    SceneWindow {
        base_x: region.x * 64 - REGION_WINDOW_MARGIN,
        base_y: region.y * 64 - REGION_WINDOW_MARGIN,
    }
}

/// Geometry of one region, in world zone coordinates.
#[derive(Debug, Clone)]
pub struct RegionGeometry {
    pub region: RegionCoord,
    pub geometry: SceneGeometry,
    /// The extraction path that actually produced `geometry`.
    pub render_path: RegionRenderPath,
    /// Material coverage when [`RegionRenderPath::Hd`] produced this region.
    pub hd_stats: Option<HdRegionStats>,
    /// Animated locs of the region, posed at runtime.
    pub animations: Vec<AnimatedInstance>,
    /// Inspection data for the picker.
    pub info: Arc<RegionInfo>,
    pub build_ms: f32,
    /// Presentation generation the region was built under (see [`RegionStreamer::set_presentation`]).
    pub generation: u32,
}

/// Build one region's geometry, or `None` when the cache has no map data for it.
pub fn build_region_geometry(
    definitions: &mut WorldDefinitions,
    region: RegionCoord,
    presentation: TerrainPresentation,
) -> Result<Option<RegionGeometry>, WorldError> {
    build_region_geometry_with(definitions, region, presentation, None)
}

/// [`build_region_geometry`] with the HD data of the 117 HD profile: when `hd` is given and
/// `presentation.hd_geometry` is set the geometry carries the HD vertex stream. When HD is
/// requested without an HD context, the resulting region is marked [`RegionRenderPath::ReferenceFallback`].
pub fn build_region_geometry_with(
    definitions: &mut WorldDefinitions,
    region: RegionCoord,
    presentation: TerrainPresentation,
    hd: Option<&HdContext>,
) -> Result<Option<RegionGeometry>, WorldError> {
    if definitions.region(region)?.is_none() {
        return Ok(None);
    }
    let started = Instant::now();
    let window = region_window(region);
    let world = build_world_scene(definitions, window, presentation)?;
    let margin = REGION_WINDOW_MARGIN as u32;
    let owned = Some(OwnedTiles {
        min: (margin, margin),
        max: (margin + REGION_TILES, margin + REGION_TILES),
    });
    let render_path = RegionRenderPath::select(presentation.hd_geometry, hd.is_some());
    let geometry = match (render_path, hd) {
        (RegionRenderPath::Hd, Some(hd)) => extract_owned_geometry_hd(&world, owned, hd),
        _ => extract_owned_geometry(&world, owned),
    };
    let hd_stats =
        (render_path == RegionRenderPath::Hd).then(|| HdRegionStats::from_geometry(&geometry));
    let animations = extract_animated_instances(&world, owned);
    let info = Arc::new(extract_region_info(
        &world,
        region,
        (
            (margin, margin),
            (margin + REGION_TILES, margin + REGION_TILES),
        ),
        &mut |id| definitions.object(id).ok().flatten(),
    ));
    Ok(Some(RegionGeometry {
        region,
        geometry,
        render_path,
        hd_stats,
        animations,
        info,
        build_ms: started.elapsed().as_secs_f32() * 1000.0,
        generation: 0,
    }))
}

/// Load the 117 HD data and prepare its GPU material table (textures decode on all cores).
fn load_hd(definitions: &WorldDefinitions) -> Result<(HdContext, Box<HdGpuData>), String> {
    let assets = crate::hd::default_assets_dir();
    let textures = definitions.textures();
    let present = textures
        .images
        .iter()
        .enumerate()
        .filter(|(_, image)| image.is_some())
        .map(|(id, _)| id as i32);
    let context = HdContext::load(&assets, osrs_hd::settings::HdSettings::default(), present)
        .map_err(|error| error.to_string())?;
    let gpu = context
        .prepare_gpu(&assets, textures, HdGpuData::RESOLUTION)
        .map_err(|error| error.to_string())?;
    Ok((context, Box::new(gpu)))
}

/// Messages from the streaming worker.
pub enum StreamEvent {
    /// The cache is open; textures are ready to upload.
    Ready {
        textures: Vec<Option<TextureLayer>>,
    },
    Region(Box<RegionGeometry>),
    /// The HD profile's material table and textures, decoded (sent once, before the first HD
    /// region).
    HdMaterials(Box<HdGpuData>),
    /// The 117 HD data could not be loaded; regions keep Reference geometry unless
    /// `RUSTOSRS_HD_STRICT=1` is set.
    HdFailed(String),
    /// The cache has no map data for this region (open ocean, unused squares).
    Empty(RegionCoord),
    /// Triangles of one picked loc (`[[x, height, z]; 3]` world units).
    Outline {
        region: (i32, i32),
        index: usize,
        triangles: Vec<[[i32; 3]; 3]>,
    },
    Failed(RegionCoord, String),
    InitFailed(String),
}

enum Request {
    Build(RegionCoord),
    SetPresentation(TerrainPresentation, u32),
    Outline {
        region: (i32, i32),
        index: usize,
        loc: Box<crate::LocInfo>,
    },
    Shutdown,
}

/// Background builder that owns the cache and definitions.
pub struct RegionStreamer {
    requests: Sender<Request>,
    events: Receiver<StreamEvent>,
    worker: Option<JoinHandle<()>>,
}

impl RegionStreamer {
    pub fn spawn(cache_dir: PathBuf, presentation: TerrainPresentation) -> Self {
        let (request_tx, request_rx) = channel::<Request>();
        let (event_tx, event_rx) = channel::<StreamEvent>();
        let worker = std::thread::Builder::new()
            .name("region-streamer".to_owned())
            .spawn(move || {
                let mut definitions = match WorldDefinitions::open(&cache_dir) {
                    Ok(definitions) => definitions,
                    Err(error) => {
                        let _ = event_tx.send(StreamEvent::InitFailed(error.to_string()));
                        return;
                    }
                };
                let _ = event_tx.send(StreamEvent::Ready {
                    textures: texture_layers(definitions.textures()),
                });
                let mut presentation = presentation;
                let mut generation = 0_u32;
                let hd_strict = env_flag("RUSTOSRS_HD_STRICT");
                let hd_diagnostics = env_flag("RUSTOSRS_HD_DIAGNOSTICS");
                // Loaded the first time an HD region is requested.
                let mut hd: Option<HdContext> = None;
                let mut hd_failure: Option<String> = None;
                while let Ok(request) = request_rx.recv() {
                    match request {
                        Request::Shutdown => break,
                        Request::Outline { region, index, loc } => {
                            let triangles = crate::loc_outline_triangles(&mut definitions, &loc)
                                .unwrap_or_default();
                            let _ = event_tx.send(StreamEvent::Outline {
                                region,
                                index,
                                triangles,
                            });
                        }
                        Request::SetPresentation(new_presentation, new_generation) => {
                            presentation = new_presentation;
                            generation = new_generation;
                        }
                        Request::Build(region) => {
                            if presentation.hd_geometry && hd.is_none() && hd_failure.is_none() {
                                match load_hd(&definitions) {
                                    Ok((context, gpu)) => {
                                        hd = Some(context);
                                        let _ = event_tx.send(StreamEvent::HdMaterials(gpu));
                                    }
                                    Err(error) => {
                                        hd_failure = Some(error.clone());
                                        let _ = event_tx.send(StreamEvent::HdFailed(error));
                                    }
                                }
                            }

                            if presentation.hd_geometry && hd.is_none() && hd_strict {
                                let reason = hd_failure
                                    .as_deref()
                                    .unwrap_or("HD context is unavailable");
                                if event_tx
                                    .send(StreamEvent::Failed(
                                        region,
                                        format!(
                                            "117 HD strict mode blocked Reference fallback: {reason}"
                                        ),
                                    ))
                                    .is_err()
                                {
                                    break;
                                }
                                continue;
                            }

                            let event = match build_region_geometry_with(
                                &mut definitions,
                                region,
                                presentation,
                                hd.as_ref(),
                            ) {
                                Ok(Some(mut geometry)) => {
                                    geometry.generation = generation;
                                    if geometry.render_path == RegionRenderPath::ReferenceFallback {
                                        let reason = hd_failure
                                            .as_deref()
                                            .unwrap_or("HD context is unavailable");
                                        eprintln!(
                                            "117 HD fallback for region {},{}: {reason}",
                                            region.x, region.y
                                        );
                                    } else if hd_diagnostics
                                        && let Some(stats) = geometry.hd_stats
                                    {
                                        eprintln!(
                                            "117 HD region {},{}: faces={} opaque={} alpha={} terrain={} models={} materialized={} none={} unique_materials={} max_material={}",
                                            region.x,
                                            region.y,
                                            stats.faces,
                                            stats.opaque_faces,
                                            stats.alpha_faces,
                                            stats.terrain_faces,
                                            stats.model_faces,
                                            stats.materialized_faces,
                                            stats.none_material_faces,
                                            stats.unique_materials,
                                            stats.max_material_index,
                                        );
                                    }
                                    StreamEvent::Region(Box::new(geometry))
                                }
                                Ok(None) => StreamEvent::Empty(region),
                                Err(error) => StreamEvent::Failed(region, error.to_string()),
                            };
                            if event_tx.send(event).is_err() {
                                break;
                            }
                        }
                    }
                }
            })
            .ok();
        Self {
            requests: request_tx,
            events: event_rx,
            worker,
        }
    }

    /// Change how later builds are presented. Regions built under an older `generation` carry
    /// that generation in [`RegionGeometry::generation`] so the caller can discard them.
    pub fn set_presentation(&self, presentation: TerrainPresentation, generation: u32) {
        let _ = self
            .requests
            .send(Request::SetPresentation(presentation, generation));
    }

    /// Ask the worker for the triangles of a picked loc.
    pub fn request_outline(&self, region: (i32, i32), index: usize, loc: crate::LocInfo) {
        let _ = self.requests.send(Request::Outline {
            region,
            index,
            loc: Box::new(loc),
        });
    }

    /// Queue a region build.
    pub fn request(&self, region: RegionCoord) {
        let _ = self.requests.send(Request::Build(region));
    }

    /// Next finished event, if any.
    pub fn try_recv(&self) -> Option<StreamEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for RegionStreamer {
    fn drop(&mut self) {
        let _ = self.requests.send(Request::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HdRegionStats, RegionRenderPath, accumulate_hd_faces};
    use osrs_render::{HD_FLAG_TERRAIN, HD_MATERIAL_OVERLAY, HdVertex};
    use std::collections::BTreeSet;

    #[test]
    fn render_path_distinguishes_reference_from_hd_fallback() {
        assert_eq!(
            RegionRenderPath::select(false, false),
            RegionRenderPath::Reference
        );
        assert_eq!(
            RegionRenderPath::select(false, true),
            RegionRenderPath::Reference
        );
        assert_eq!(RegionRenderPath::select(true, true), RegionRenderPath::Hd);
        assert_eq!(
            RegionRenderPath::select(true, false),
            RegionRenderPath::ReferenceFallback
        );
    }

    #[test]
    fn hd_stats_count_faces_not_repeated_vertex_payloads() {
        let material_vertex = HdVertex {
            normal: [0; 2],
            uv: [0.0; 2],
            hsl_flags: [0, 0, 0, HD_FLAG_TERRAIN],
            material_water: [2, 3 | HD_MATERIAL_OVERLAY, 0, 0],
            depth: [0; 4],
        };
        let none_vertex = HdVertex {
            hsl_flags: [0; 4],
            material_water: [0; 4],
            ..material_vertex
        };
        let mut stats = HdRegionStats::default();
        let mut unique = BTreeSet::new();
        accumulate_hd_faces(&mut stats, &mut unique, &[material_vertex; 3], false);
        accumulate_hd_faces(&mut stats, &mut unique, &[none_vertex; 3], true);
        stats.unique_materials = unique.len();

        assert_eq!(stats.faces, 2);
        assert_eq!(stats.opaque_faces, 1);
        assert_eq!(stats.alpha_faces, 1);
        assert_eq!(stats.terrain_faces, 1);
        assert_eq!(stats.model_faces, 1);
        assert_eq!(stats.materialized_faces, 1);
        assert_eq!(stats.none_material_faces, 1);
        assert_eq!(stats.unique_materials, 2);
        assert_eq!(stats.max_material_index, 3);
    }
}
