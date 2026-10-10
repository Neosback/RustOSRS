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
    extract_region_info, texture_layers,
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
    let geometry = extract_owned_geometry(&world, owned);
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
        animations,
        info,
        build_ms: started.elapsed().as_secs_f32() * 1000.0,
        generation: 0,
    }))
}

/// Messages from the streaming worker.
pub enum StreamEvent {
    /// The cache is open; textures are ready to upload.
    Ready {
        textures: Vec<Option<TextureLayer>>,
    },
    Region(Box<RegionGeometry>),
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
                            let event =
                                match build_region_geometry(&mut definitions, region, presentation)
                                {
                                    Ok(Some(mut geometry)) => {
                                        geometry.generation = generation;
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
