//! Planning a prefab back into a map: turn it, move it, and compare with what is there.

use super::{
    Prefab, PrefabError, footprint, objects_overlapping_rect, overlay_of, placement_of, plane_at,
};
use crate::{HeightVertexCoord, LocKind, MapDocument, MapOp, MapPatch, MapPosition, TerrainTile};
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::{FloorUnderlayId, ObjectId},
};
use std::collections::HashSet;

/// Where and how to put a prefab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaceSpec {
    /// South-west tile of the placed (already turned) rectangle.
    pub origin: MapTile,
    /// Plane the prefab's lowest plane lands on.
    pub base_plane: SourcePlane,
    /// Clockwise quarter turns, any value (taken modulo four).
    pub quarter_turns: u8,
    pub terrain: bool,
    pub heights: bool,
    pub objects: bool,
    /// Remove the objects already inside the rectangle first (a clean replacement).
    pub replace_objects: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedPrefab {
    pub patch: MapPatch,
    /// The rectangle the prefab covers once turned.
    pub width: u16,
    pub height: u16,
    /// Height vertices the map has no value for (left alone).
    pub skipped_heights: usize,
    /// Objects already standing exactly as the prefab wants them.
    pub already_present: usize,
}

/// The size of a `width` x `height` rectangle after `turns` quarter turns.
pub fn turned_size(width: u16, height: u16, turns: u8) -> (u16, u16) {
    if turns.is_multiple_of(2) {
        (width, height)
    } else {
        (height, width)
    }
}

/// One clockwise quarter turn of a tile in a `w` x `h` rectangle (south-west origin, y up).
fn turn_tile(x: i32, y: i32, w: i32) -> (i32, i32) {
    (y, w - 1 - x)
}

/// Plan the placement of `prefab` as one patch against `document`.
///
/// `size_of` gives an object's base size in tiles (from its definition); it only matters for
/// scenery and roof pieces, whose stored tile is the south-west corner of their footprint and so
/// moves when the prefab is turned. Terrain and heights already equal to the prefab's values
/// produce no operation.
pub fn plan_place_prefab(
    document: &MapDocument,
    prefab: &Prefab,
    spec: PlaceSpec,
    size_of: &dyn Fn(ObjectId, LocKind) -> (u8, u8),
) -> Result<PlacedPrefab, PrefabError> {
    prefab.validate()?;
    if plane_at(spec.base_plane, prefab.planes - 1).is_none() {
        return Err(PrefabError::PlaneOverflow {
            base: spec.base_plane.index().get(),
            planes: prefab.planes,
        });
    }
    let turns = spec.quarter_turns & 3;
    let (width, height) = turned_size(prefab.width, prefab.height, turns);
    let world = |x: i32, y: i32| MapTile::new(spec.origin.x + x, spec.origin.y + y);
    let plane = |offset: u8| {
        plane_at(spec.base_plane, offset)
            .unwrap_or_else(|| unreachable!("plane overflow was rejected above"))
    };
    let mut ops = Vec::new();
    let mut skipped_heights = 0;
    let mut already_present = 0;

    if spec.terrain {
        for tile in &prefab.tiles {
            let (mut x, mut y) = (i32::from(tile.x), i32::from(tile.y));
            let (mut w, mut h) = (i32::from(prefab.width), i32::from(prefab.height));
            for _ in 0..turns {
                (x, y) = turn_tile(x, y, w);
                (w, h) = (h, w);
            }
            let coord = MapPosition::new(world(x, y), plane(tile.plane));
            let after = TerrainTile {
                underlay: tile.underlay.map(FloorUnderlayId::new),
                overlay: tile.overlay.as_ref().and_then(|o| overlay_of(o, turns)),
                flags: tile.flags,
            };
            let before = document.terrain_at(coord);
            if before != after {
                ops.push(MapOp::SetTerrain {
                    coord,
                    before,
                    after,
                });
            }
        }
    }
    if spec.heights {
        for vertex in &prefab.heights {
            let (mut x, mut y) = (i32::from(vertex.x), i32::from(vertex.y));
            let (mut w, mut h) = (i32::from(prefab.width), i32::from(prefab.height));
            for _ in 0..turns {
                // Vertices turn like tiles in a rectangle one wider: x -> y, y -> w - x.
                (x, y) = (y, w - x);
                (w, h) = (h, w);
            }
            let coord = HeightVertexCoord::new(world(x, y), plane(vertex.plane));
            match document.height_at(coord) {
                None => skipped_heights += 1,
                Some(before) if before == vertex.value => {}
                Some(before) => ops.push(MapOp::SetHeight {
                    coord,
                    before,
                    after: vertex.value,
                }),
            }
        }
    }
    if spec.objects {
        let mut removing = HashSet::new();
        if spec.replace_objects {
            for offset in 0..prefab.planes {
                for placement in objects_overlapping_rect(
                    document,
                    (spec.origin.x, spec.origin.y),
                    (
                        spec.origin.x + i32::from(width),
                        spec.origin.y + i32::from(height),
                    ),
                    plane(offset),
                    size_of,
                ) {
                    if removing.insert(placement) {
                        ops.push(MapOp::RemoveObject { placement });
                    }
                }
            }
        }
        let mut adding = HashSet::new();
        for object in &prefab.objects {
            let (mut x, mut y) = (i32::from(object.x), i32::from(object.y));
            let (mut w, mut h) = (i32::from(prefab.width), i32::from(prefab.height));
            let mut orientation = object.orientation;
            for _ in 0..turns {
                let (fw, _) =
                    footprint(ObjectId::new(object.id), object.kind, orientation, size_of);
                // The anchor is the footprint's south-west tile; the turned footprint's is
                // `w - fw - x` up from the bottom (one tile footprints give `w - 1 - x`).
                (x, y) = (y, w - fw - x);
                (w, h) = (h, w);
                orientation = (orientation + 1) & 3;
            }
            let placement = placement_of(object, world(x, y), plane(object.plane), orientation)
                .ok_or(PrefabError::BadValue {
                    what: "object kind",
                    value: u32::from(object.kind),
                })?;
            if !adding.insert(placement) {
                continue;
            }
            if document.contains_object(placement) && !removing.contains(&placement) {
                already_present += 1;
                continue;
            }
            ops.push(MapOp::AddObject { placement });
        }
    }
    let patch = MapPatch::new(ops);
    document
        .validate_patch(&patch)
        .map_err(PrefabError::Patch)?;
    Ok(PlacedPrefab {
        patch,
        width,
        height,
        skipped_heights,
        already_present,
    })
}
