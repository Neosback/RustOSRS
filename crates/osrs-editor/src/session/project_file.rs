//! The project file: a versioned, human-readable record of an editing session's changes.
//!
//! A project stores only `RegionDiff`s against the source cache (identified by its profile id,
//! which is bound to the cache fingerprint), never cache bytes or editor history. Decoding
//! validates every value through the semantic constructors, so a hand-edited or corrupted file
//! is rejected instead of loaded into the document.

use super::region_diff::RegionDiff;
use osrs_core::coords::{MapTile, RegionCoord, SourcePlane};
use osrs_core::ids::{FloorOverlayId, FloorUnderlayId, ObjectId};
use osrs_edit::{
    HeightVertexCoord, LocKind, MapPosition, ObjectOrientation, ObjectPlacement, OverlayRotation,
    OverlayShape, TerrainOverlay, TerrainTile,
};
use serde::{Deserialize, Serialize};

pub const PROJECT_FORMAT: &str = "rustosrs-project-v2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectError {
    Json(String),
    UnsupportedFormat(String),
    /// A stored value is outside what the semantic model allows.
    Invalid(&'static str),
}

/// One edited square. Rows are positional arrays to keep files compact and diff-friendly:
/// - `tiles`: `[x, y, plane, underlay, overlay, shape, rotation, flags]` (`0` = none)
/// - `heights`: `[x, y, plane, height]`
/// - objects: `[id, x, y, plane, kind, orientation]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionRecord {
    pub region: [i32; 2],
    pub tiles: Vec<[i32; 8]>,
    pub heights: Vec<[i32; 4]>,
    pub objects_added: Vec<[i32; 6]>,
    pub objects_removed: Vec<[i32; 6]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub format: String,
    /// The bundled cache profile the edits were made against.
    pub profile: String,
    pub regions: Vec<RegionRecord>,
    #[serde(default)]
    pub appearance: osrs_hd::theme::ThemePack,
}

fn plane(value: i32, what: &'static str) -> Result<SourcePlane, ProjectError> {
    u8::try_from(value)
        .ok()
        .and_then(SourcePlane::new)
        .ok_or(ProjectError::Invalid(what))
}

fn byte(value: i32, what: &'static str) -> Result<u8, ProjectError> {
    u8::try_from(value).map_err(|_| ProjectError::Invalid(what))
}

fn id(value: i32, what: &'static str) -> Result<u32, ProjectError> {
    u32::try_from(value).map_err(|_| ProjectError::Invalid(what))
}

fn object_row(placement: ObjectPlacement) -> [i32; 6] {
    [
        placement.object_id.get() as i32,
        placement.position.tile.x,
        placement.position.tile.y,
        i32::from(placement.position.plane.index().get()),
        i32::from(placement.kind.get()),
        i32::from(placement.orientation.get()),
    ]
}

fn object_from(row: [i32; 6]) -> Result<ObjectPlacement, ProjectError> {
    Ok(ObjectPlacement {
        object_id: ObjectId::new(id(row[0], "object id")?),
        position: MapPosition::new(MapTile::new(row[1], row[2]), plane(row[3], "object plane")?),
        kind: LocKind::new(byte(row[4], "object kind")?)
            .ok_or(ProjectError::Invalid("object kind"))?,
        orientation: ObjectOrientation::new(byte(row[5], "object orientation")?)
            .ok_or(ProjectError::Invalid("object orientation"))?,
    })
}

impl ProjectFile {
    pub fn from_diffs(profile: &str, diffs: &[RegionDiff]) -> Self {
        let regions = diffs
            .iter()
            .filter(|diff| !diff.is_empty())
            .map(|diff| RegionRecord {
                region: [diff.region.x, diff.region.y],
                tiles: diff
                    .tiles
                    .iter()
                    .map(|(position, tile)| {
                        [
                            position.tile.x,
                            position.tile.y,
                            i32::from(position.plane.index().get()),
                            tile.underlay.map_or(0, |id| id.get() as i32),
                            tile.overlay.map_or(0, |overlay| overlay.id.get() as i32),
                            tile.overlay
                                .map_or(0, |overlay| i32::from(overlay.shape.get())),
                            tile.overlay
                                .map_or(0, |overlay| i32::from(overlay.rotation.get())),
                            i32::from(tile.flags),
                        ]
                    })
                    .collect(),
                heights: diff
                    .heights
                    .iter()
                    .map(|(vertex, height)| {
                        [
                            vertex.corner.x,
                            vertex.corner.y,
                            i32::from(vertex.plane.index().get()),
                            *height,
                        ]
                    })
                    .collect(),
                objects_added: diff.objects_added.iter().copied().map(object_row).collect(),
                objects_removed: diff
                    .objects_removed
                    .iter()
                    .copied()
                    .map(object_row)
                    .collect(),
            })
            .collect();
        Self {
            format: PROJECT_FORMAT.to_owned(),
            profile: profile.to_owned(),
            regions,
            appearance: Default::default(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self)
            .unwrap_or_else(|error| unreachable!("a project is plain data: {error}"))
    }

    pub fn from_json(text: &str) -> Result<Self, ProjectError> {
        let mut file: Self =
            serde_json::from_str(text).map_err(|error| ProjectError::Json(error.to_string()))?;
        if file.format != PROJECT_FORMAT && file.format != "rustosrs-project-v1" {
            return Err(ProjectError::UnsupportedFormat(file.format));
        }
        super::validate_appearance_pack(&file.appearance).map_err(ProjectError::Json)?;
        file.format = PROJECT_FORMAT.to_owned();
        Ok(file)
    }

    /// Decode and validate every record.
    pub fn into_diffs(self) -> Result<Vec<RegionDiff>, ProjectError> {
        self.regions
            .into_iter()
            .map(|record| {
                let region = RegionCoord::new(record.region[0], record.region[1]);
                let tiles = record
                    .tiles
                    .into_iter()
                    .map(|row| {
                        let position = MapPosition::new(
                            MapTile::new(row[0], row[1]),
                            plane(row[2], "tile plane")?,
                        );
                        let overlay = if row[4] == 0 {
                            None
                        } else {
                            Some(TerrainOverlay {
                                id: FloorOverlayId::new(id(row[4], "overlay id")?),
                                shape: OverlayShape::new(byte(row[5], "overlay shape")?)
                                    .ok_or(ProjectError::Invalid("overlay shape"))?,
                                rotation: OverlayRotation::new(byte(row[6], "overlay rotation")?)
                                    .ok_or(ProjectError::Invalid("overlay rotation"))?,
                            })
                        };
                        let tile = TerrainTile {
                            underlay: (row[3] != 0)
                                .then(|| id(row[3], "underlay id").map(FloorUnderlayId::new))
                                .transpose()?,
                            overlay,
                            flags: byte(row[7], "tile flags")?,
                        };
                        Ok((position, tile))
                    })
                    .collect::<Result<Vec<_>, ProjectError>>()?;
                let heights = record
                    .heights
                    .into_iter()
                    .map(|row| {
                        Ok((
                            HeightVertexCoord::new(
                                MapTile::new(row[0], row[1]),
                                plane(row[2], "height plane")?,
                            ),
                            row[3],
                        ))
                    })
                    .collect::<Result<Vec<_>, ProjectError>>()?;
                Ok(RegionDiff {
                    region,
                    tiles,
                    heights,
                    objects_added: record
                        .objects_added
                        .into_iter()
                        .map(object_from)
                        .collect::<Result<_, _>>()?,
                    objects_removed: record
                        .objects_removed
                        .into_iter()
                        .map(object_from)
                        .collect::<Result<_, _>>()?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
