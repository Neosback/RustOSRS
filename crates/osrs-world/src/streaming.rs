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
use osrs_render::{SceneGeometry, gpu::TextureLayer};
use std::{
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
    use super::RegionRenderPath;

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
}
