//! Background asset worker: UI sprites and world-map tiles read from the cache.
//!
//! One thread owns its own [`WorldDefinitions`] and answers requests, so the editor never touches
//! the cache from the render thread. Results come back as events to upload to textures.
//!
//! * **Sprites** (cache index 8): the compass, minimap frame, wiki icon and any other interface
//!   sprite, decoded to RGBA.
//! * **World map tiles**: one 64x64 image per map region, coloured from the underlay and overlay
//!   floors (underlays blended over a 11x11 window like the client's terrain, overlays mixed by the
//!   share of the tile their shape covers). This is the same data the in-game world map and
//!   minimap are drawn from.

use crate::{WorldDefinitions, WorldError};
use osrs_cache::decode::decode_sprite_group;
use osrs_core::coords::RegionCoord;
use osrs_scene::terrain_build::FloorLookup;
use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, Sender, channel},
    thread::JoinHandle,
};

const SPRITE_INDEX: u8 = 8;
/// Tiles per region edge.
pub const TILES: usize = 64;
/// World-map pixels per tile.
pub const PX: usize = 2;
/// World-map image edge in pixels.
pub const IMAGE: usize = TILES * PX;
/// The largest region coordinates swept for the world map.
const MAX_REGION_X: i32 = 90;
const MAX_REGION_Y: i32 = 200;

/// A decoded sprite (first frame, on its full canvas).
#[derive(Debug, Clone)]
pub struct SpriteImage {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    /// RGBA, row-major, straight alpha.
    pub rgba: Vec<u8>,
}

/// One world-map region image.
#[derive(Debug, Clone)]
pub struct WorldMapTile {
    pub region: (i32, i32),
    /// `TILES x TILES` RGBA, row 0 = the northernmost tile row.
    pub rgba: Vec<u8>,
}

pub enum AssetEvent {
    Sprite(SpriteImage),
    SpriteMissing(u32),
    WorldMapTile(Box<WorldMapTile>),
    /// The sweep requested by [`AssetWorker::request_world_map`] finished.
    WorldMapDone,
    Failed(String),
}

enum Request {
    Sprite(u32),
    WorldMap { center: (i32, i32) },
    Shutdown,
}

pub struct AssetWorker {
    requests: Sender<Request>,
    events: Receiver<AssetEvent>,
    worker: Option<JoinHandle<()>>,
}

impl AssetWorker {
    pub fn spawn(cache_dir: PathBuf) -> Self {
        let (request_tx, request_rx) = channel::<Request>();
        let (event_tx, event_rx) = channel::<AssetEvent>();
        let worker = std::thread::Builder::new()
            .name("assets".to_owned())
            .spawn(move || {
                let definitions = match WorldDefinitions::open(&cache_dir) {
                    Ok(definitions) => definitions,
                    Err(error) => {
                        let _ = event_tx.send(AssetEvent::Failed(error.to_string()));
                        return;
                    }
                };
                while let Ok(request) = request_rx.recv() {
                    match request {
                        Request::Shutdown => break,
                        Request::Sprite(id) => {
                            let event = match load_sprite(&definitions, id) {
                                Ok(Some(sprite)) => AssetEvent::Sprite(sprite),
                                Ok(None) => AssetEvent::SpriteMissing(id),
                                Err(error) => AssetEvent::Failed(format!("sprite {id}: {error}")),
                            };
                            if event_tx.send(event).is_err() {
                                break;
                            }
                        }
                        Request::WorldMap { center } => {
                            if !sweep_world_map(&definitions, center, &event_tx, &request_rx) {
                                break;
                            }
                            let _ = event_tx.send(AssetEvent::WorldMapDone);
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

    pub fn request_sprite(&self, id: u32) {
        let _ = self.requests.send(Request::Sprite(id));
    }

    /// Render every map region, nearest to `center` (a region) first.
    pub fn request_world_map(&self, center: (i32, i32)) {
        let _ = self.requests.send(Request::WorldMap { center });
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

/// Returns `false` when the worker should shut down.
fn sweep_world_map(
    definitions: &WorldDefinitions,
    center: (i32, i32),
    events: &Sender<AssetEvent>,
    requests: &Receiver<Request>,
) -> bool {
    let mut cache = RegionCache::new();
    let mut regions: Vec<(i32, i32)> = (0..=MAX_REGION_X)
        .flat_map(|x| (0..=MAX_REGION_Y).map(move |y| (x, y)))
        .collect();
    regions.sort_by_key(|&(x, y)| (x - center.0).pow(2) + (y - center.1).pow(2));
    for (index, (x, y)) in regions.into_iter().enumerate() {
        // Stay responsive to shutdown and to sprite requests while sweeping.
        if index % 32 == 0 {
            while let Ok(request) = requests.try_recv() {
                match request {
                    Request::Shutdown => return false,
                    Request::Sprite(id) => {
                        let event = match load_sprite(definitions, id) {
                            Ok(Some(sprite)) => AssetEvent::Sprite(sprite),
                            Ok(None) => AssetEvent::SpriteMissing(id),
                            Err(error) => AssetEvent::Failed(format!("sprite {id}: {error}")),
                        };
                        let _ = events.send(event);
                    }
                    Request::WorldMap { .. } => {}
                }
            }
        }
        match render_region(definitions, &mut cache, RegionCoord::new(x, y)) {
            Ok(Some(rgba)) => {
                let tile = WorldMapTile {
                    region: (x, y),
                    rgba,
                };
                if events
                    .send(AssetEvent::WorldMapTile(Box::new(tile)))
                    .is_err()
                {
                    return false;
                }
            }
            Ok(None) => {}
            Err(error) => {
                let _ = events.send(AssetEvent::Failed(format!("region {x},{y}: {error}")));
            }
        }
    }
    true
}

/// Render the world-map image of one region (diagnostics and tests).
pub fn render_region_image(
    definitions: &WorldDefinitions,
    region: RegionCoord,
) -> Result<Option<Vec<u8>>, WorldError> {
    render_region(definitions, &mut RegionCache::new(), region)
}

/// One region of the world map rendered like the game terrain.
///
/// The region is built inside its own scene window with the real terrain builder (so underlay
/// blending across region borders, slope lighting, overlay shapes and textures match the 3D view),
/// then every tile is rasterised top-down with per-corner colours.
fn render_region(
    definitions: &WorldDefinitions,
    cache: &mut RegionCache,
    region: RegionCoord,
) -> Result<Option<Vec<u8>>, WorldError> {
    use osrs_core::color_palette::build_color_palette;
    use osrs_scene::{
        terrain::{TerrainColorSource, TerrainSurface},
        terrain_build::{TerrainJitter, build_terrain},
        terrain_load::TerrainLoadGrid,
    };

    if cache.get(definitions, (region.x, region.y))?.is_none() {
        return Ok(None);
    }
    let margin = crate::REGION_WINDOW_MARGIN;
    let (base_x, base_y) = (region.x * 64 - margin, region.y * 64 - margin);
    let Some(window) = crate::SceneWindow::new(base_x, base_y) else {
        return Ok(None);
    };
    let mut grid = TerrainLoadGrid::reference();
    let mut missing = Vec::new();
    for window_region in window.regions() {
        let origin = window.region_scene_origin(window_region);
        match cache.get(definitions, (window_region.x, window_region.y))? {
            Some(map) => grid.load_region(&map.terrain, origin, (base_x, base_y), 0),
            None => missing.push(origin),
        }
    }
    for (scene_x, scene_y) in missing {
        grid.fill_missing_region(scene_x, scene_y, 64, 64);
    }
    let palette = build_color_palette(0.8);
    let floors = definitions.floors();
    let output = build_terrain(&grid, floors, TerrainJitter::default(), &palette)
        .map_err(|error| WorldError::Placement(error.to_string()))?;

    let mut image = vec![0_u8; IMAGE * IMAGE * 4];
    // The game's minimap colours are flat (no slope lighting), which keeps the map smooth; a
    // little of the lit corner colour is mixed back in so hills still read.
    const LIT: f32 = 0.2;
    let lit = |hsl: i32| -> [f32; 3] {
        let rgb = usize::try_from(hsl)
            .ok()
            .and_then(|index| palette.get(index))
            .copied()
            .unwrap_or(0);
        unpack(rgb)
    };
    let mix = |flat: [f32; 3], lit: [f32; 3]| -> [f32; 3] {
        [
            flat[0] * (1.0 - LIT) + lit[0] * LIT,
            flat[1] * (1.0 - LIT) + lit[1] * LIT,
            flat[2] * (1.0 - LIT) + lit[2] * LIT,
        ]
    };
    // A textured floor's flat colour: the texture average, else the overlay's own colour.
    let textured = |texture: i32, tile_x: i32, tile_y: i32| -> [f32; 3] {
        let average = floors.texture_average_rgb(texture);
        if average != 0 {
            return unpack(average);
        }
        let id = u32::from(grid.overlay_raw(0, tile_x as usize, tile_y as usize) & 0x7fff);
        let fallback = if id > 0 {
            floors.overlay(id - 1).primary_rgb
        } else {
            0
        };
        unpack(if fallback > 0 { fallback } else { 0x8A_7A_5A })
    };
    let scale = PX as f32 / 128.0;
    let image_x = |unit_x: i32| (unit_x - margin * 128) as f32 * scale;
    let image_y = |unit_z: i32| (64 * 128 - (unit_z - margin * 128)) as f32 * scale;
    for tile in output.tiles.iter().filter(|tile| tile.plane == 0) {
        let (tx, ty) = (tile.x as i32, tile.y as i32);
        if !(margin..margin + 64).contains(&tx) || !(margin..margin + 64).contains(&ty) {
            continue;
        }
        match &tile.surface {
            TerrainSurface::Flat(flat) => {
                if flat.colors.northeast == SKIPPED {
                    continue;
                }
                let flat_rgb = match flat.texture_id {
                    Some(texture) => textured(texture, tx, ty),
                    None => unpack(flat.rgb),
                };
                let corner = |h: i32| mix(flat_rgb, lit(h));
                let (sw, se, ne, nw) = (
                    corner(flat.colors.southwest),
                    corner(flat.colors.southeast),
                    corner(flat.colors.northeast),
                    corner(flat.colors.northwest),
                );
                let (x0, x1) = (tx * 128, tx * 128 + 128);
                let (z0, z1) = (ty * 128, ty * 128 + 128);
                for (a, b, c) in [
                    ((x1, z1, ne), (x0, z1, nw), (x1, z0, se)),
                    ((x0, z0, sw), (x1, z0, se), (x0, z1, nw)),
                ] {
                    raster(
                        &mut image,
                        [
                            (image_x(a.0), image_y(a.1), a.2),
                            (image_x(b.0), image_y(b.1), b.2),
                            (image_x(c.0), image_y(c.1), c.2),
                        ],
                    );
                }
            }
            TerrainSurface::Shaped(shaped) => {
                for face in &shaped.faces {
                    if face.colors[0] == SKIPPED {
                        continue;
                    }
                    let flat_rgb = match (face.texture_id, face.color_source) {
                        (Some(texture), _) => textured(texture, tx, ty),
                        (None, TerrainColorSource::Underlay) => unpack(shaped.underlay_rgb),
                        (None, TerrainColorSource::Overlay) => unpack(shaped.overlay_rgb),
                    };
                    let mut vertices = [(0.0, 0.0, [0.0; 3]); 3];
                    for (slot, &index) in face.indices.iter().enumerate() {
                        let vertex = &shaped.vertices[index];
                        vertices[slot] = (
                            image_x(vertex.position.x.units()),
                            image_y(vertex.position.z.units()),
                            mix(flat_rgb, lit(face.colors[slot])),
                        );
                    }
                    raster(&mut image, vertices);
                }
            }
        }
    }

    // Walls outline buildings like the game's map: dark edge pixels along wall tiles.
    if let Some(map) = cache.get(definitions, (region.x, region.y))? {
        for location in map.locations.locations() {
            if location.source_plane.index().get() != 0 {
                continue;
            }
            let (lx, ly) = (
                usize::from(location.tile.x()),
                usize::from(location.tile.y()),
            );
            let edges: &[u8] = match (location.loc_type, location.orientation & 3) {
                (0, o) => &[[0u8, 1, 2, 3][usize::from(o)]],
                (2, 0) => &[0, 1],
                (2, 1) => &[1, 2],
                (2, 2) => &[2, 3],
                (2, 3) => &[3, 0],
                _ => &[],
            };
            for &edge in edges {
                wall_edge(&mut image, lx, ly, edge);
            }
        }
    }
    Ok(Some(image))
}

const SKIPPED: i32 = 12_345_678;
const WALL: [u8; 3] = [48, 38, 30];

/// Dark pixels along one side of a tile (0 west, 1 north, 2 east, 3 south).
fn wall_edge(image: &mut [u8], tile_x: usize, tile_y: usize, edge: u8) {
    let base_x = tile_x * PX;
    let base_row = (TILES - 1 - tile_y) * PX;
    for step in 0..PX {
        let (x, row) = match edge {
            0 => (base_x, base_row + step),
            2 => (base_x + PX - 1, base_row + step),
            1 => (base_x + step, base_row),
            _ => (base_x + step, base_row + PX - 1),
        };
        let pixel = (row * IMAGE + x) * 4;
        image[pixel..pixel + 3].copy_from_slice(&WALL);
        image[pixel + 3] = 255;
    }
}

/// Gouraud-fill a triangle (pixel-space vertices with RGB colours) into the image.
fn raster(image: &mut [u8], v: [(f32, f32, [f32; 3]); 3]) {
    let min_x = v
        .iter()
        .map(|p| p.0)
        .fold(f32::MAX, f32::min)
        .floor()
        .max(0.0) as usize;
    let max_x = v
        .iter()
        .map(|p| p.0)
        .fold(f32::MIN, f32::max)
        .ceil()
        .min(IMAGE as f32) as usize;
    let min_y = v
        .iter()
        .map(|p| p.1)
        .fold(f32::MAX, f32::min)
        .floor()
        .max(0.0) as usize;
    let max_y = v
        .iter()
        .map(|p| p.1)
        .fold(f32::MIN, f32::max)
        .ceil()
        .min(IMAGE as f32) as usize;
    let area = (v[1].0 - v[0].0) * (v[2].1 - v[0].1) - (v[2].0 - v[0].0) * (v[1].1 - v[0].1);
    if area.abs() < 1e-6 {
        return;
    }
    for py in min_y..max_y {
        for px in min_x..max_x {
            let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
            let w0 = ((v[1].0 - x) * (v[2].1 - y) - (v[2].0 - x) * (v[1].1 - y)) / area;
            let w1 = ((v[2].0 - x) * (v[0].1 - y) - (v[0].0 - x) * (v[2].1 - y)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                continue;
            }
            let pixel = (py * IMAGE + px) * 4;
            for c in 0..3 {
                let value = w0 * v[0].2[c] + w1 * v[1].2[c] + w2 * v[2].2[c];
                image[pixel + c] = (value.clamp(0.0, 1.0) * 255.0) as u8;
            }
            image[pixel + 3] = 255;
        }
    }
}

fn unpack(rgb: i32) -> [f32; 3] {
    [
        ((rgb >> 16) & 255) as f32 / 255.0,
        ((rgb >> 8) & 255) as f32 / 255.0,
        (rgb & 255) as f32 / 255.0,
    ]
}

/// Decoded regions kept while sweeping (neighbouring windows share regions).
struct RegionCache {
    maps: std::collections::HashMap<
        (i32, i32),
        Option<std::sync::Arc<crate::definitions::RegionMap>>,
    >,
}

impl RegionCache {
    fn new() -> Self {
        Self {
            maps: std::collections::HashMap::new(),
        }
    }

    fn get(
        &mut self,
        definitions: &WorldDefinitions,
        key: (i32, i32),
    ) -> Result<Option<std::sync::Arc<crate::definitions::RegionMap>>, WorldError> {
        if self.maps.len() > 64 {
            self.maps.clear();
        }
        if let Some(cached) = self.maps.get(&key) {
            return Ok(cached.clone());
        }
        let map = definitions
            .region(RegionCoord::new(key.0, key.1))?
            .map(std::sync::Arc::new);
        self.maps.insert(key, map.clone());
        Ok(map)
    }
}
