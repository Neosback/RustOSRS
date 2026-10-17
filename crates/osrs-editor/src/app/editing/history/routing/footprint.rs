//! The semantic coordinate classes a history route touches, for the edit gateway's replay.

use super::{Direction, Route};
use crate::app::edit_runtime::EditFootprint;
use crate::app::editing::TileKey;
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::ObjectId,
};
use osrs_edit::{HeightVertexCoord, LocKind, MapPosition, ObjectOrientation, ObjectPlacement};
use osrs_world::LocSpec;

impl Route {
    /// Reconstruct the semantic coordinate classes of the opaque session-history patch. Object
    /// replay retains exact placement identity and reports the mutation polarity of the replay that
    /// is about to occur: Undo swaps the original removed/added sets, while Redo preserves them.
    pub(super) fn footprint(&self, direction: Direction) -> EditFootprint {
        let map_tiles = |keys: &[TileKey]| {
            keys.iter()
                .map(|key| MapTile::new(key.1, key.2))
                .collect::<Vec<_>>()
        };
        let height_vertices = |keys: &[TileKey]| {
            keys.iter()
                .map(|key| {
                    let plane = SourcePlane::new(key.0)
                        .unwrap_or_else(|| unreachable!("history TileKey plane is valid"));
                    HeightVertexCoord::new(MapTile::new(key.1, key.2), plane)
                })
                .collect::<Vec<_>>()
        };
        let footprint = match self {
            Self::Scenario | Self::Appearance => EditFootprint::default(),
            Self::Paint(paint) => EditFootprint::new(map_tiles(&paint.keys), [], []),
            Self::Height(height) => EditFootprint::new([], height_vertices(&height.keys), []),
            Self::Path(path) if path.include_height => {
                EditFootprint::new(map_tiles(&path.keys), height_vertices(&path.keys), [])
            }
            Self::Path(path) => EditFootprint::new(map_tiles(&path.keys), [], []),
            Self::Object(object) => {
                let (removed, added) = match direction {
                    Direction::Undo => (object.added(), object.removed()),
                    Direction::Redo => (object.removed(), object.added()),
                };
                EditFootprint::with_object_mutations(
                    [],
                    [],
                    removed.iter().copied().map(history_object_placement),
                    added.iter().copied().map(history_object_placement),
                )
            }
            Self::Composite(path, object) => {
                let (removed, added) = match direction {
                    Direction::Undo => (object.added(), object.removed()),
                    Direction::Redo => (object.removed(), object.added()),
                };
                let vertices = if path.include_height {
                    height_vertices(&path.keys)
                } else {
                    Vec::new()
                };
                EditFootprint::with_object_mutations(
                    map_tiles(&path.keys),
                    vertices,
                    removed.iter().copied().map(history_object_placement),
                    added.iter().copied().map(history_object_placement),
                )
            }
        };
        let keys: &[TileKey] = match self {
            Self::Paint(paint) => &paint.keys,
            Self::Path(path) => &path.keys,
            Self::Composite(path, _) => &path.keys,
            _ => &[],
        };
        footprint.with_terrain_positions(keys.iter().map(|&(plane, x, y)| {
            MapPosition::new(
                MapTile::new(x, y),
                SourcePlane::new(plane).unwrap_or_else(|| unreachable!("validated history plane")),
            )
        }))
    }
}

pub(super) fn history_object_placement(spec: LocSpec) -> ObjectPlacement {
    let plane = SourcePlane::new(spec.plane)
        .unwrap_or_else(|| unreachable!("semantic Object history plane is canonical"));
    let kind = LocKind::new(spec.loc_type)
        .unwrap_or_else(|| unreachable!("semantic Object history loc kind is canonical"));
    let orientation = ObjectOrientation::new(spec.orientation)
        .unwrap_or_else(|| unreachable!("semantic Object history orientation is canonical"));
    ObjectPlacement {
        object_id: ObjectId::new(spec.object_id),
        position: MapPosition::new(MapTile::new(spec.tile.0, spec.tile.1), plane),
        kind,
        orientation,
    }
}
