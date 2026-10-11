//! Background asset worker: interface sprites and the world map, read from the cache.
//!
//! One thread owns its own [`WorldDefinitions`] and answers requests, so the editor never touches
//! the cache from the render thread. Results come back as events to upload to textures.
//!
//! * **Sprites** (cache index 8): the compass, minimap frame, wiki icon and any other interface
//!   sprite, decoded to RGBA.
//! * **World map**: the client's own data and renderer, see [`crate::worldmap`]. The catalog (areas,
//!   elements) loads first; region images, icon lists and overview pictures are produced on
//!   request, newest view first (requests older than the current epoch are dropped).

use crate::{
    WorldDefinitions, WorldError,
    minimap::{self, MinimapRegion},
    worldmap::{self, Catalog, MapIcon, RegionRenderer},
};
use osrs_cache::decode::decode_sprite_group;
use osrs_cache::source::CacheSource;
#[cfg(not(target_arch = "wasm32"))]
use std::thread::JoinHandle;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
};

const SPRITE_INDEX: u8 = 8;

/// A decoded sprite (first frame, on its full canvas).
#[derive(Debug, Clone)]
pub struct SpriteImage {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    /// RGBA, row-major, straight alpha.
    pub rgba: Vec<u8>,
}

/// One rendered world-map region.
#[derive(Debug, Clone)]
pub struct WorldMapRegionImage {
    pub area: usize,
    pub region: (i32, i32),
    pub pixels_per_tile: i32,
    /// `64 * ppt` squared RGBA, row 0 = the northernmost tile row.
    pub rgba: Vec<u8>,
}

pub enum AssetEvent {
    Sprite(SpriteImage),
    SpriteMissing(u32),
    /// The world-map areas, elements and map scenes.
    WorldMapCatalog(Arc<Catalog>),
    /// A region has no data in its area (so it is not requested again).
    WorldMapRegionEmpty {
        area: usize,
        region: (i32, i32),
        pixels_per_tile: i32,
    },
    WorldMapRegion(Box<WorldMapRegionImage>),
    WorldMapIcons {
        area: usize,
        icons: Arc<Vec<MapIcon>>,
        /// Element id -> object name (what the key shows).
        names: Arc<std::collections::HashMap<u32, String>>,
    },
    WorldMapOverview {
        area: usize,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    /// A minimap region image (see [`crate::minimap`]).
    MinimapRegion(Arc<MinimapRegion>),
    /// The region has no map data (`region`, `plane`).
    MinimapEmpty((i32, i32), u8),
    Failed(String),
}

enum Request {
    Sprite(u32),
    Region {
        area: usize,
        region: (i32, i32),
        pixels_per_tile: i32,
        epoch: u64,
    },
    Minimap {
        region: (i32, i32),
        plane: u8,
    },
    Icons {
        area: usize,
    },
    Overview {
        area: usize,
    },
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Shutdown,
}

/// What the worker owns: the cache, the world-map catalog and the region renderer.
struct AssetState {
    definitions: WorldDefinitions,
    catalog: Arc<Catalog>,
    renderer: RegionRenderer,
    epoch: Arc<AtomicU64>,
}

impl AssetState {
    fn open(
        source: &CacheSource,
        events: &Sender<AssetEvent>,
        epoch: Arc<AtomicU64>,
    ) -> Option<Self> {
        let definitions = match WorldDefinitions::open(source) {
            Ok(definitions) => definitions,
            Err(error) => {
                let _ = events.send(AssetEvent::Failed(error.to_string()));
                return None;
            }
        };
        let catalog = match Catalog::load(&definitions) {
            Ok(catalog) => Arc::new(catalog),
            Err(error) => {
                let _ = events.send(AssetEvent::Failed(error.to_string()));
                return None;
            }
        };
        let _ = events.send(AssetEvent::WorldMapCatalog(Arc::clone(&catalog)));
        Some(Self {
            definitions,
            catalog,
            renderer: RegionRenderer::new(),
            epoch,
        })
    }

    /// Handle one request; `false` when the worker should stop.
    fn handle(&mut self, request: Request, events: &Sender<AssetEvent>) -> bool {
        let Self {
            definitions,
            catalog,
            renderer,
            epoch: worker_epoch,
        } = self;
        let event = match request {
            Request::Shutdown => return false,
            Request::Sprite(id) => Some(match load_sprite(definitions, id) {
                Ok(Some(sprite)) => AssetEvent::Sprite(sprite),
                Ok(None) => AssetEvent::SpriteMissing(id),
                Err(error) => AssetEvent::Failed(format!("sprite {id}: {error}")),
            }),
            Request::Region {
                area,
                region,
                pixels_per_tile,
                epoch,
            } => {
                if epoch < worker_epoch.load(Ordering::Relaxed) {
                    None
                } else {
                    Some(
                        match renderer.render(definitions, &catalog, area, region, pixels_per_tile)
                        {
                            Some(image) => {
                                AssetEvent::WorldMapRegion(Box::new(WorldMapRegionImage {
                                    area,
                                    region,
                                    pixels_per_tile,
                                    rgba: image.rgba,
                                }))
                            }
                            None => AssetEvent::WorldMapRegionEmpty {
                                area,
                                region,
                                pixels_per_tile,
                            },
                        },
                    )
                }
            }
            Request::Minimap { region, plane } => Some(
                match minimap::render_region(definitions, &catalog, region, plane) {
                    Ok(Some(image)) => AssetEvent::MinimapRegion(Arc::new(image)),
                    Ok(None) => AssetEvent::MinimapEmpty(region, plane),
                    Err(error) => AssetEvent::Failed(format!("minimap {region:?}: {error}")),
                },
            ),
            Request::Icons { area } => {
                let (icons, names) = worldmap::area_icons(definitions, &catalog, area, true);
                Some(AssetEvent::WorldMapIcons {
                    area,
                    icons: Arc::new(icons),
                    names: Arc::new(names),
                })
            }
            Request::Overview { area } => catalog
                .areas
                .get(area)
                .and_then(|area_data| worldmap::load_overview(definitions, area_data))
                .map(|(width, height, rgba)| AssetEvent::WorldMapOverview {
                    area,
                    width,
                    height,
                    rgba,
                }),
        };

        match event {
            Some(event) => events.send(event).is_ok(),
            None => true,
        }
    }
}

/// Small jobs (sprites, icon lists, overview, minimap) go before region images, so the key and
/// icons do not wait behind hundreds of queued regions.
fn next_request(pending: &mut VecDeque<Request>) -> Option<Request> {
    let next = pending
        .iter()
        .position(|request| !matches!(request, Request::Region { .. }))
        .unwrap_or(0);
    pending.remove(next)
}

/// Owns the cache on a worker thread (native) or inside the owner, run a slice per frame through
/// [`AssetWorker::pump`] (the browser has no threads).
pub struct AssetWorker {
    requests: Sender<Request>,
    events: Receiver<AssetEvent>,
    epoch: Arc<AtomicU64>,
    #[cfg(not(target_arch = "wasm32"))]
    worker: Option<JoinHandle<()>>,
    #[cfg(target_arch = "wasm32")]
    local: std::cell::RefCell<LocalAssets>,
}

#[cfg(target_arch = "wasm32")]
struct LocalAssets {
    queue: Receiver<Request>,
    events: Sender<AssetEvent>,
    opening: Option<CacheSource>,
    state: Option<AssetState>,
    pending: VecDeque<Request>,
}

impl AssetWorker {
    pub fn spawn(source: impl Into<CacheSource>) -> Self {
        let source = source.into();
        let (request_tx, request_rx) = channel::<Request>();
        let (event_tx, event_rx) = channel::<AssetEvent>();
        let epoch = Arc::new(AtomicU64::new(0));
        #[cfg(not(target_arch = "wasm32"))]
        {
            let worker_epoch = Arc::clone(&epoch);
            let worker = std::thread::Builder::new()
                .name("assets".to_owned())
                .spawn(move || {
                    let Some(mut state) = AssetState::open(&source, &event_tx, worker_epoch) else {
                        return;
                    };
                    let mut pending: VecDeque<Request> = VecDeque::new();
                    loop {
                        if pending.is_empty() {
                            match request_rx.recv() {
                                Ok(request) => pending.push_back(request),
                                Err(_) => break,
                            }
                        }
                        while let Ok(request) = request_rx.try_recv() {
                            pending.push_back(request);
                        }
                        let Some(request) = next_request(&mut pending) else {
                            continue;
                        };
                        if !state.handle(request, &event_tx) {
                            break;
                        }
                    }
                })
                .ok();
            Self {
                requests: request_tx,
                events: event_rx,
                epoch,
                worker,
            }
        }
        #[cfg(target_arch = "wasm32")]
        Self {
            requests: request_tx,
            events: event_rx,
            epoch,
            local: std::cell::RefCell::new(LocalAssets {
                queue: request_rx,
                events: event_tx,
                opening: Some(source),
                state: None,
                pending: VecDeque::new(),
            }),
        }
    }

    /// Run queued work for about `budget` (at least one request). A no-op where the worker has a
    /// thread of its own.
    #[cfg(target_arch = "wasm32")]
    pub fn pump(&self, budget: std::time::Duration) {
        let started = web_time::Instant::now();
        let mut local = self.local.borrow_mut();
        let local = &mut *local;
        if let Some(source) = local.opening.take() {
            local.state = AssetState::open(&source, &local.events, Arc::clone(&self.epoch));
            return;
        }
        let Some(state) = local.state.as_mut() else {
            return;
        };
        while let Ok(request) = local.queue.try_recv() {
            local.pending.push_back(request);
        }
        while let Some(request) = next_request(&mut local.pending) {
            if !state.handle(request, &local.events) {
                local.state = None;
                return;
            }
            if started.elapsed() >= budget {
                break;
            }
        }
    }

    /// Run queued work for about `budget`. A no-op where the worker has a thread of its own.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn pump(&self, _budget: std::time::Duration) {}

    pub fn request_sprite(&self, id: u32) {
        let _ = self.requests.send(Request::Sprite(id));
    }

    /// Drop every queued region request older than the one this returns the epoch of.
    pub fn new_epoch(&self) -> u64 {
        self.epoch.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn request_region(
        &self,
        area: usize,
        region: (i32, i32),
        pixels_per_tile: i32,
        epoch: u64,
    ) {
        let _ = self.requests.send(Request::Region {
            area,
            region,
            pixels_per_tile,
            epoch,
        });
    }

    pub fn request_minimap(&self, region: (i32, i32), plane: u8) {
        let _ = self.requests.send(Request::Minimap { region, plane });
    }

    pub fn request_icons(&self, area: usize) {
        let _ = self.requests.send(Request::Icons { area });
    }

    pub fn request_overview(&self, area: usize) {
        let _ = self.requests.send(Request::Overview { area });
    }

    pub fn try_recv(&self) -> Option<AssetEvent> {
        self.events.try_recv().ok()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for AssetWorker {
    fn drop(&mut self) {
        let _ = self.requests.send(Request::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn load_sprite(definitions: &WorldDefinitions, id: u32) -> Result<Option<SpriteImage>, WorldError> {
    let Ok(file) = definitions.cache().read_file(SPRITE_INDEX, id, 0, None) else {
        return Ok(None);
    };
    let frames = decode_sprite_group(id, &file.bytes, definitions.context(), &file.provenance)
        .map_err(WorldError::Decode)?;
    let Some(frame) = frames.first() else {
        return Ok(None);
    };
    let (width, height) = (usize::from(frame.max_width), usize::from(frame.max_height));
    let indices = frame.normalized_indices();
    let alpha = frame.alpha.as_ref().map(|alpha| {
        // Expand the per-pixel alpha onto the same canvas.
        let mut canvas = vec![0_u8; width * height];
        let (fw, fh) = (usize::from(frame.width), usize::from(frame.height));
        for row in 0..fh {
            for column in 0..fw {
                let target = (row + usize::from(frame.y_offset)) * width
                    + column
                    + usize::from(frame.x_offset);
                if let Some(slot) = canvas.get_mut(target) {
                    *slot = alpha[row * fw + column];
                }
            }
        }
        canvas
    });
    let mut rgba = vec![0_u8; width * height * 4];
    for (pixel, &index) in indices.iter().enumerate() {
        if index == 0 {
            continue;
        }
        let rgb = frame.palette.get(usize::from(index)).copied().unwrap_or(0);
        rgba[pixel * 4] = (rgb >> 16) as u8;
        rgba[pixel * 4 + 1] = (rgb >> 8) as u8;
        rgba[pixel * 4 + 2] = rgb as u8;
        rgba[pixel * 4 + 3] = alpha.as_ref().map_or(255, |alpha| alpha[pixel]);
    }
    Ok(Some(SpriteImage {
        id,
        width: width as u32,
        height: height as u32,
        rgba,
    }))
}
