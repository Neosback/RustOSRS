//! The minimap's scene image, built like the client's `method2150` (`SecureRandomCallable`) and
//! `Scene.drawTileMinimap`: every tile is 4x4 pixels, filled with its paint colour (or the
//! underlay/overlay colours through the tile-shape template), then walls and map scenes are drawn
//! over the tiles and map-icon objects are collected.
//!
//! The client builds one 104x104-tile sprite for the loaded scene. This module builds one image
//! per 64x64 map region (a 256x256 image) so the viewer can composite any window of the world.

use crate::{WorldDefinitions, WorldError, worldmap::Catalog};
use osrs_core::{color_palette::build_color_palette, coords::RegionCoord};
use osrs_scene::{
    terrain::TerrainSurface,
    terrain_build::{TerrainJitter, build_terrain},
    terrain_load::TerrainLoadGrid,
};

/// Pixels per tile.
pub const TILE_PX: usize = 4;
/// Region image edge in pixels.
pub const REGION_PX: usize = 64 * TILE_PX;
/// The client's default brightness for `Rasterizer3D_colorPalette`.
const BRIGHTNESS: f64 = 0.8;
/// `0xEEEEEE` (walls) and `0xEE0000` (interactive objects), the centres of the client's
/// randomised colours.
const WALL: u32 = 0xEE_EEEE;
const DOOR: u32 = 0xEE_0000;

/// `Scene.tileShape2D`: 16 template entries (a 4x4 pixel mask) per tile shape.
const TILE_SHAPE_2D: [[u8; 16]; 13] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1],
    [1, 1, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0],
    [0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0],
    [1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 1, 1],
    [1, 1, 1, 1, 1, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 1, 1, 1, 1],
];

/// `Scene.tileRotation2D`.
const TILE_ROTATION_2D: [[u8; 16]; 4] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [12, 8, 4, 0, 13, 9, 5, 1, 14, 10, 6, 2, 15, 11, 7, 3],
    [15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
    [3, 7, 11, 15, 2, 6, 10, 14, 1, 5, 9, 13, 0, 4, 8, 12],
];

/// A map-icon object found on the minimap (a floor decoration with a map icon).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimapIcon {
    pub element: u32,
    /// Tile within the region.
    pub x: u8,
    pub y: u8,
}

/// One region of the minimap.
#[derive(Debug, Clone)]
pub struct MinimapRegion {
    pub region: (i32, i32),
    pub plane: u8,
    /// `REGION_PX` squared RGBA, row 0 = the northernmost tile row; tiles without terrain are
    /// transparent.
    pub rgba: Vec<u8>,
    pub icons: Vec<MinimapIcon>,
}

struct Image {
    pixels: Vec<u32>,
}

impl Image {
    fn set(&mut self, x: usize, y: usize, colour: u32) {
        if x < REGION_PX && y < REGION_PX {
            self.pixels[y * REGION_PX + x] = 0xFF00_0000 | colour;
        }
    }
}

/// Render the minimap image of `region` at `plane`.
pub fn render_region(
    definitions: &mut WorldDefinitions,
    catalog: &Catalog,
    region: (i32, i32),
    plane: u8,
) -> Result<Option<MinimapRegion>, WorldError> {
    let coord = RegionCoord::new(region.0, region.1);
    let Some(own) = definitions.region(coord)? else {
        return Ok(None);
    };
    let margin = crate::REGION_WINDOW_MARGIN;
    let (base_x, base_y) = (region.0 * 64 - margin, region.1 * 64 - margin);
    let Some(window) = crate::SceneWindow::new(base_x, base_y) else {
        return Ok(None);
    };
    let mut grid = TerrainLoadGrid::reference();
    let mut missing = Vec::new();
    for window_region in window.regions() {
        let origin = window.region_scene_origin(window_region);
        match definitions.region(window_region)? {
            Some(map) => grid.load_region(&map.terrain, origin, (base_x, base_y), 0),
            None => missing.push(origin),
        }
    }
    for (scene_x, scene_y) in missing {
        grid.fill_missing_region(scene_x, scene_y, 64, 64);
    }
    let palette = build_color_palette(BRIGHTNESS);
    let output = build_terrain(
        &grid,
        definitions.floors(),
        TerrainJitter::default(),
        &palette,
    )
    .map_err(|error| WorldError::Placement(error.to_string()))?;

    let mut image = Image {
        pixels: vec![0; REGION_PX * REGION_PX],
    };
    let margin_tiles = margin as usize;
    // Bridge tiles of the plane above show on this plane (`tileSettings & 8`).
    let drawn_planes = [plane, plane + 1];
    for built in &output.tiles {
        if !drawn_planes.contains(&built.plane) {
            continue;
        }
        let (tx, ty) = (built.x as usize, built.y as usize);
        if tx < margin_tiles
            || ty < margin_tiles
            || tx >= margin_tiles + 64
            || ty >= margin_tiles + 64
        {
            continue;
        }
        let (lx, ly) = (tx - margin_tiles, ty - margin_tiles);
        let settings = own
            .terrain
            .tile(usize::from(built.plane), lx, ly)
            .settings
            .unwrap_or(0);
        let wanted = if built.plane == plane {
            settings & 24 == 0
        } else {
            settings & 8 != 0
        };
        if !wanted {
            continue;
        }
        let (px, py) = (lx * TILE_PX, (63 - ly) * TILE_PX);
        match &built.surface {
            TerrainSurface::Flat(flat) => {
                if flat.rgb != 0 {
                    for row in 0..TILE_PX {
                        for column in 0..TILE_PX {
                            image.set(px + column, py + row, flat.rgb as u32 & 0xFF_FFFF);
                        }
                    }
                }
            }
            TerrainSurface::Shaped(shaped) => {
                let shape = usize::from(shaped.shape).min(TILE_SHAPE_2D.len() - 1);
                let template = &TILE_SHAPE_2D[shape];
                let rotation = &TILE_ROTATION_2D[usize::from(shaped.rotation & 3)];
                let (underlay, overlay) = (
                    shaped.underlay_rgb as u32 & 0xFF_FFFF,
                    shaped.overlay_rgb as u32 & 0xFF_FFFF,
                );
                for row in 0..TILE_PX {
                    for column in 0..TILE_PX {
                        let on = template[usize::from(rotation[row * TILE_PX + column])] != 0;
                        if shaped.underlay_rgb != 0 {
                            image.set(px + column, py + row, if on { overlay } else { underlay });
                        } else if on {
                            image.set(px + column, py + row, overlay);
                        }
                    }
                }
            }
        }
    }

    // Walls and map scenes, plane by plane like the client's second pass.
    let mut icons = Vec::new();
    for location in own.locations.locations() {
        let location_plane = location.source_plane.index().get();
        if location_plane != plane && location_plane != plane + 1 {
            continue;
        }
        let (lx, ly) = (
            usize::from(location.tile.x()),
            usize::from(location.tile.y()),
        );
        let Ok(Some(definition)) = definitions.object(location.object_id) else {
            continue;
        };
        let (px, py) = (lx * TILE_PX, (63 - ly) * TILE_PX);
        let colour = if definition.map_interactive() {
            DOOR
        } else {
            WALL
        };
        let rotation = location.orientation & 3;
        let scene = if (4..=8).contains(&location.loc_type) {
            None
        } else {
            definition
                .map_scene
                .and_then(|id| catalog.map_scene(id.get()))
        };
        if let Some(scene) = scene {
            let (size_x, size_y) = (
                i32::from(definition.size_x) * TILE_PX as i32,
                i32::from(definition.size_y) * TILE_PX as i32,
            );
            let (frame_w, frame_h) = scene.frame_size();
            let x = lx as i32 * TILE_PX as i32 + (size_x - frame_w) / 2;
            let y = (64 - ly as i32 - i32::from(definition.size_y)) * TILE_PX as i32
                + (size_y - frame_h) / 2;
            scene.blit(x, y, |sx, sy, rgb| {
                if sx >= 0 && sy >= 0 {
                    image.set(sx as usize, sy as usize, rgb);
                }
            });
            continue;
        }
        match location.loc_type {
            0 | 2 => {
                // `var10 == 0 || var10 == 2`: the wall edge by rotation.
                edge(&mut image, px, py, rotation, colour);
                if location.loc_type == 2 {
                    // A corner wall also covers the adjoining edge.
                    edge(&mut image, px, py, (rotation + 1) & 3, colour);
                }
            }
            3 => {
                let (x, y) = match rotation {
                    0 => (px, py),
                    1 => (px + 3, py),
                    2 => (px + 3, py + 3),
                    _ => (px, py + 3),
                };
                image.set(x, y, colour);
            }
            9 => {
                let colour = if definition.map_interactive() {
                    0xEE_0000
                } else {
                    0xEE_EEEE
                };
                for step in 0..TILE_PX {
                    let y = if rotation == 0 || rotation == 2 {
                        py + 3 - step
                    } else {
                        py + step
                    };
                    image.set(px + step, y, colour);
                }
            }
            22 => {
                if let Some(icon) = definition.map_icon {
                    let minimap = catalog
                        .elements
                        .get(&icon.get())
                        .is_some_and(|element| element.minimap);
                    if minimap && location_plane == plane {
                        icons.push(MinimapIcon {
                            element: icon.get(),
                            x: lx as u8,
                            y: ly as u8,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    let mut rgba = vec![0_u8; REGION_PX * REGION_PX * 4];
    for (index, pixel) in image.pixels.iter().enumerate() {
        if *pixel != 0 {
            rgba[index * 4] = (pixel >> 16) as u8;
            rgba[index * 4 + 1] = (pixel >> 8) as u8;
            rgba[index * 4 + 2] = *pixel as u8;
            rgba[index * 4 + 3] = 255;
        }
    }
    Ok(Some(MinimapRegion {
        region,
        plane,
        rgba,
        icons,
    }))
}

/// One wall edge of a tile (`drawObject`): rotation 0 west column, 1 north row, 2 east column,
/// 3 south row.
fn edge(image: &mut Image, px: usize, py: usize, rotation: u8, colour: u32) {
    for step in 0..TILE_PX {
        let (x, y) = match rotation {
            0 => (px, py + step),
            1 => (px + step, py),
            2 => (px + 3, py + step),
            _ => (px + step, py + 3),
        };
        image.set(x, y, colour);
    }
}
