//! Convert an edited semantic region back into build-241 map-square streams.
//!
//! Export is a pure function of the original decoded square and the authoritative
//! `MapDocument`: the document is diffed against the semantic baseline of that same square and
//! only what differs is patched into the original tiles and locations. Untouched tiles keep their
//! exact opcodes, so an unedited square re-encodes byte-for-byte and is reported as `None`
//! (untouched archives are never rewritten).

use super::region_ingest::{RegionAdaptError, adapt_region_map};
use crate::heights::{decode_height, encode_height};
use osrs_cache::decode::{
    DecodedLocation, DecodedLocations, DecodedTerrain, EncodedTerrainOverlay, EncodedTerrainTile,
    EncodedTileHeight,
};
use osrs_cache::encode::{MapEncodeError, encode_locations, encode_terrain};
use osrs_core::coords::{MapTile, RegionCoord, RegionTile, SourcePlane};
use osrs_edit::{HeightVertexCoord, MapDocument, MapPosition, ObjectPlacement};
use osrs_scene::terrain_load::{
    REGION_TILES, RegionTerrain, SCENE_PLANES, TerrainLoadError, TerrainLoadGrid,
};
use osrs_world::{RegionMap, region_tiles_from_decoded};
use std::collections::HashSet;

#[cfg(test)]
mod tests;

const FLOOR_ID_MASK: u16 = 0x7fff;
const TILES_PER_PLANE: usize = REGION_TILES * REGION_TILES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError {
    /// The source square could not be adapted to its semantic baseline.
    Baseline(RegionAdaptError),
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
    let baseline = baseline_document(terrain, locations)?;
    let mut tiles = terrain.tiles().to_vec();
    let mut changes = ExportChanges::default();

    patch_channels(region, &mut tiles, &baseline, document, &mut changes);
    patch_heights(region, &mut tiles, document, &mut changes)?;
    let locations = patch_locations(region, locations, &baseline, document, &mut changes);

    if changes == ExportChanges::default() {
        return Ok(None);
    }
    let patched = DecodedTerrain::from_parts(region, tiles, 0);
    Ok(Some(ExportedRegion {
        region,
        terrain: encode_terrain(&patched)?,
        locations: encode_locations(&locations)?,
        changes,
    }))
}

fn baseline_document(
    terrain: &DecodedTerrain,
    locations: &DecodedLocations,
) -> Result<MapDocument, ExportError> {
    let region_terrain =
        RegionTerrain::new(region_tiles_from_decoded(terrain)).map_err(ExportError::Terrain)?;
    let map = RegionMap {
        terrain: region_terrain,
        locations: DecodedLocations::with_locations(
            locations.region(),
            locations.locations().to_vec(),
        ),
    };
    let snapshot = adapt_region_map(&map).map_err(ExportError::Baseline)?;
    let mut document = MapDocument::new();
    document.replace_region(&snapshot);
    Ok(document)
}

fn plane_of(index: usize) -> SourcePlane {
    SourcePlane::new((index / TILES_PER_PLANE) as u8)
        .unwrap_or_else(|| unreachable!("a region holds four planes"))
}

fn tile_of(region: RegionCoord, index: usize) -> MapTile {
    let local = RegionTile::new(
        ((index / REGION_TILES) % REGION_TILES) as u8,
        (index % REGION_TILES) as u8,
    )
    .unwrap_or_else(|| unreachable!("local coordinates stay inside the region"));
    MapTile::from_region(region, local)
        .unwrap_or_else(|| unreachable!("a loaded region has in-range world coordinates"))
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

fn patch_channels(
    region: RegionCoord,
    tiles: &mut [EncodedTerrainTile],
    baseline: &MapDocument,
    document: &MapDocument,
    changes: &mut ExportChanges,
) {
    for (index, tile) in tiles.iter_mut().enumerate() {
        let position = MapPosition::new(tile_of(region, index), plane_of(index));
        let (before, after) = (baseline.terrain_at(position), document.terrain_at(position));
        if before == after {
            continue;
        }
        changes.tiles += 1;
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
/// planes are settled first because explicit heights are relative to the plane beneath.
fn patch_heights(
    region: RegionCoord,
    tiles: &mut [EncodedTerrainTile],
    document: &MapDocument,
    changes: &mut ExportChanges,
) -> Result<(), ExportError> {
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
                if touched.insert(index) {
                    changes.heights += 1;
                }
            }
        }
    }
    Ok(())
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
    baseline: &MapDocument,
    document: &MapDocument,
    changes: &mut ExportChanges,
) -> DecodedLocations {
    let before: HashSet<ObjectPlacement> = baseline.objects_in_region(region).into_iter().collect();
    let after: HashSet<ObjectPlacement> = document.objects_in_region(region).into_iter().collect();
    let removed: HashSet<_> = before.difference(&after).copied().collect();
    let added: Vec<_> = after.difference(&before).copied().collect();
    changes.objects_removed = removed.len();
    changes.objects_added = added.len();

    let mut kept: Vec<DecodedLocation> = original
        .locations()
        .iter()
        .filter(|location| {
            placement_of(region, location).is_none_or(|placement| !removed.contains(&placement))
        })
        .copied()
        .collect();
    kept.extend(added.into_iter().map(location_of));
    DecodedLocations::with_locations(region, kept)
}
