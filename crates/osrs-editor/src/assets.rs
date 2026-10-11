//! Editor-side asset cache: sprites and world-map tiles delivered by the `AssetWorker` thread,
//! uploaded as egui textures. Everything is requested lazily and at most once.

use eframe::egui;
use osrs_world::{AssetEvent, AssetWorker, SpriteImage, WORLD_MAP_IMAGE, WorldMapTile};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Interface sprite ids used by the editor (the same ids as the game's `SpriteID`).
pub mod sprite {
    #[allow(dead_code, reason = "minimap frame: used by the minimap HUD")]
    pub const MAP_BACK: u32 = 12;
    pub const COMPASS: u32 = 169;
    #[allow(dead_code, reason = "wiki button sprite for the Classic skin")]
    pub const WIKI_ICON: u32 = 2420;
}

pub struct Assets {
    worker: AssetWorker,
    sprites: HashMap<u32, egui::TextureHandle>,
    missing: HashSet<u32>,
    requested: HashSet<u32>,
    /// World-map region textures, keyed by region.
    pub map_tiles: HashMap<(i32, i32), egui::TextureHandle>,
    pub map_requested: bool,
    pub map_done: bool,
    pub status: Option<String>,
}

impl Assets {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            worker: AssetWorker::spawn(cache_dir),
            sprites: HashMap::new(),
            missing: HashSet::new(),
            requested: HashSet::new(),
            map_tiles: HashMap::new(),
            map_requested: false,
            map_done: false,
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

    /// Start the world-map sweep (nearest to `center_region` first), once.
    pub fn ensure_world_map(&mut self, center_region: (i32, i32)) {
        if !self.map_requested {
            self.map_requested = true;
            self.worker.request_world_map(center_region);
        }
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
                AssetEvent::Sprite(SpriteImage {
                    id,
                    width,
                    height,
                    rgba,
                }) => {
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [width as usize, height as usize],
                        &rgba,
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
                AssetEvent::WorldMapTile(tile) => {
                    let WorldMapTile { region, rgba } = *tile;
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [WORLD_MAP_IMAGE, WORLD_MAP_IMAGE],
                        &rgba,
                    );
                    let handle = ctx.load_texture(
                        format!("map-{}-{}", region.0, region.1),
                        image,
                        egui::TextureOptions::NEAREST,
                    );
                    self.map_tiles.insert(region, handle);
                }
                AssetEvent::WorldMapDone => self.map_done = true,
                AssetEvent::Failed(message) => self.status = Some(message),
            }
        }
        changed
    }
}
