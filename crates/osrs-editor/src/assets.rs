//! Editor-side asset cache: sprites and the world map delivered by the `AssetWorker` thread,
//! uploaded as egui textures. Everything is requested lazily and at most once.

use eframe::egui;
use osrs_world::{
    AssetEvent, AssetWorker, SpriteImage, WorldMapRegionImage,
    minimap::MinimapRegion,
    worldmap::{Catalog, MapIcon},
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

/// Interface sprite ids used by the editor (the same ids as the game's `SpriteID`).
pub mod sprite {
    #[allow(dead_code, reason = "minimap frame: used by the minimap HUD")]
    pub const MAP_BACK: u32 = 12;
    pub const COMPASS: u32 = 169;
    #[allow(dead_code, reason = "wiki button sprite for the Classic skin")]
    pub const WIKI_ICON: u32 = 2420;
}

/// A minimap region cache key: region and plane.
pub type MinimapKey = ((i32, i32), u8);

/// A world-map region image: area index, region, pixels per tile.
pub type RegionKey = (usize, i32, i32, i32);

/// Region textures kept before the ones for other zoom levels are dropped.
const REGION_TEXTURE_BUDGET: usize = 700;

pub struct Assets {
    worker: AssetWorker,
    sprites: HashMap<u32, egui::TextureHandle>,
    /// CPU copies of sprites the minimap composites itself (masks, the compass).
    sprite_pixels: HashMap<u32, Arc<SpriteImage>>,
    /// Minimap regions: `None` = no data.
    minimap: HashMap<MinimapKey, Option<Arc<MinimapRegion>>>,
    minimap_requests: HashSet<MinimapKey>,
    missing: HashSet<u32>,
    requested: HashSet<u32>,
    pub catalog: Option<Arc<Catalog>>,
    /// `None` = the area has no data for that region.
    regions: HashMap<RegionKey, Option<egui::TextureHandle>>,
    region_requests: HashSet<RegionKey>,
    icons: HashMap<usize, Arc<Vec<MapIcon>>>,
    icon_requests: HashSet<usize>,
    overviews: HashMap<usize, egui::TextureHandle>,
    overview_requests: HashSet<usize>,
    epoch: u64,
    pub status: Option<String>,
}

impl Assets {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            worker: AssetWorker::spawn(cache_dir),
            sprites: HashMap::new(),
            sprite_pixels: HashMap::new(),
            minimap: HashMap::new(),
            minimap_requests: HashSet::new(),
            missing: HashSet::new(),
            requested: HashSet::new(),
            catalog: None,
            regions: HashMap::new(),
            region_requests: HashSet::new(),
            icons: HashMap::new(),
            icon_requests: HashSet::new(),
            overviews: HashMap::new(),
            overview_requests: HashSet::new(),
            epoch: 0,
            status: None,
        }
    }

    /// The sprite texture, requesting it on first use.
    pub fn sprite(&mut self, id: u32) -> Option<&egui::TextureHandle> {
        if !self.sprites.contains_key(&id)
            && !self.missing.contains(&id)
            && self.requested.insert(id)
        {
            self.worker.request_sprite(id);
        }
        self.sprites.get(&id)
    }

    /// The decoded pixels of a sprite (requested like [`Self::sprite`]).
    pub fn sprite_image(&mut self, id: u32) -> Option<Arc<SpriteImage>> {
        self.sprite(id);
        self.sprite_pixels.get(&id).cloned()
    }

    /// One minimap region, requesting it when unknown. `Some(None)` means "no data here".
    pub fn minimap_region(
        &mut self,
        region: (i32, i32),
        plane: u8,
    ) -> Option<Option<Arc<MinimapRegion>>> {
        let key = (region, plane);
        if !self.minimap.contains_key(&key) && self.minimap_requests.insert(key) {
            self.worker.request_minimap(region, plane);
        }
        self.minimap.get(&key).cloned()
    }

    /// Forget queued region work (the view changed area or zoom).
    pub fn new_map_view(&mut self) {
        self.epoch = self.worker.new_epoch();
        self.region_requests.clear();
        if self.regions.len() > REGION_TEXTURE_BUDGET {
            self.regions.clear();
        }
    }

    /// The texture of a region, requesting it when unknown. `Some(None)` means "no data here".
    pub fn region(&mut self, key: RegionKey) -> Option<Option<&egui::TextureHandle>> {
        if !self.regions.contains_key(&key) && self.region_requests.insert(key) {
            self.worker
                .request_region(key.0, (key.1, key.2), key.3, self.epoch);
        }
        self.regions.get(&key).map(Option::as_ref)
    }

    pub fn region_requests_in_flight(&self) -> usize {
        self.region_requests.len()
    }

    pub fn icons(&mut self, area: usize) -> Option<Arc<Vec<MapIcon>>> {
        if !self.icons.contains_key(&area) && self.icon_requests.insert(area) {
            self.worker.request_icons(area);
        }
        self.icons.get(&area).cloned()
    }

    pub fn overview(&mut self, area: usize) -> Option<&egui::TextureHandle> {
        if !self.overviews.contains_key(&area) && self.overview_requests.insert(area) {
            self.worker.request_overview(area);
        }
        self.overviews.get(&area)
    }

    /// Upload what the worker finished since the last call; returns whether anything arrived.
    pub fn poll(&mut self, ctx: &egui::Context) -> bool {
        let mut changed = false;
        for _ in 0..64 {
            let Some(event) = self.worker.try_recv() else {
                break;
            };
            changed = true;
            match event {
                AssetEvent::Sprite(sprite) => {
                    let (id, width, height) = (sprite.id, sprite.width, sprite.height);
                    let rgba = &sprite.rgba;
                    self.sprite_pixels.insert(id, Arc::new(sprite.clone()));
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [width as usize, height as usize],
                        rgba,
                    );
                    let handle = ctx.load_texture(
                        format!("sprite-{id}"),
                        image,
                        egui::TextureOptions::NEAREST,
                    );
                    self.sprites.insert(id, handle);
                }
                AssetEvent::SpriteMissing(id) => {
                    self.missing.insert(id);
                }
                AssetEvent::WorldMapCatalog(catalog) => self.catalog = Some(catalog),
                AssetEvent::WorldMapRegionEmpty {
                    area,
                    region,
                    pixels_per_tile,
                } => {
                    let key = (area, region.0, region.1, pixels_per_tile);
                    self.region_requests.remove(&key);
                    self.regions.insert(key, None);
                }
                AssetEvent::WorldMapRegion(image) => {
                    let WorldMapRegionImage {
                        area,
                        region,
                        pixels_per_tile,
                        rgba,
                    } = *image;
                    let key = (area, region.0, region.1, pixels_per_tile);
                    let side = (64 * pixels_per_tile) as usize;
                    let color = egui::ColorImage::from_rgba_unmultiplied([side, side], &rgba);
                    let handle = ctx.load_texture(
                        format!("map-{area}-{}-{}-{pixels_per_tile}", region.0, region.1),
                        color,
                        egui::TextureOptions::NEAREST,
                    );
                    self.region_requests.remove(&key);
                    self.regions.insert(key, Some(handle));
                }
                AssetEvent::WorldMapIcons { area, icons } => {
                    self.icons.insert(area, icons);
                }
                AssetEvent::WorldMapOverview {
                    area,
                    width,
                    height,
                    rgba,
                } => {
                    let color = egui::ColorImage::from_rgba_unmultiplied(
                        [width as usize, height as usize],
                        &rgba,
                    );
                    let handle = ctx.load_texture(
                        format!("overview-{area}"),
                        color,
                        egui::TextureOptions::LINEAR,
                    );
                    self.overviews.insert(area, handle);
                }
                AssetEvent::MinimapRegion(region) => {
                    self.minimap
                        .insert((region.region, region.plane), Some(region));
                }
                AssetEvent::MinimapEmpty(region, plane) => {
                    self.minimap.insert((region, plane), None);
                }
                AssetEvent::Failed(message) => self.status = Some(message),
            }
        }
        changed
    }
}
