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
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    thread::JoinHandle,
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
    Shutdown,
}

pub struct AssetWorker {
    requests: Sender<Request>,
    events: Receiver<AssetEvent>,
    epoch: Arc<AtomicU64>,
    worker: Option<JoinHandle<()>>,
}

impl AssetWorker {
    pub fn spawn(cache_dir: PathBuf) -> Self {
        let (request_tx, request_rx) = channel::<Request>();
        let (event_tx, event_rx) = channel::<AssetEvent>();
        let epoch = Arc::new(AtomicU64::new(0));
        let worker_epoch = Arc::clone(&epoch);
        let worker = std::thread::Builder::new()
            .name("assets".to_owned())
            .spawn(move || {
                let mut definitions = match WorldDefinitions::open(&cache_dir) {
                    Ok(definitions) => definitions,
                    Err(error) => {
                        let _ = event_tx.send(AssetEvent::Failed(error.to_string()));
                        return;
                    }
                };
                let catalog = match Catalog::load(&definitions) {
                    Ok(catalog) => Arc::new(catalog),
                    Err(error) => {
                        let _ = event_tx.send(AssetEvent::Failed(error.to_string()));
                        return;
                    }
                };
                let _ = event_tx.send(AssetEvent::WorldMapCatalog(Arc::clone(&catalog)));
                let mut renderer = RegionRenderer::new();
                while let Ok(request) = request_rx.recv() {
                    let event = match request {
                        Request::Shutdown => break,
                        Request::Sprite(id) => Some(match load_sprite(&definitions, id) {
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
                                    match renderer.render(
                                        &mut definitions,
                                        &catalog,
                                        area,
                                        region,
                                        pixels_per_tile,
                                    ) {
                                        Some(image) => AssetEvent::WorldMapRegion(Box::new(
                                            WorldMapRegionImage {
                                                area,
                                                region,
                                                pixels_per_tile,
                                                rgba: image.rgba,
                                            },
                                        )),
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
                            match minimap::render_region(&mut definitions, &catalog, region, plane)
                            {
                                Ok(Some(image)) => AssetEvent::MinimapRegion(Arc::new(image)),
                                Ok(None) => AssetEvent::MinimapEmpty(region, plane),
                                Err(error) => {
                                    AssetEvent::Failed(format!("minimap {region:?}: {error}"))
                                }
                            },
                        ),
                        Request::Icons { area } => Some(AssetEvent::WorldMapIcons {
                            area,
                            icons: Arc::new(worldmap::area_icons(
                                &mut definitions,
                                &catalog,
                                area,
                                true,
                            )),
                        }),
                        Request::Overview { area } => catalog
                            .areas
                            .get(area)
                            .and_then(|area_data| worldmap::load_overview(&definitions, area_data))
                            .map(|(width, height, rgba)| AssetEvent::WorldMapOverview {
                                area,
                                width,
                                height,
                                rgba,
                            }),
                    };
                    if let Some(event) = event
                        && event_tx.send(event).is_err()
                    {
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
