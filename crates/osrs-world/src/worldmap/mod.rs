//! The in-game world map, ported from the client (`WorldMap`, `WorldMapRenderer`,
//! `WorldMapRegion`, `WorldMapScaleHandler`, `WorldMapArea*`, `WorldMapElement`).
//!
//! Cache layout (build 241):
//!
//! * index 19 group 0 `details`: one file per map area (name, origin, background, zoom, the
//!   [`Section`]s that map world coordinates onto the area);
//! * index 19 group 1 `compositemap`: per area, the geography references of every region/chunk
//!   plus the area's fixed icon list;
//! * index 19 group 2 `compositetexture`: per area, the overview picture (PNG);
//! * index 18 `geography`: per region, underlay/overlay ids with shape + rotation and the
//!   decorations (walls, scenery) used for wall lines, map scenes and map icons;
//! * index 20 `ground`: per region, a 64x64 PNG of the pre-blended ground colour of every tile;
//! * config 2 group 35: `WorldMapElement` (icon sprite, name, label colour/size, alignment).
//!
//! A region image is `64 * ppt` pixels square, rendered once per pixels-per-tile value exactly as
//! `WorldMapRegion.drawTile` does, and scaled by the viewer.

mod templates;

use crate::{WorldDefinitions, WorldError};
use osrs_cache::decode::decode_sprite_group;
use osrs_core::{
    color_palette::build_color_palette,
    floor_color::pack_terrain_hsl,
    ids::ObjectId,
};
use osrs_scene::terrain_build::FloorLookup;
use std::collections::HashMap;

const DETAILS_INDEX: u8 = 19;
const DETAILS_GROUP: u32 = 0;
const COMPOSITE_MAP_GROUP: u32 = 1;
const COMPOSITE_TEXTURE_GROUP: u32 = 2;
const GEOGRAPHY_INDEX: u8 = 18;
const GROUND_INDEX: u8 = 20;
const CONFIG_INDEX: u8 = 2;
const ELEMENT_GROUP: u32 = 35;
const SPRITE_INDEX: u8 = 8;
/// `Jagex name hash("mapscene")`: the sprite group holding the map-scene (tree, rock ...) tiles.
const MAPSCENE_NAME_HASH: i32 = 197_029_040;
/// The brightness the world map builds its colour palette with.
pub const PALETTE_BRIGHTNESS: f64 = 0.725;
/// Lightness the overlay colours are rewritten with (`method5241(.., 96)`).
const OVERLAY_LIGHT: i32 = 96;

/// Map element used for the (hidden) lookup `iconMap`.
pub const MAP_SIZE_TILES: i32 = 64;

// ---------------------------------------------------------------------------------------------
// byte reader
// ---------------------------------------------------------------------------------------------

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    fn u8(&mut self) -> Option<i32> {
        let value = *self.bytes.get(self.at)?;
        self.at += 1;
        Some(i32::from(value))
    }

    fn u16(&mut self) -> Option<i32> {
        Some((self.u8()? << 8) | self.u8()?)
    }

    fn i16(&mut self) -> Option<i32> {
        Some(i32::from(self.u16()? as u16 as i16))
    }

    fn medium(&mut self) -> Option<i32> {
        Some((self.u8()? << 16) | (self.u8()? << 8) | self.u8()?)
    }

    fn i32(&mut self) -> Option<i32> {
        Some((self.u8()? << 24) | (self.u8()? << 16) | (self.u8()? << 8) | self.u8()?)
    }

    fn string(&mut self) -> Option<String> {
        let start = self.at;
        while *self.bytes.get(self.at)? != 0 {
            self.at += 1;
        }
        let text = self.bytes[start..self.at].iter().map(|&b| char::from(b)).collect();
        self.at += 1;
        Some(text)
    }

    /// `Buffer.readNullableLargeSmart`.
    fn nullable_large_smart(&mut self) -> Option<i32> {
        if *self.bytes.get(self.at)? & 0x80 != 0 {
            Some(self.i32()? & 0x7fff_ffff)
        } else {
            let value = self.u16()?;
            Some(if value == 32_767 { -1 } else { value })
        }
    }
}

// ---------------------------------------------------------------------------------------------
// areas
// ---------------------------------------------------------------------------------------------

/// A packed client `Coord`: plane, tile x, tile y.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Coord {
    pub plane: i32,
    pub x: i32,
    pub y: i32,
}

impl Coord {
    fn unpack(value: i32) -> Option<Self> {
        (value != -1).then_some(Self {
            plane: (value >> 28) & 3,
            x: (value >> 14) & 16_383,
            y: value & 16_383,
        })
    }
}

/// One rule mapping world coordinates onto the area's map coordinates
/// (`WorldMapSection0..2` and the chunk variant).
#[derive(Debug, Clone, Copy)]
pub enum Section {
    /// Section type 0: a rectangle of whole regions.
    Rect {
        min_plane: i32,
        planes: i32,
        world: [i32; 4],
        map: [i32; 4],
    },
    /// Section type 1: one region.
    Region {
        min_plane: i32,
        planes: i32,
        world: (i32, i32),
        map: (i32, i32),
    },
    /// Section type 2: a block of 8x8 chunks inside one region.
    Chunks {
        world_plane: i32,
        planes: i32,
        world_region: (i32, i32),
        world_chunks: [i32; 4],
        map_region: (i32, i32),
        map_chunks: [i32; 4],
    },
    /// Section type 3: one chunk.
    Chunk {
        world_plane: i32,
        planes: i32,
        world_region: (i32, i32),
        world_chunk: (i32, i32),
        map_region: (i32, i32),
        map_chunk: (i32, i32),
    },
}

impl Section {
    fn read(reader: &mut Reader<'_>) -> Option<Self> {
        Some(match reader.u8()? {
            0 => {
                let min_plane = reader.u8()?;
                let planes = reader.u8()?;
                let world = [reader.u16()?, reader.u16()?, reader.u16()?, reader.u16()?];
                // The file stores the map rectangle as x-low, y-low, x-high, y-high.
                let map = [reader.u16()?, reader.u16()?, reader.u16()?, reader.u16()?];
                Self::Rect {
                    min_plane,
                    planes,
                    world,
                    map,
                }
            }
            1 => Self::Region {
                min_plane: reader.u8()?,
                planes: reader.u8()?,
                world: (reader.u16()?, reader.u16()?),
                map: (reader.u16()?, reader.u16()?),
            },
            2 => {
                let world_plane = reader.u8()?;
                let planes = reader.u8()?;
                let old_x = reader.u16()?;
                let (old_cx0, old_cx1) = (reader.u8()?, reader.u8()?);
                let old_y = reader.u16()?;
                let (old_cy0, old_cy1) = (reader.u8()?, reader.u8()?);
                let new_x = reader.u16()?;
                let (new_cx0, new_cx1) = (reader.u8()?, reader.u8()?);
                let new_y = reader.u16()?;
                let (new_cy0, new_cy1) = (reader.u8()?, reader.u8()?);
                Self::Chunks {
                    world_plane,
                    planes,
                    world_region: (old_x, old_y),
                    world_chunks: [old_cx0, old_cy0, old_cx1, old_cy1],
                    map_region: (new_x, new_y),
                    map_chunks: [new_cx0, new_cy0, new_cx1, new_cy1],
                }
            }
            3 => {
                let world_plane = reader.u8()?;
                let planes = reader.u8()?;
                let world_x = reader.u16()?;
                let world_cx = reader.u8()?;
                let world_y = reader.u16()?;
                let world_cy = reader.u8()?;
                let map_x = reader.u16()?;
                let map_cx = reader.u8()?;
                let map_y = reader.u16()?;
                let map_cy = reader.u8()?;
                Self::Chunk {
                    world_plane,
                    planes,
                    world_region: (world_x, world_y),
                    world_chunk: (world_cx, world_cy),
                    map_region: (map_x, map_y),
                    map_chunk: (map_cx, map_cy),
                }
            }
            _ => return None,
        })
    }

    /// `expandBounds`: the region bounds of the map side, as `[low_x, high_x, low_y, high_y]`.
    fn map_bounds(&self) -> [i32; 4] {
        match *self {
            Self::Rect { map, .. } => [map[0], map[2], map[1], map[3]],
            Self::Region { map, .. } => [map.0, map.0, map.1, map.1],
            Self::Chunks { map_region, .. } | Self::Chunk { map_region, .. } => {
                [map_region.0, map_region.0, map_region.1, map_region.1]
            }
        }
    }

    /// `containsCoord`: does the section cover this world coordinate?
    pub fn contains_world(&self, plane: i32, x: i32, y: i32) -> bool {
        match *self {
            Self::Rect {
                min_plane,
                planes,
                world,
                ..
            } => {
                plane >= min_plane
                    && plane < min_plane + planes
                    && (x >> 6) >= world[0]
                    && (x >> 6) <= world[2]
                    && (y >> 6) >= world[1]
                    && (y >> 6) <= world[3]
            }
            Self::Region {
                min_plane,
                planes,
                world,
                ..
            } => {
                plane >= min_plane
                    && plane < min_plane + planes
                    && x >> 6 == world.0
                    && y >> 6 == world.1
            }
            Self::Chunks {
                world_plane,
                planes,
                world_region,
                world_chunks,
                ..
            } => {
                plane >= world_plane
                    && plane < world_plane + planes
                    && x >= (world_region.0 << 6) + (world_chunks[0] << 3)
                    && x <= (world_region.0 << 6) + (world_chunks[2] << 3) + 7
                    && y >= (world_region.1 << 6) + (world_chunks[1] << 3)
                    && y <= (world_region.1 << 6) + (world_chunks[3] << 3) + 7
            }
            Self::Chunk {
                world_plane,
                planes,
                world_region,
                world_chunk,
                ..
            } => {
                plane >= world_plane
                    && plane < world_plane + planes
                    && x >= (world_region.0 << 6) + (world_chunk.0 << 3)
                    && x <= (world_region.0 << 6) + (world_chunk.0 << 3) + 7
                    && y >= (world_region.1 << 6) + (world_chunk.1 << 3)
                    && y <= (world_region.1 << 6) + (world_chunk.1 << 3) + 7
            }
        }
    }

    /// `containsPosition`: does the section cover this map tile?
    pub fn contains_map(&self, x: i32, y: i32) -> bool {
        match *self {
            Self::Rect { map, .. } => {
                (x >> 6) >= map[0] && (x >> 6) <= map[2] && (y >> 6) >= map[1] && (y >> 6) <= map[3]
            }
            Self::Region { map, .. } => x >> 6 == map.0 && y >> 6 == map.1,
            Self::Chunks {
                map_region,
                map_chunks,
                ..
            } => {
                x >= (map_region.0 << 6) + (map_chunks[0] << 3)
                    && x <= (map_region.0 << 6) + (map_chunks[2] << 3) + 7
                    && y >= (map_region.1 << 6) + (map_chunks[1] << 3)
                    && y <= (map_region.1 << 6) + (map_chunks[3] << 3) + 7
            }
            Self::Chunk {
                map_region,
                map_chunk,
                ..
            } => {
                x >= (map_region.0 << 6) + (map_chunk.0 << 3)
                    && x <= (map_region.0 << 6) + (map_chunk.0 << 3) + 7
                    && y >= (map_region.1 << 6) + (map_chunk.1 << 3)
                    && y <= (map_region.1 << 6) + (map_chunk.1 << 3) + 7
            }
        }
    }

    /// `getBorderTileLengths`: the map tile of a world coordinate.
    pub fn to_map(&self, plane: i32, x: i32, y: i32) -> Option<(i32, i32)> {
        if !self.contains_world(plane, x, y) {
            return None;
        }
        Some(match *self {
            Self::Rect { world, map, .. } => (map[0] * 64 - world[0] * 64 + x, y + (map[1] * 64 - world[1] * 64)),
            Self::Region { world, map, .. } => (map.0 * 64 - world.0 * 64 + x, y + (map.1 * 64 - world.1 * 64)),
            Self::Chunks {
                world_region,
                world_chunks,
                map_region,
                map_chunks,
                ..
            } => (
                map_region.0 * 64 - world_region.0 * 64 + x + (map_chunks[0] * 8 - world_chunks[0] * 8),
                y + (map_region.1 * 64 - world_region.1 * 64) + (map_chunks[1] * 8 - world_chunks[1] * 8),
            ),
            Self::Chunk {
                world_region,
                world_chunk,
                map_region,
                map_chunk,
                ..
            } => (
                map_region.0 * 64 - world_region.0 * 64 + x + (map_chunk.0 * 8 - world_chunk.0 * 8),
                y + (map_region.1 * 64 - world_region.1 * 64) + (map_chunk.1 * 8 - world_chunk.1 * 8),
            ),
        })
    }

    /// `coord`: the world coordinate behind a map tile.
    pub fn to_world(&self, x: i32, y: i32) -> Option<Coord> {
        if !self.contains_map(x, y) {
            return None;
        }
        Some(match *self {
            Self::Rect {
                min_plane,
                world,
                map,
                ..
            } => Coord {
                plane: min_plane,
                x: world[0] * 64 - map[0] * 64 + x,
                y: world[1] * 64 - map[1] * 64 + y,
            },
            Self::Region {
                min_plane,
                world,
                map,
                ..
            } => Coord {
                plane: min_plane,
                x: world.0 * 64 - map.0 * 64 + x,
                y: world.1 * 64 - map.1 * 64 + y,
            },
            Self::Chunks {
                world_plane,
                world_region,
                world_chunks,
                map_region,
                map_chunks,
                ..
            } => Coord {
                plane: world_plane,
                x: world_region.0 * 64 - map_region.0 * 64 + (world_chunks[0] * 8 - map_chunks[0] * 8) + x,
                y: world_region.1 * 64 - map_region.1 * 64 + y + (world_chunks[1] * 8 - map_chunks[1] * 8),
            },
            Self::Chunk {
                world_plane,
                world_region,
                world_chunk,
                map_region,
                map_chunk,
                ..
            } => Coord {
                plane: world_plane,
                x: world_region.0 * 64 - map_region.0 * 64 + (world_chunk.0 * 8 - map_chunk.0 * 8) + x,
                y: world_region.1 * 64 - map_region.1 * 64 + y + (world_chunk.1 * 8 - map_chunk.1 * 8),
            },
        })
    }
}

/// Geography of a whole 64x64 region (`WorldMapData_0`).
#[derive(Debug, Clone)]
struct RegionData {
    region: (i32, i32),
}

/// Geography of one 8x8 chunk (`WorldMapData_1`).
#[derive(Debug, Clone)]
struct ChunkData {
    region: (i32, i32),
    chunk: (i32, i32),
}

/// A fixed icon of an area's `compositemap` (cities, dungeon entrances, links).
#[derive(Debug, Clone, Copy)]
pub struct LinkIcon {
    pub element: i32,
    /// Map tile (area coordinates).
    pub x: i32,
    pub y: i32,
    /// Only shown on members worlds.
    pub members: bool,
}

/// One map area (`WorldMapArea` + `WorldMapAreaData`).
#[derive(Debug, Clone)]
pub struct Area {
    pub id: u32,
    pub internal_name: String,
    pub external_name: String,
    pub origin: Coord,
    /// Background colour, ARGB (`field3364 | 0xFF000000`).
    pub background: u32,
    /// Default zoom as a percentage (25, 37, 50, 75, 100, 200).
    pub zoom_percent: i32,
    pub is_main: bool,
    pub sections: Vec<Section>,
    /// Region bounds of the map side (inclusive).
    pub low_x: i32,
    pub high_x: i32,
    pub low_y: i32,
    pub high_y: i32,
    regions: Vec<RegionData>,
    chunks: Vec<ChunkData>,
    pub links: Vec<LinkIcon>,
}

impl Area {
    /// `containsCoord`: does the area show this world coordinate?
    pub fn contains_world(&self, plane: i32, x: i32, y: i32) -> bool {
        self.sections.iter().any(|section| section.contains_world(plane, x, y))
    }

    /// `position`: the map tile (area coordinates) of a world coordinate.
    pub fn position(&self, plane: i32, x: i32, y: i32) -> Option<(i32, i32)> {
        self.sections.iter().find_map(|section| section.to_map(plane, x, y))
    }

    /// `coord`: the world coordinate shown at a map tile.
    pub fn world_coord(&self, x: i32, y: i32) -> Option<Coord> {
        if x >> 6 < self.low_x || x >> 6 > self.high_x || y >> 6 < self.low_y || y >> 6 > self.high_y {
            return None;
        }
        self.sections.iter().find_map(|section| section.to_world(x, y))
    }

    /// The map tile the area opens on (its origin).
    pub fn origin_position(&self) -> (i32, i32) {
        self.position(self.origin.plane, self.origin.x, self.origin.y)
            .unwrap_or((self.low_x * 64 + 32, self.low_y * 64 + 32))
    }

    /// Does any geography exist for this map region?
    pub fn has_region(&self, region_x: i32, region_y: i32) -> bool {
        self.regions.iter().any(|data| data.region == (region_x, region_y))
            || self.chunks.iter().any(|data| data.region == (region_x, region_y))
    }

    /// Every map region that has geography.
    pub fn region_keys(&self) -> Vec<(i32, i32)> {
        let mut keys: Vec<(i32, i32)> = self
            .regions
            .iter()
            .map(|data| data.region)
            .chain(self.chunks.iter().map(|data| data.region))
            .collect();
        keys.sort_unstable();
        keys.dedup();
        keys
    }
}

fn read_area(details: &[u8], composite: &[u8], id: u32) -> Option<Area> {
    let mut reader = Reader::new(details);
    let internal_name = reader.string()?;
    let external_name = reader.string()?;
    let origin = Coord::unpack(reader.i32()?).unwrap_or_default();
    let background = reader.i32()?;
    let _field3369 = reader.i32()?;
    let _ = reader.u8()?;
    let is_main = reader.u8()? == 1;
    let zoom_percent = reader.u8()?;
    let section_count = reader.u8()?;
    let mut sections = Vec::new();
    for _ in 0..section_count {
        sections.push(Section::read(&mut reader)?);
    }
    let mut area = Area {
        id,
        internal_name,
        external_name,
        origin,
        background: (background as u32) | 0xFF00_0000,
        zoom_percent,
        is_main,
        sections,
        low_x: i32::MAX,
        high_x: 0,
        low_y: i32::MAX,
        high_y: 0,
        regions: Vec::new(),
        chunks: Vec::new(),
        links: Vec::new(),
    };
    for section in &area.sections {
        let [low_x, high_x, low_y, high_y] = section.map_bounds();
        area.low_x = area.low_x.min(low_x);
        area.high_x = area.high_x.max(high_x);
        area.low_y = area.low_y.min(low_y);
        area.high_y = area.high_y.max(high_y);
    }

    let mut reader = Reader::new(composite);
    let region_count = reader.u16()?;
    for _ in 0..region_count {
        // A region whose header does not validate is skipped, like the client's catch block.
        let magic = reader.u8()?;
        let _min_plane = reader.u8()?;
        let _planes = reader.u8()?;
        let _region_x_low = reader.u16()?;
        let _region_y_low = reader.u16()?;
        let region_x = reader.u16()?;
        let region_y = reader.u16()?;
        if magic == 0 {
            area.regions.push(RegionData {
                region: (region_x, region_y),
            });
        }
    }
    let chunk_count = reader.u16()?;
    for _ in 0..chunk_count {
        let magic = reader.u8()?;
        let _min_plane = reader.u8()?;
        let _planes = reader.u8()?;
        let _region_x_low = reader.u16()?;
        let _region_y_low = reader.u16()?;
        let _chunk_x_low = reader.u8()?;
        let _chunk_y_low = reader.u8()?;
        let region_x = reader.u16()?;
        let region_y = reader.u16()?;
        let chunk_x = reader.u8()?;
        let chunk_y = reader.u8()?;
        if magic == 1 {
            area.chunks.push(ChunkData {
                region: (region_x, region_y),
                chunk: (chunk_x, chunk_y),
            });
        }
    }
    let link_count = reader.u16()?;
    for _ in 0..link_count {
        let element = reader.nullable_large_smart()?;
        let coord = Coord::unpack(reader.i32()?).unwrap_or_default();
        let members = reader.u8()? == 1;
        area.links.push(LinkIcon {
            element,
            x: coord.x,
            y: coord.y,
            members,
        });
    }
    Some(area)
}

// ---------------------------------------------------------------------------------------------
// elements
// ---------------------------------------------------------------------------------------------

/// A `WorldMapElement`: the icon and label of a kind of map feature.
#[derive(Debug, Clone)]
pub struct Element {
    pub id: u32,
    /// Sprite id (cache index 8), or `-1`.
    pub sprite: i32,
    pub name: Option<String>,
    /// Label colour (RGB).
    pub color: u32,
    /// 0 small (verdana 11), 1 medium (13), 2 large (15).
    pub text_size: u8,
    /// Shown unless the player hides its category.
    pub visible: bool,
    pub category: i32,
    /// Horizontal anchor: 0 centred, 1 left edge at the point, 2 right edge at the point.
    pub horizontal: u8,
    /// Vertical anchor: 0 centred, 2 top edge at the point, 1 bottom edge at the point.
    pub vertical: u8,
}

impl Element {
    fn read(id: u32, bytes: &[u8]) -> Self {
        let mut element = Self {
            id,
            sprite: -1,
            name: None,
            color: 0,
            text_size: 0,
            visible: true,
            category: -1,
            horizontal: 0,
            vertical: 0,
        };
        let mut reader = Reader::new(bytes);
        while let Some(opcode) = reader.u8() {
            let ok = (|| -> Option<()> {
                match opcode {
                    1 => element.sprite = reader.nullable_large_smart()?,
                    2 => {
                        reader.nullable_large_smart()?;
                    }
                    3 => element.name = Some(reader.string()?),
                    4 => element.color = reader.medium()? as u32,
                    5 => {
                        reader.medium()?;
                    }
                    6 => element.text_size = reader.u8()? as u8,
                    7 => {
                        let flags = reader.u8()?;
                        if flags & 1 == 0 {
                            element.visible = false;
                        }
                    }
                    8 => {
                        reader.u8()?;
                    }
                    10..=14 => {
                        reader.string()?;
                    }
                    15 => {
                        let count = reader.u8()?;
                        for _ in 0..count * 2 {
                            reader.i16()?;
                        }
                        reader.i32()?;
                        let colours = reader.u8()?;
                        for _ in 0..colours {
                            reader.i32()?;
                        }
                        for _ in 0..count {
                            reader.u8()?;
                        }
                    }
                    16 => {}
                    17 => {
                        reader.string()?;
                    }
                    18 => {
                        reader.nullable_large_smart()?;
                    }
                    19 => element.category = reader.u16()?,
                    21 | 22 => {
                        reader.i32()?;
                    }
                    23 => {
                        reader.u8()?;
                        reader.u8()?;
                        reader.u8()?;
                    }
                    24 => {
                        reader.i16()?;
                        reader.i16()?;
                    }
                    25 => {
                        reader.nullable_large_smart()?;
                    }
                    28 => {
                        reader.u8()?;
                    }
                    29 => {
                        // raw id -> alignment value: 0 -> 1 (left), 1 -> 0 (centred), 2 -> 2.
                        element.horizontal = match reader.u8()? {
                            0 => 1,
                            2 => 2,
                            _ => 0,
                        };
                    }
                    30 => {
                        // raw id -> alignment value: 0 -> 2 (top), 1 -> 0 (centred), 2 -> 1.
                        element.vertical = match reader.u8()? {
                            0 => 2,
                            2 => 1,
                            _ => 0,
                        };
                    }
                    _ => {}
                }
                Some(())
            })();
            if ok.is_none() {
                break;
            }
        }
        element
    }
}

/// An indexed-colour sprite of the `mapscene` group.
#[derive(Debug, Clone)]
pub struct MapScene {
    /// Frame size.
    sub_width: i32,
    sub_height: i32,
    /// Canvas size.
    width: i32,
    height: i32,
    x_offset: i32,
    y_offset: i32,
    palette: Vec<u32>,
    indices: Vec<u8>,
}

/// Everything loaded once: areas, elements and map-scene sprites.
#[derive(Debug, Clone)]
pub struct Catalog {
    pub areas: Vec<Area>,
    pub elements: HashMap<u32, Element>,
    scenes: Vec<MapScene>,
}

impl Catalog {
    pub fn load(definitions: &WorldDefinitions) -> Result<Self, WorldError> {
        let cache = definitions.cache();
        let missing = |what: &str| WorldError::Placement(format!("world map: {what}"));
        let details = cache
            .read_group_files(DETAILS_INDEX, DETAILS_GROUP)
            .map_err(|error| missing(&format!("details: {error}")))?;
        let composite: HashMap<u32, Vec<u8>> = cache
            .read_group_files(DETAILS_INDEX, COMPOSITE_MAP_GROUP)
            .map_err(|error| missing(&format!("compositemap: {error}")))?
            .into_iter()
            .map(|(id, file)| (id, file.bytes))
            .collect();
        let mut areas = Vec::new();
        for (id, file) in details {
            let Some(composite) = composite.get(&id) else {
                continue;
            };
            if let Some(area) = read_area(&file.bytes, composite, id) {
                areas.push(area);
            }
        }
        areas.sort_by(|a, b| {
            b.is_main
                .cmp(&a.is_main)
                .then_with(|| a.external_name.cmp(&b.external_name))
        });

        let mut elements = HashMap::new();
        if let Ok(files) = cache.read_group_files(CONFIG_INDEX, ELEMENT_GROUP) {
            for (id, file) in files {
                elements.insert(id, Element::read(id, &file.bytes));
            }
        }
        let scenes = load_map_scenes(definitions);
        Ok(Self {
            areas,
            elements,
            scenes,
        })
    }

    pub fn main_area(&self) -> usize {
        self.areas.iter().position(|area| area.is_main).unwrap_or(0)
    }
}

fn load_map_scenes(definitions: &WorldDefinitions) -> Vec<MapScene> {
    let cache = definitions.cache();
    let Ok(groups) = cache.group_ids(SPRITE_INDEX) else {
        return Vec::new();
    };
    let Some(group) = groups.into_iter().find(|&group| {
        cache
            .group_metadata(SPRITE_INDEX, group)
            .is_ok_and(|meta| meta.name_hash == MAPSCENE_NAME_HASH)
    }) else {
        return Vec::new();
    };
    let Ok(file) = cache.read_file(SPRITE_INDEX, group, 0, None) else {
        return Vec::new();
    };
    let Ok(frames) = decode_sprite_group(group, &file.bytes, definitions.context(), &file.provenance)
    else {
        return Vec::new();
    };
    frames
        .into_iter()
        .map(|frame| MapScene {
            sub_width: i32::from(frame.width),
            sub_height: i32::from(frame.height),
            width: i32::from(frame.max_width),
            height: i32::from(frame.max_height),
            x_offset: i32::from(frame.x_offset),
            y_offset: i32::from(frame.y_offset),
            palette: frame.palette.clone(),
            indices: frame.indices.clone(),
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// geography
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Decoration {
    object: i32,
    kind: u8,
    rotation: u8,
}

/// The tiles of one map region (the union of its `WorldMapData_0/1` files).
struct Geography {
    underlay: Vec<i16>,
    overlay: [Vec<i16>; 4],
    shape: [Vec<u8>; 4],
    rotation: [Vec<u8>; 4],
    decorations: [Vec<Vec<Decoration>>; 4],
    planes: usize,
}

const TILE_COUNT: usize = 64 * 64;

impl Geography {
    fn new() -> Self {
        Self {
            underlay: vec![0; TILE_COUNT],
            overlay: std::array::from_fn(|_| vec![0; TILE_COUNT]),
            shape: std::array::from_fn(|_| vec![0; TILE_COUNT]),
            rotation: std::array::from_fn(|_| vec![0; TILE_COUNT]),
            decorations: std::array::from_fn(|_| vec![Vec::new(); TILE_COUNT]),
            planes: 1,
        }
    }

    /// `AbstractWorldMapData.readTile`.
    fn read_tile(&mut self, x: usize, y: usize, reader: &mut Reader<'_>) -> Option<()> {
        // Build 238+ stores decoration object ids as plain ints (`readInt`).
        let flags = reader.u8()?;
        if flags == 0 {
            return Some(());
        }
        let at = x * 64 + y;
        if flags & 1 != 0 {
            if flags & 2 != 0 {
                self.overlay[0][at] = reader.u16()? as i16;
            }
            self.underlay[at] = reader.u16()? as i16;
            return Some(());
        }
        let deco_planes = (((flags & 24) >> 3) + 1) as usize;
        self.underlay[at] = reader.u16()? as i16;
        if flags & 2 != 0 {
            let count = reader.u8()? as usize;
            for plane in 0..count {
                let id = reader.u16()?;
                if id != 0 {
                    let packed = reader.u8()?;
                    if plane < 4 {
                        self.overlay[plane][at] = id as i16;
                        self.shape[plane][at] = (packed >> 2) as u8;
                        self.rotation[plane][at] = (packed & 3) as u8;
                    }
                }
            }
        }
        if flags & 4 != 0 {
            for plane in 0..deco_planes {
                let count = reader.u8()?;
                if count == 0 {
                    continue;
                }
                let mut list = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let object = reader.i32()?;
                    let packed = reader.u8()?;
                    list.push(Decoration {
                        object,
                        kind: (packed >> 2) as u8,
                        rotation: (packed & 3) as u8,
                    });
                }
                if plane < 4 {
                    self.decorations[plane][at] = list;
                }
            }
        }
        Some(())
    }
}

/// Geography and ground group of a map region: the region coordinates packed `x << 8 | y`.
fn region_group(region_x: i32, region_y: i32) -> u32 {
    ((region_x << 8) | region_y) as u32
}

fn load_geography(
    definitions: &WorldDefinitions,
    area: &Area,
    region_x: i32,
    region_y: i32,
) -> Option<(Geography, u32, u32)> {
    let whole = area.regions.iter().any(|data| data.region == (region_x, region_y));
    let chunks: Vec<(i32, i32)> = area
        .chunks
        .iter()
        .filter(|data| data.region == (region_x, region_y))
        .map(|data| data.chunk)
        .collect();
    if !whole && chunks.is_empty() {
        return None;
    }
    // Build 238+: group `x << 8 | y`, file = the area id. A whole region holds its 64x64 tiles,
    // a zoned region the 8x8 zones listed in the compositemap, back to back.
    let file_id = area.id;
    let group = region_group(region_x, region_y);
    let file = definitions.cache().read_file(GEOGRAPHY_INDEX, group, file_id, None).ok()?;
    let mut reader = Reader::new(&file.bytes);
    let mut geography = Geography::new();
    if whole {
        for x in 0..64 {
            for y in 0..64 {
                geography.read_tile(x, y, &mut reader)?;
            }
        }
    } else {
        for &(chunk_x, chunk_y) in &chunks {
            for x in 0..8 {
                for y in 0..8 {
                    geography.read_tile(chunk_x as usize * 8 + x, chunk_y as usize * 8 + y, &mut reader)?;
                }
            }
        }
    }
    geography.planes = 4;
    Some((geography, group, file_id))
}

/// The 64x64 pre-blended ground colours (`WorldMapSprite`), ARGB, indexed `y * 64 + x`.
fn load_ground(definitions: &WorldDefinitions, group: u32, file_id: u32) -> Vec<u32> {
    let mut colours = vec![0_u32; TILE_COUNT];
    let Ok(file) = definitions.cache().read_file(GROUND_INDEX, group, file_id, None) else {
        return colours;
    };
    let mut decoder = png::Decoder::new(std::io::Cursor::new(&file.bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let Ok(mut reader) = decoder.read_info() else {
        return colours;
    };
    let mut buffer = vec![0_u8; reader.output_buffer_size().unwrap_or(0)];
    let Ok(info) = reader.next_frame(&mut buffer) else {
        return colours;
    };
    let channels = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Indexed => return colours,
    };
    for index in 0..TILE_COUNT.min((info.width * info.height) as usize) {
        let base = index * channels;
        let (r, g, b, a) = match channels {
            1 => (buffer[base], buffer[base], buffer[base], 255),
            2 => (buffer[base], buffer[base], buffer[base], buffer[base + 1]),
            3 => (buffer[base], buffer[base + 1], buffer[base + 2], 255),
            _ => (buffer[base], buffer[base + 1], buffer[base + 2], buffer[base + 3]),
        };
        colours[index] = u32::from(a) << 24 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b);
    }
    colours
}

// ---------------------------------------------------------------------------------------------
// icons
// ---------------------------------------------------------------------------------------------

/// A map icon resolved to a position in an area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapIcon {
    pub element: u32,
    /// Absolute map tile (area coordinates).
    pub x: i32,
    pub y: i32,
    /// Fixed `compositemap` icons are positioned at the tile corner; scenery icons use the
    /// `iconMap` placement (half a tile left of the tile, vertically centred).
    pub link: bool,
}

/// An object's current map icon: the object itself, or its first transform at all-zero varps.
fn object_icon(definitions: &mut WorldDefinitions, object: i32) -> Option<u32> {
    let definition = definitions.object(ObjectId::new(u32::try_from(object).ok()?)).ok()??;
    if let Some(morphs) = &definition.morphs {
        let target = morphs.transforms.first().copied().flatten().or(morphs.fallback);
        let target = definitions.object(target?).ok()??;
        return target.map_icon.map(|icon| icon.get());
    }
    definition.map_icon.map(|icon| icon.get())
}

fn has_icon(definitions: &mut WorldDefinitions, object: i32) -> bool {
    let Ok(id) = u32::try_from(object) else {
        return false;
    };
    let Ok(Some(definition)) = definitions.object(ObjectId::new(id)) else {
        return false;
    };
    if let Some(morphs) = &definition.morphs {
        let targets: Vec<ObjectId> = morphs
            .transforms
            .iter()
            .flatten()
            .copied()
            .chain(morphs.fallback)
            .collect();
        return targets.into_iter().any(|target| {
            matches!(definitions.object(target), Ok(Some(def)) if def.map_icon.is_some())
        });
    }
    definition.map_icon.is_some()
}

/// All scenery icons of an area plus its fixed icons, as the client builds them
/// (`WorldMapRegion.getIconsForTiles` and `addAllToIconList`).
pub fn area_icons(
    definitions: &mut WorldDefinitions,
    catalog: &Catalog,
    area_index: usize,
    members: bool,
) -> Vec<MapIcon> {
    let area = &catalog.areas[area_index];
    let mut icons = Vec::new();
    for (region_x, region_y) in area.region_keys() {
        let Some((geography, _, _)) = load_geography(definitions, area, region_x, region_y) else {
            continue;
        };
        for x in 0..64 {
            for y in 0..64 {
                let at = x * 64 + y;
                'tile: for plane in 0..geography.planes {
                    for decoration in &geography.decorations[plane][at] {
                        if has_icon(definitions, decoration.object) {
                            if let Some(element) = object_icon(definitions, decoration.object) {
                                let visible = catalog
                                    .elements
                                    .get(&element)
                                    .is_some_and(|element| element.visible);
                                if visible {
                                    icons.push(MapIcon {
                                        element,
                                        x: region_x * 64 + x as i32,
                                        y: region_y * 64 + y as i32,
                                        link: false,
                                    });
                                }
                            }
                            break 'tile;
                        }
                    }
                }
            }
        }
    }
    for link in &area.links {
        if link.element < 0 || (link.members && !members) {
            continue;
        }
        let visible = catalog
            .elements
            .get(&(link.element as u32))
            .is_some_and(|element| element.visible);
        if visible {
            icons.push(MapIcon {
                element: link.element as u32,
                x: link.x,
                y: link.y,
                link: true,
            });
        }
    }
    icons
}

// ---------------------------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------------------------

/// One rendered region at a pixels-per-tile value.
#[derive(Debug, Clone)]
pub struct RegionImage {
    pub region: (i32, i32),
    pub pixels_per_tile: i32,
    /// `64 * ppt` squared, RGBA, row 0 = northernmost tile row.
    pub rgba: Vec<u8>,
}

/// Renders region images; caches the palette and the shape masks.
pub struct RegionRenderer {
    palette: Vec<i32>,
    masks: HashMap<(i32, usize, usize), Vec<bool>>,
}

impl Default for RegionRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// `class239.method5241`: rewrite the lightness bits of a packed HSL (`-1` selects `127 - light`).
fn rewrite_lightness(packed: i32, light: i32) -> i32 {
    match packed {
        -2 => 12_345_678,
        -1 => 127 - light.clamp(0, 127),
        _ => {
            let scaled = ((packed & 127) * light / 128).clamp(2, 126);
            (packed & 65_408) + scaled
        }
    }
}

struct Canvas {
    size: i32,
    pixels: Vec<u32>,
}

impl Canvas {
    fn put(&mut self, x: i32, y: i32, colour: u32) {
        if x >= 0 && y >= 0 && x < self.size && y < self.size {
            self.pixels[(y * self.size + x) as usize] = colour;
        }
    }

    fn fill(&mut self, x: i32, y: i32, width: i32, height: i32, colour: u32) {
        for row in y..y + height {
            for column in x..x + width {
                self.put(column, row, colour);
            }
        }
    }

    fn horizontal(&mut self, x: i32, y: i32, length: i32, colour: u32) {
        for column in x..x + length {
            self.put(column, y, colour);
        }
    }

    fn vertical(&mut self, x: i32, y: i32, length: i32, colour: u32) {
        for row in y..y + length {
            self.put(x, row, colour);
        }
    }

    /// `IndexedSprite.method12104`: nearest-neighbour scaled blit of a map-scene sprite.
    fn blit_scene(&mut self, sprite: &MapScene, mut x: i32, mut y: i32, mut width: i32, mut height: i32) {
        if width <= 0 || height <= 0 {
            return;
        }
        let (sub_width, sub_height) = (sprite.sub_width, sprite.sub_height);
        let (mut source_x, mut source_y) = (0_i32, 0_i32);
        let step_x = (sprite.width << 16) / width;
        let step_y = (sprite.height << 16) / height;
        if step_x == 0 || step_y == 0 {
            return;
        }
        if sprite.x_offset > 0 {
            let shift = (step_x + (sprite.x_offset << 16) - 1) / step_x;
            x += shift;
            source_x += shift * step_x - (sprite.x_offset << 16);
        }
        if sprite.y_offset > 0 {
            let shift = (step_y + (sprite.y_offset << 16) - 1) / step_y;
            y += shift;
            source_y += shift * step_y - (sprite.y_offset << 16);
        }
        if sub_width < sprite.width {
            width = (step_x + ((sub_width << 16) - source_x) - 1) / step_x;
        }
        if sub_height < sprite.height {
            height = (step_y + ((sub_height << 16) - source_y) - 1) / step_y;
        }
        // Clip to the canvas.
        if y + height > self.size {
            height -= y + height - self.size;
        }
        if y < 0 {
            height += y;
            source_y += step_y * -y;
            y = 0;
        }
        if x + width > self.size {
            width -= x + width - self.size;
        }
        if x < 0 {
            width += x;
            source_x += step_x * -x;
            x = 0;
        }
        let mut row_source = source_y;
        for row in 0..height.max(0) {
            let line = sub_width * (row_source >> 16);
            let mut column_source = source_x;
            for column in 0..width.max(0) {
                let index = (column_source >> 16) + line;
                if let Some(&palette_index) = usize::try_from(index).ok().and_then(|i| sprite.indices.get(i)) {
                    if palette_index != 0 {
                        let rgb = sprite.palette.get(usize::from(palette_index)).copied().unwrap_or(0);
                        self.put(x + column, y + row, 0xFF00_0000 | rgb);
                    }
                }
                column_source += step_x;
            }
            row_source += step_y;
        }
    }
}

impl RegionRenderer {
    pub fn new() -> Self {
        Self {
            palette: build_color_palette(PALETTE_BRIGHTNESS),
            masks: HashMap::new(),
        }
    }

    /// `VerticalAlignment.method4541`: the colour of an overlay id (zero-based).
    fn overlay_colour(&self, definitions: &WorldDefinitions, overlay: i32, background: u32) -> u32 {
        let floors = definitions.floors();
        let definition = floors.overlay(overlay as u32);
        let packed = if definition.secondary_rgb >= 0 {
            pack_terrain_hsl(
                definition.secondary_hue,
                definition.secondary_saturation,
                definition.secondary_lightness,
            )
        } else if definition.texture >= 0 {
            floors.texture_average_rgb(definition.texture)
        } else if definition.primary_rgb == 0x00FF_00FF {
            return background;
        } else {
            pack_terrain_hsl(definition.hue, definition.saturation, definition.lightness)
        };
        let index = rewrite_lightness(packed, OVERLAY_LIGHT);
        let rgb = usize::try_from(index)
            .ok()
            .and_then(|index| self.palette.get(index))
            .copied()
            .unwrap_or(0);
        0xFF00_0000 | (rgb as u32 & 0x00FF_FFFF)
    }

    /// `WorldMapRegion.drawTile`: render one region, or `None` when the area has no data there.
    pub fn render(
        &mut self,
        definitions: &mut WorldDefinitions,
        catalog: &Catalog,
        area_index: usize,
        region: (i32, i32),
        ppt: i32,
    ) -> Option<RegionImage> {
        let area = catalog.areas.get(area_index)?;
        let (geography, ground_group, ground_file) = load_geography(definitions, area, region.0, region.1)?;
        let ground = load_ground(definitions, ground_group, ground_file);
        let size = 64 * ppt;
        let mut canvas = Canvas {
            size,
            pixels: vec![0; (size * size) as usize],
        };
        let background = area.background;
        // Pass 1: ground and upper overlays.
        for x in 0..64_i32 {
            for y in 0..64_i32 {
                self.draw_ground(&mut canvas, definitions, &geography, &ground, x, y, ppt, background);
                self.draw_upper_overlays(&mut canvas, definitions, &geography, x, y, ppt, background);
            }
        }
        // Pass 2: wall lines then map scenes, tile by tile.
        for x in 0..64_i32 {
            for y in 0..64_i32 {
                Self::draw_walls(&mut canvas, definitions, &geography, x, y, ppt);
                Self::draw_scenes(&mut canvas, definitions, catalog, &geography, x, y, ppt);
            }
        }
        let mut rgba = vec![0_u8; canvas.pixels.len() * 4];
        for (index, pixel) in canvas.pixels.iter().enumerate() {
            rgba[index * 4] = (pixel >> 16) as u8;
            rgba[index * 4 + 1] = (pixel >> 8) as u8;
            rgba[index * 4 + 2] = *pixel as u8;
            rgba[index * 4 + 3] = if *pixel == 0 { 0 } else { 255 };
        }
        Some(RegionImage {
            region,
            pixels_per_tile: ppt,
            rgba,
        })
    }

    fn mask(&mut self, ppt: i32, shape: usize, rotation: usize) -> &[bool] {
        self.masks
            .entry((ppt, shape, rotation))
            .or_insert_with(|| templates::build(ppt, shape, rotation))
    }

    /// `drawTileGround`.
    #[allow(clippy::too_many_arguments)]
    fn draw_ground(
        &mut self,
        canvas: &mut Canvas,
        definitions: &WorldDefinitions,
        geography: &Geography,
        ground: &[u32],
        x: i32,
        y: i32,
        ppt: i32,
        background: u32,
    ) {
        let at = (x * 64 + y) as usize;
        let underlay = i32::from(geography.underlay[at]) - 1;
        let overlay = i32::from(geography.overlay[0][at]) - 1;
        let (px, py) = (ppt * x, ppt * (63 - y));
        if underlay == -1 && overlay == -1 {
            canvas.fill(px, py, ppt, ppt, background);
        }
        let overlay_colour = if overlay != -1 {
            self.overlay_colour(definitions, overlay, background)
        } else {
            0x00FF_00FF
        };
        let shape = geography.shape[0][at];
        if overlay > -1 && shape == 0 {
            canvas.fill(px, py, ppt, ppt, overlay_colour);
            return;
        }
        let ground_colour = if geography.underlay[at] == 0 {
            background
        } else {
            ground[(y * 64 + x) as usize]
        };
        if overlay == -1 {
            canvas.fill(px, py, ppt, ppt, ground_colour);
        } else {
            self.gradient(canvas, px, py, ppt, ground_colour, overlay_colour, shape, geography.rotation[0][at]);
        }
    }

    /// `method6713`: overlays of the planes above the ground.
    #[allow(clippy::too_many_arguments)]
    fn draw_upper_overlays(
        &mut self,
        canvas: &mut Canvas,
        definitions: &WorldDefinitions,
        geography: &Geography,
        x: i32,
        y: i32,
        ppt: i32,
        background: u32,
    ) {
        let at = (x * 64 + y) as usize;
        for plane in 1..geography.planes {
            let overlay = i32::from(geography.overlay[plane][at]) - 1;
            if overlay <= -1 {
                continue;
            }
            let colour = self.overlay_colour(definitions, overlay, background);
            let (px, py) = (ppt * x, ppt * (63 - y));
            let shape = geography.shape[plane][at];
            if shape == 0 {
                canvas.fill(px, py, ppt, ppt, colour);
            } else {
                self.gradient(canvas, px, py, ppt, 0, colour, shape, geography.rotation[plane][at]);
            }
        }
    }

    /// `WorldMapScaleHandler.method7129` + `Rasterizer2D_drawGradientPixels` (masked fill).
    #[allow(clippy::too_many_arguments)]
    fn gradient(
        &mut self,
        canvas: &mut Canvas,
        x: i32,
        y: i32,
        ppt: i32,
        ground: u32,
        overlay: u32,
        shape: u8,
        rotation: u8,
    ) {
        if shape == 0 {
            return;
        }
        let mut rotation = i32::from(rotation);
        if shape == 9 {
            rotation = (rotation + 1) & 3;
        }
        if shape == 10 || shape == 11 {
            rotation = (rotation + 3) & 3;
        }
        let template = match shape {
            9 | 10 => 1,
            11 => 8,
            other => usize::from(other),
        };
        if template == 0 || template > 8 {
            return;
        }
        let mask = self.mask(ppt, template - 1, rotation as usize).to_vec();
        for row in 0..ppt {
            for column in 0..ppt {
                let on = mask[(row * ppt + column) as usize];
                let colour = if on { overlay } else { ground };
                // A fully transparent colour leaves the pixel as it is.
                if colour >> 24 != 0 {
                    canvas.put(x + column, y + row, colour);
                }
            }
        }
    }

    /// `method6708`: wall and door lines.
    fn draw_walls(
        canvas: &mut Canvas,
        definitions: &mut WorldDefinitions,
        geography: &Geography,
        x: i32,
        y: i32,
        ppt: i32,
    ) {
        let at = (x * 64 + y) as usize;
        for plane in 0..geography.planes {
            for decoration in &geography.decorations[plane][at] {
                let kind = i32::from(decoration.kind);
                if !((0..=3).contains(&kind) || kind == 9) {
                    continue;
                }
                let Ok(object) = u32::try_from(decoration.object) else {
                    continue;
                };
                let Ok(Some(definition)) = definitions.object(ObjectId::new(object)) else {
                    continue;
                };
                let colour: u32 = if definition.map_interactive() {
                    0xFFCC_0000
                } else {
                    0xFFCC_CCCC
                };
                let rotation = i32::from(decoration.rotation);
                let (left, top) = (ppt * x, ppt * (63 - y));
                let line = |canvas: &mut Canvas, rotation: i32, colour: u32| match rotation % 4 {
                    0 => canvas.vertical(left, top, ppt, colour),
                    1 => canvas.horizontal(left, top, ppt, colour),
                    2 => canvas.vertical(left + ppt - 1, top, ppt, colour),
                    _ => canvas.horizontal(left, top + ppt - 1, ppt, colour),
                };
                match kind {
                    0 => line(canvas, rotation, colour),
                    2 => {
                        line(canvas, rotation, 0xFFCC_CCCC);
                        line(canvas, rotation + 1, colour);
                    }
                    3 => match rotation {
                        0 => canvas.horizontal(left, top, 1, colour),
                        1 => canvas.horizontal(left + ppt - 1, top, 1, colour),
                        2 => canvas.horizontal(left + ppt - 1, top + ppt - 1, 1, colour),
                        _ => canvas.horizontal(left, top + ppt - 1, 1, colour),
                    },
                    9 => {
                        if rotation % 2 == 0 {
                            for step in 0..ppt {
                                canvas.horizontal(step + left, (64 - y) * ppt - 1 - step, 1, colour);
                            }
                        } else {
                            for step in 0..ppt {
                                canvas.horizontal(step + left, step + top, 1, colour);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// `method6804`: map-scene sprites for scenery (trees, rocks ...).
    fn draw_scenes(
        canvas: &mut Canvas,
        definitions: &mut WorldDefinitions,
        catalog: &Catalog,
        geography: &Geography,
        x: i32,
        y: i32,
        ppt: i32,
    ) {
        let at = (x * 64 + y) as usize;
        for plane in 0..geography.planes {
            for decoration in &geography.decorations[plane][at] {
                let kind = i32::from(decoration.kind);
                if !(kind == 10 || kind == 11 || kind == 22) {
                    continue;
                }
                let Ok(object) = u32::try_from(decoration.object) else {
                    continue;
                };
                let Ok(Some(definition)) = definitions.object(ObjectId::new(object)) else {
                    continue;
                };
                let Some(scene) = definition.map_scene else {
                    continue;
                };
                let Some(sprite) = catalog.scenes.get(scene.get() as usize) else {
                    continue;
                };
                let length = if decoration.rotation != 1 && decoration.rotation != 3 {
                    i32::from(definition.size_y)
                } else {
                    i32::from(definition.size_x)
                };
                canvas.blit_scene(sprite, ppt * x, ppt * (64 - length - y), ppt * 2, ppt * 2);
            }
        }
    }
}

/// The overview picture of an area (`compositetexture`), decoded to RGBA.
pub fn load_overview(definitions: &WorldDefinitions, area: &Area) -> Option<(u32, u32, Vec<u8>)> {
    let file = definitions
        .cache()
        .read_file(DETAILS_INDEX, COMPOSITE_TEXTURE_GROUP, area.id, None)
        .ok()?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(&file.bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0_u8; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buffer).ok()?;
    let pixels = (info.width * info.height) as usize;
    let mut rgba = vec![255_u8; pixels * 4];
    for index in 0..pixels {
        match info.color_type {
            png::ColorType::Rgb => rgba[index * 4..index * 4 + 3].copy_from_slice(&buffer[index * 3..index * 3 + 3]),
            png::ColorType::Rgba => rgba[index * 4..index * 4 + 4].copy_from_slice(&buffer[index * 4..index * 4 + 4]),
            png::ColorType::Grayscale => rgba[index * 4..index * 4 + 3].fill(buffer[index]),
            png::ColorType::GrayscaleAlpha => {
                rgba[index * 4..index * 4 + 3].fill(buffer[index * 2]);
                rgba[index * 4 + 3] = buffer[index * 2 + 1];
            }
            png::ColorType::Indexed => return None,
        }
    }
    Some((info.width, info.height, rgba))
}
