//! What an edit session changed in one source map square, relative to the square as stored.
//!
//! `RegionDiff` is the shared core of export and project files: it compares the authoritative
//! `MapDocument` with the semantic baseline of the original decoded square and keeps only the
//! differences, in canonical order. It is pure: no UI, renderer, worker, or cache I/O.

use super::region_ingest::{RegionAdaptError, adapt_region_map};
use osrs_cache::decode::{DecodedLocations, DecodedTerrain};
use osrs_core::coords::{MapTile, RegionCoord, RegionTile, SourcePlane};
use osrs_edit::{
    HeightVertexCoord, MapDocument, MapOp, MapPatch, MapPosition, ObjectPlacement, TerrainTile,
};
use osrs_scene::terrain_load::{REGION_TILES, RegionTerrain, SCENE_PLANES, TerrainLoadError};
use osrs_world::{RegionMap, region_tiles_from_decoded};

pub(super) const TILES_PER_PLANE: usize = REGION_TILES * REGION_TILES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffError {
    /// The document holds no state for this square, so a diff would read as "everything deleted".
    RegionNotLoaded(RegionCoord),
    /// The source square could not be adapted to its semantic baseline.
    Baseline(RegionAdaptError),
    /// The source tiles were not a valid region terrain.
    Terrain(TerrainLoadError),
}

/// The differences between the document and one stored square. Heights are final (absolute)
/// vertex heights, exactly as the document holds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionDiff {
    pub region: RegionCoord,
    pub tiles: Vec<(MapPosition, TerrainTile)>,
    pub heights: Vec<(HeightVertexCoord, i32)>,
    pub objects_added: Vec<ObjectPlacement>,
    pub objects_removed: Vec<ObjectPlacement>,
}

impl RegionDiff {
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
            && self.heights.is_empty()
            && self.objects_added.is_empty()
            && self.objects_removed.is_empty()
    }

    /// The patch that makes `current` match this diff, with `before` values read from `current`.
    /// Entries already satisfied by `current` produce no operation, so applying a diff twice is a
    /// no-op. Heights for vertices `current` does not hold are skipped.
    pub fn to_patch(&self, current: &MapDocument) -> MapPatch {
        let mut ops = Vec::new();
        for &(coord, after) in &self.tiles {
            let before = current.terrain_at(coord);
            if before != after {
                ops.push(MapOp::SetTerrain {
                    coord,
                    before,
                    after,
                });
            }
        }
        for &(coord, after) in &self.heights {
            if let Some(before) = current.height_at(coord)
                && before != after
            {
                ops.push(MapOp::SetHeight {
                    coord,
                    before,
                    after,
                });
            }
        }
        for &placement in &self.objects_removed {
            if current.contains_object(placement) {
                ops.push(MapOp::RemoveObject { placement });
            }
        }
        for &placement in &self.objects_added {
            if !current.contains_object(placement) {
                ops.push(MapOp::AddObject { placement });
            }
        }
        MapPatch::new(ops)
    }
}

/// The semantic state of a square exactly as stored (what the editor loads before any edit).
pub fn baseline_document(
    terrain: &DecodedTerrain,
    locations: &DecodedLocations,
) -> Result<MapDocument, DiffError> {
    let region_terrain =
        RegionTerrain::new(region_tiles_from_decoded(terrain)).map_err(DiffError::Terrain)?;
    let map = RegionMap {
        terrain: region_terrain,
        locations: DecodedLocations::with_locations(
            locations.region(),
            locations.locations().to_vec(),
        ),
    };
    let snapshot = adapt_region_map(&map).map_err(DiffError::Baseline)?;
    let mut document = MapDocument::new();
    document.replace_region(&snapshot);
    Ok(document)
}

pub(super) fn plane_of(index: usize) -> SourcePlane {
    SourcePlane::new((index / TILES_PER_PLANE) as u8)
        .unwrap_or_else(|| unreachable!("a region holds four planes"))
}

pub(super) fn tile_of(region: RegionCoord, index: usize) -> MapTile {
    let local = RegionTile::new(
        ((index / REGION_TILES) % REGION_TILES) as u8,
        (index % REGION_TILES) as u8,
    )
    .unwrap_or_else(|| unreachable!("local coordinates stay inside the region"));
    MapTile::from_region(region, local)
        .unwrap_or_else(|| unreachable!("a loaded region has in-range world coordinates"))
}

fn object_key(placement: &ObjectPlacement) -> (u32, MapPosition, u8, u8) {
    (
        placement.object_id.get(),
        placement.position,
        placement.kind.get(),
        placement.orientation.get(),
    )
}

/// Diff `document` against the stored square. `RegionNotLoaded` when the document holds nothing
/// for the square.
pub fn diff_region(
    terrain: &DecodedTerrain,
    locations: &DecodedLocations,
    document: &MapDocument,
) -> Result<RegionDiff, DiffError> {
    let region = terrain.region();
    let corner = HeightVertexCoord::new(tile_of(region, 0), plane_of(0));
    if document.height_at(corner).is_none() {
        return Err(DiffError::RegionNotLoaded(region));
    }
    let baseline = baseline_document(terrain, locations)?;

    let mut tiles = Vec::new();
    let mut heights = Vec::new();
    for index in 0..SCENE_PLANES * TILES_PER_PLANE {
        let (tile, plane) = (tile_of(region, index), plane_of(index));
        let position = MapPosition::new(tile, plane);
        let after = document.terrain_at(position);
        if after != baseline.terrain_at(position) {
            tiles.push((position, after));
        }
        let vertex = HeightVertexCoord::new(tile, plane);
        if let (Some(before), Some(after)) =
            (baseline.height_at(vertex), document.height_at(vertex))
            && before != after
        {
            heights.push((vertex, after));
        }
    }
    tiles.sort_by_key(|&(position, _)| position);
    heights.sort_by_key(|&(vertex, _)| vertex);

    let mut objects_added: Vec<ObjectPlacement> = document
        .objects_in_region(region)
        .into_iter()
        .filter(|placement| !baseline.contains_object(*placement))
        .collect();
    let mut objects_removed: Vec<ObjectPlacement> = baseline
        .objects_in_region(region)
        .into_iter()
        .filter(|placement| !document.contains_object(*placement))
        .collect();
    objects_added.sort_by_key(object_key);
    objects_removed.sort_by_key(object_key);

    Ok(RegionDiff {
        region,
        tiles,
        heights,
        objects_added,
        objects_removed,
    })
}

#[cfg(test)]
mod tests;
