//! Prefabs: a named, reusable piece of map (terrain, heights and objects) over a rectangle of up
//! to four planes.
//!
//! A prefab is plain data in its own versioned format. It is captured from a `MapDocument`
//! (a real building, or a generated one) and later planned back into a document as one
//! `MapPatch`, optionally turned by quarter turns. Nothing here knows about egui, files, the
//! cache or workers; the editor owns storage and gestures, and commits the patch through the
//! normal edit gateway so one placement is one history entry.

use crate::{
    HeightVertexCoord, LocKind, MapDocument, MapOp, MapPatch, MapPosition, ObjectOrientation,
    ObjectPlacement, OverlayRotation, OverlayShape, PatchApplyError, TerrainOverlay, TerrainTile,
};
use osrs_core::{
    coords::{MapTile, RegionCoord, SourcePlane},
    ids::{FloorOverlayId, FloorUnderlayId, ObjectId},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt};

mod building;
mod capture;
mod foundation;
mod place;
pub use building::{
    BuildingCaptureSpec, BuildingPrefabError, PrefabBuilding, PrefabCell, PrefabPortal,
    PrefabRelation, PrefabRoom, PrefabSemantics, capture_building_prefab, capture_prefab_over,
};
pub use capture::{capture_prefab, prefab_from_patch};

pub use place::{
    PlaceSpec, PlacedPrefab, plan_place_exact, plan_place_prefab, prefab_placement_origin,
    turned_size,
};

#[cfg(test)]
mod tests;

pub const PREFAB_FORMAT: &str = "rustosrs-prefab";
pub const PREFAB_VERSION: u32 = 3;
/// The largest side of a prefab, in tiles (two map regions).
pub const MAX_PREFAB_SIDE: u16 = 128;

/// Where a prefab came from. Informational: nothing keys on it except display and regeneration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum PrefabSource {
    /// Copied out of a map (the south-west corner it came from, for reference).
    Captured { tile_x: i32, tile_y: i32, plane: u8 },
    /// Made by the building generator.
    Generated {
        archetype: String,
        style: String,
        seed: u64,
        floors: u8,
    },
    /// Built by hand.
    Authored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefabOverlay {
    pub id: u32,
    pub shape: u8,
    pub rotation: u8,
}

/// One tile's terrain channels, relative to the prefab's south-west corner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefabTile {
    pub x: u16,
    pub y: u16,
    pub plane: u8,
    pub underlay: Option<u32>,
    pub overlay: Option<PrefabOverlay>,
    pub flags: u8,
}

/// One height vertex (`0..=width` by `0..=height`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefabHeight {
    pub x: u16,
    pub y: u16,
    pub plane: u8,
    pub value: i32,
}

/// One object placement; `x`/`y` is the tile the map file stores (the footprint's south-west).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefabObject {
    pub id: u32,
    pub x: u16,
    pub y: u16,
    pub plane: u8,
    pub kind: u8,
    pub orientation: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prefab {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub tags: Vec<String>,
    pub notes: String,
    pub source: PrefabSource,
    pub width: u16,
    pub height: u16,
    pub planes: u8,
    pub tiles: Vec<PrefabTile>,
    pub heights: Vec<PrefabHeight>,
    pub objects: Vec<PrefabObject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub building: Option<PrefabBuilding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefabError {
    Format(String),
    Version(u32),
    EmptyName,
    BadSize {
        width: u16,
        height: u16,
        planes: u8,
    },
    OutOfBox {
        what: &'static str,
        x: u16,
        y: u16,
        plane: u8,
    },
    BadValue {
        what: &'static str,
        value: u32,
    },
    Duplicate {
        what: &'static str,
        x: u16,
        y: u16,
        plane: u8,
    },
    PlaneOverflow {
        base: u8,
        planes: u8,
    },
    Nothing,
    Patch(PatchApplyError),
    Building(BuildingPrefabError),
}

impl fmt::Display for PrefabError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format(found) => write!(f, "not a prefab file (format {found:?})"),
            Self::Version(version) => write!(f, "prefab version {version} is not supported"),
            Self::EmptyName => write!(f, "a prefab needs a name"),
            Self::BadSize {
                width,
                height,
                planes,
            } => write!(
                f,
                "a prefab is 1..={MAX_PREFAB_SIDE} tiles on a side and 1..=4 planes, not {width}x{height}x{planes}"
            ),
            Self::OutOfBox { what, x, y, plane } => {
                write!(f, "{what} at {x},{y} plane {plane} is outside the prefab")
            }
            Self::BadValue { what, value } => write!(f, "{what} {value} is not valid"),
            Self::Duplicate { what, x, y, plane } => {
                write!(f, "two {what} entries at {x},{y} plane {plane}")
            }
            Self::PlaneOverflow { base, planes } => {
                write!(f, "{planes} planes do not fit above plane {base}")
            }
            Self::Nothing => write!(f, "there is nothing to put in the prefab"),
            Self::Building(error) => write!(f, "building: {error}"),
            Self::Patch(error) => write!(f, "the placement does not apply: {error:?}"),
        }
    }
}

impl std::error::Error for PrefabError {}

impl Prefab {
    /// Check everything a file could get wrong, so later stages can rely on it.
    pub fn validate(&self) -> Result<(), PrefabError> {
        if self.format != PREFAB_FORMAT {
            return Err(PrefabError::Format(self.format.clone()));
        }
        if !(1..=PREFAB_VERSION).contains(&self.version) {
            return Err(PrefabError::Version(self.version));
        }
        if self.name.trim().is_empty() {
            return Err(PrefabError::EmptyName);
        }
        let side = 1..=MAX_PREFAB_SIDE;
        if !side.contains(&self.width)
            || !side.contains(&self.height)
            || !(1..=4).contains(&self.planes)
        {
            return Err(PrefabError::BadSize {
                width: self.width,
                height: self.height,
                planes: self.planes,
            });
        }
        let inside = |what, x: u16, y: u16, plane: u8, extra: u16| {
            if x < self.width + extra && y < self.height + extra && plane < self.planes {
                Ok(())
            } else {
                Err(PrefabError::OutOfBox { what, x, y, plane })
            }
        };
        let mut seen = HashSet::new();
        for tile in &self.tiles {
            inside("a tile", tile.x, tile.y, tile.plane, 0)?;
            if !seen.insert((tile.x, tile.y, tile.plane)) {
                return Err(PrefabError::Duplicate {
                    what: "tile",
                    x: tile.x,
                    y: tile.y,
                    plane: tile.plane,
                });
            }
            if let Some(overlay) = &tile.overlay {
                if overlay.shape > OverlayShape::MAX {
                    return Err(PrefabError::BadValue {
                        what: "overlay shape",
                        value: u32::from(overlay.shape),
                    });
                }
                if overlay.rotation > OverlayRotation::MAX {
                    return Err(PrefabError::BadValue {
                        what: "overlay rotation",
                        value: u32::from(overlay.rotation),
                    });
                }
            }
        }
        seen.clear();
        for height in &self.heights {
            inside("a height", height.x, height.y, height.plane, 1)?;
            if !seen.insert((height.x, height.y, height.plane)) {
                return Err(PrefabError::Duplicate {
                    what: "height",
                    x: height.x,
                    y: height.y,
                    plane: height.plane,
                });
            }
        }
        for object in &self.objects {
            inside("an object", object.x, object.y, object.plane, 0)?;
            if object.kind > LocKind::MAX {
                return Err(PrefabError::BadValue {
                    what: "object kind",
                    value: u32::from(object.kind),
                });
            }
            if object.orientation > ObjectOrientation::MAX {
                return Err(PrefabError::BadValue {
                    what: "object orientation",
                    value: u32::from(object.orientation),
                });
            }
        }
        if let Some(building) = &self.building {
            building.validate(self).map_err(PrefabError::Building)?;
        }
        Ok(())
    }

    /// A short line for lists: size and contents.
    pub fn summary(&self) -> String {
        format!(
            "{}x{} tiles, {} plane{}, {} objects",
            self.width,
            self.height,
            self.planes,
            if self.planes == 1 { "" } else { "s" },
            self.objects.len()
        )
    }
}

/// What to copy out of the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureSpec {
    /// South-west tile of the rectangle.
    pub origin: MapTile,
    pub width: u16,
    pub height: u16,
    /// Lowest plane copied; `planes` planes are taken from here upward.
    pub base_plane: SourcePlane,
    pub planes: u8,
    pub terrain: bool,
    pub heights: bool,
    pub objects: bool,
}

/// Every placement stored on a tile of `[min.0, max.0) x [min.1, max.1)` on `plane`.
pub fn objects_in_rect(
    document: &MapDocument,
    min: (i32, i32),
    max: (i32, i32),
    plane: SourcePlane,
) -> Vec<ObjectPlacement> {
    if max.0 <= min.0 || max.1 <= min.1 {
        return Vec::new();
    }
    let low = MapTile::new(min.0, min.1).split_region().0;
    let high = MapTile::new(max.0 - 1, max.1 - 1).split_region().0;
    let mut found = Vec::new();
    for ry in low.y..=high.y {
        for rx in low.x..=high.x {
            for placement in document.objects_in_region(RegionCoord::new(rx, ry)) {
                let tile = placement.position.tile;
                if placement.position.plane == plane
                    && (min.0..max.0).contains(&tile.x)
                    && (min.1..max.1).contains(&tile.y)
                {
                    found.push(placement);
                }
            }
        }
    }
    found
}

/// What an object covers, given its orientation: odd orientations swap the two sides.
pub(crate) fn footprint(
    id: ObjectId,
    kind: u8,
    orientation: u8,
    size_of: &dyn Fn(ObjectId, LocKind) -> (u8, u8),
) -> (i32, i32) {
    // Boundary pieces, wall decorations, diagonal walls and ground decoration take one tile
    // whatever the definition's size says.
    if !matches!(kind, 10 | 11) {
        return (1, 1);
    }
    let (sx, sy) = LocKind::new(kind).map_or((1, 1), |kind| size_of(id, kind));
    let (sx, sy) = (i32::from(sx.max(1)), i32::from(sy.max(1)));
    if orientation & 1 == 1 {
        (sy, sx)
    } else {
        (sx, sy)
    }
}

/// Every placement whose footprint touches `[min.0, max.0) x [min.1, max.1)` on `plane`: those
/// stored inside it and the big ones (a tree, a table) stored on a tile outside but reaching in.
pub fn objects_overlapping_rect(
    document: &MapDocument,
    min: (i32, i32),
    max: (i32, i32),
    plane: SourcePlane,
    size_of: &dyn Fn(ObjectId, LocKind) -> (u8, u8),
) -> Vec<ObjectPlacement> {
    /// No object is wider than this many tiles, so none stored further away can reach in.
    const REACH: i32 = 8;
    objects_in_rect(document, (min.0 - REACH, min.1 - REACH), max, plane)
        .into_iter()
        .filter(|p| {
            let (w, h) = footprint(p.object_id, p.kind.get(), p.orientation.get(), size_of);
            let tile = p.position.tile;
            tile.x < max.0 && tile.x + w > min.0 && tile.y < max.1 && tile.y + h > min.1
        })
        .collect()
}

pub(crate) fn plane_at(base: SourcePlane, offset: u8) -> Option<SourcePlane> {
    SourcePlane::new(base.index().get().checked_add(offset)?)
}

fn placement_of(
    object: &PrefabObject,
    tile: MapTile,
    plane: SourcePlane,
    orientation: u8,
) -> Option<ObjectPlacement> {
    Some(ObjectPlacement {
        object_id: ObjectId::new(object.id),
        position: MapPosition::new(tile, plane),
        kind: LocKind::new(object.kind)?,
        orientation: ObjectOrientation::new(orientation & 3)?,
    })
}

fn overlay_of(overlay: &PrefabOverlay, turns: u8) -> Option<TerrainOverlay> {
    Some(TerrainOverlay {
        id: FloorOverlayId::new(overlay.id),
        shape: OverlayShape::new(overlay.shape)?,
        rotation: OverlayRotation::new(overlay.rotation.wrapping_add(turns) & 3)?,
    })
}
