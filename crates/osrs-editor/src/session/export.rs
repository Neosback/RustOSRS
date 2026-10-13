//! Convert an edited semantic region back into build-241 map-square streams.
//!
//! Export is a pure function of the original decoded square and the authoritative
//! `MapDocument`: the document is diffed against the semantic baseline of that same square and
//! only what differs is patched into the original tiles and locations. Untouched tiles keep their
//! exact opcodes, so an unedited square re-encodes byte-for-byte and is reported as `None`
//! (untouched archives are never rewritten).

use super::region_diff::{DiffError, RegionDiff, TILES_PER_PLANE, diff_region, plane_of, tile_of};
use crate::heights::{decode_height, encode_height};
use osrs_cache::decode::{
    DecodedLocation, DecodedLocations, DecodedTerrain, EncodedTerrainOverlay, EncodedTerrainTile,
    EncodedTileHeight,
};
use osrs_cache::encode::{MapEncodeError, encode_locations, encode_terrain};
use osrs_core::coords::{MapTile, RegionCoord, RegionTile};
use osrs_edit::{HeightVertexCoord, MapDocument, MapPosition, ObjectPlacement};
use osrs_scene::terrain_load::{
    REGION_TILES, RegionTerrain, SCENE_PLANES, TerrainLoadError, TerrainLoadGrid,
};
use osrs_world::region_tiles_from_decoded;
use std::collections::HashSet;

#[cfg(test)]
mod tests;

const FLOOR_ID_MASK: u16 = 0x7fff;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError {
    /// The document could not be compared with the source square.
    Diff(DiffError),
    /// The patched tile list was not a valid region terrain.
    Terrain(TerrainLoadError),
    /// An edited height cannot be stored: the format keeps multiples of 8 below the plane under it.
    HeightNotStorable { position: MapPosition, height: i32 },
    /// The document holds a value the map streams cannot represent.
    Encode(MapEncodeError),
}

impl From<MapEncodeError> for ExportError {
    fn from(error: MapEncodeError) -> Self {
        Self::Encode(error)
    }
}

impl From<DiffError> for ExportError {
    fn from(error: DiffError) -> Self {
        Self::Diff(error)
    }
}

/// What changed in one exported region, for the export report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExportChanges {
    pub tiles: usize,
    pub heights: usize,
    pub objects_added: usize,
    pub objects_removed: usize,
}

/// The re-encoded `terrain` (file 0) and `locations` (file 1) streams of one edited square.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedRegion {
    pub region: RegionCoord,
    pub terrain: Vec<u8>,
    pub locations: Vec<u8>,
    pub changes: ExportChanges,
}

/// Re-encode `region` from `document`, or `None` when the document matches the source square.
pub fn export_region(
    terrain: &DecodedTerrain,
    locations: &DecodedLocations,
    document: &MapDocument,
) -> Result<Option<ExportedRegion>, ExportError> {
    let region = terrain.region();
    let diff = diff_region(terrain, locations, document)?;
    if diff.is_empty() {
        return Ok(None);
    }
    let mut tiles = terrain.tiles().to_vec();
    patch_channels(&mut tiles, &diff);
    let heights = patch_heights(region, &mut tiles, document)?;
    let patched_locations = patch_locations(region, locations, &diff);

    let changes = ExportChanges {
        tiles: diff.tiles.len(),
        heights,
        objects_added: diff.objects_added.len(),
        objects_removed: diff.objects_removed.len(),
    };
    let patched = DecodedTerrain::from_parts(region, tiles, 0);
    Ok(Some(ExportedRegion {
        region,
        terrain: encode_terrain(&patched)?,
        locations: encode_locations(&patched_locations)?,
        changes,
    }))
}

/// Index of a tile in the plane/x/y terrain stream.
fn stream_index(position: MapPosition) -> usize {
    let (_, local) = position.tile.split_region();
    usize::from(position.plane.index().get()) * TILES_PER_PLANE
        + usize::from(local.x()) * REGION_TILES
        + usize::from(local.y())
}

/// Keep the source's raw id bits (bit 15 is preserved by the stream) when the semantic id is
/// unchanged; otherwise store the new id.
fn raw_id(original: u16, semantic: u32) -> u16 {
    if u32::from(original & FLOOR_ID_MASK) == semantic {
        original
    } else {
        semantic as u16
    }
}

fn patch_channels(tiles: &mut [EncodedTerrainTile], diff: &RegionDiff) {
    for &(position, after) in &diff.tiles {
        let tile = &mut tiles[stream_index(position)];
        tile.underlay_id = after
            .underlay
            .map_or(0, |id| raw_id(tile.underlay_id, id.get()));
        tile.overlay = after.overlay.map(|overlay| EncodedTerrainOverlay {
            encoded_id: raw_id(
                tile.overlay.map_or(0, |existing| existing.encoded_id),
                overlay.id.get(),
            ),
            shape: overlay.shape.get(),
            rotation: overlay.rotation.get(),
        });
        tile.settings = after.flags;
    }
}

/// Plane by plane, re-store every vertex whose loaded height differs from the document. Lower
/// planes are settled first because explicit heights are relative to the plane beneath. Returns
/// how many vertices were re-stored.
fn patch_heights(
    region: RegionCoord,
    tiles: &mut [EncodedTerrainTile],
    document: &MapDocument,
) -> Result<usize, ExportError> {
    let origin = MapTile::from_region(
        region,
        RegionTile::new(0, 0).unwrap_or_else(|| unreachable!("region origin is valid")),
    )
    .unwrap_or_else(|| unreachable!("a loaded region has in-range world coordinates"));
    let mut touched = HashSet::new();
    for plane in 0..SCENE_PLANES {
        let source = RegionTerrain::new(region_tiles_from_decoded(&DecodedTerrain::from_parts(
            region,
            tiles.to_vec(),
            0,
        )))
        .map_err(ExportError::Terrain)?;
        let mut grid = TerrainLoadGrid::new(REGION_TILES, REGION_TILES)
            .unwrap_or_else(|error| unreachable!("64x64 terrain grid is valid: {error:?}"));
        grid.load_region(&source, (0, 0), (origin.x, origin.y), 0);

        for x in 0..REGION_TILES {
            for y in 0..REGION_TILES {
                let index = plane * TILES_PER_PLANE + x * REGION_TILES + y;
                let vertex = HeightVertexCoord::new(tile_of(region, index), plane_of(index));
                let Some(target) = document.height_at(vertex) else {
                    continue;
                };
                if grid.height(plane, x, y) == target {
                    continue;
                }
                let base = if plane == 0 {
                    0
                } else {
                    grid.height(plane - 1, x, y)
                };
                let byte = encode_height(base, target);
                if decode_height(base, byte) != target {
                    return Err(ExportError::HeightNotStorable {
                        position: MapPosition::new(vertex.corner, vertex.plane),
                        height: target,
                    });
                }
                tiles[index].height = EncodedTileHeight::Explicit(byte);
                touched.insert(index);
            }
        }
    }
    Ok(touched.len())
}

fn placement_of(region: RegionCoord, location: &DecodedLocation) -> Option<ObjectPlacement> {
    let tile = MapTile::from_region(region, location.tile)?;
    Some(ObjectPlacement {
        object_id: location.object_id,
        position: MapPosition::new(tile, location.source_plane),
        kind: osrs_edit::LocKind::new(location.loc_type)?,
        orientation: osrs_edit::ObjectOrientation::new(location.orientation)?,
    })
}

fn location_of(placement: ObjectPlacement) -> DecodedLocation {
    let (_, tile) = placement.position.tile.split_region();
    DecodedLocation {
        object_id: placement.object_id,
        tile,
        source_plane: placement.position.plane,
        loc_type: placement.kind.get(),
        orientation: placement.orientation.get(),
    }
}

fn patch_locations(
    region: RegionCoord,
    original: &DecodedLocations,
    diff: &RegionDiff,
) -> DecodedLocations {
    let removed: HashSet<ObjectPlacement> = diff.objects_removed.iter().copied().collect();
    let mut kept: Vec<DecodedLocation> = original
        .locations()
        .iter()
        .filter(|location| {
            placement_of(region, location).is_none_or(|placement| !removed.contains(&placement))
        })
        .copied()
        .collect();
    kept.extend(diff.objects_added.iter().copied().map(location_of));
    DecodedLocations::with_locations(region, kept)
}
