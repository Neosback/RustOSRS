#![cfg_attr(not(test), allow(dead_code))]

use super::super::TileKey;
use crate::app::object_tool::spec_from_placement;
use crate::paint::TileValues;
use osrs_edit::{MapQuery, MapReplayFootprint, TerrainChannels};
use osrs_world::{LocEdit, LocSpec};
use std::collections::{BTreeMap, BTreeSet};

/// Transitional preview values captured before a semantic map replay begins.
///
/// Missing tiles stay explicit so preparation fails inside the checkpointed gateway and can trigger
/// exact semantic rollback instead of mutating worker state partially.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MapReplayInput {
    tiles: Vec<(TileKey, Option<TileValues>)>,
}

/// One fully validated tile projection prepared from authoritative post-replay semantic state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PreparedTileReplay {
    pub key: TileKey,
    pub before: TileValues,
    pub after: TileValues,
    pub height_changed: bool,
}

/// Fully validated map replay projection. Applying this after gateway finalization is mechanical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedMapReplay {
    pub tiles: Vec<PreparedTileReplay>,
    pub object_edits: Vec<LocEdit>,
    pub reselect: Vec<LocSpec>,
    object_count: usize,
}

impl PreparedMapReplay {
    pub(super) fn count(&self) -> usize {
        self.tiles.len() + self.object_count
    }
}

/// Capture only the preview tiles named by the authoritative semantic replay footprint.
pub(super) fn capture_map_replay_input(
    footprint: &MapReplayFootprint,
    mut current_tile: impl FnMut(TileKey) -> Option<TileValues>,
) -> MapReplayInput {
    let mut keys = BTreeSet::new();
    keys.extend(footprint.terrain().iter().map(|change| {
        (
            change.coord.plane.index().get(),
            change.coord.tile.x,
            change.coord.tile.y,
        )
    }));
    keys.extend(footprint.heights().iter().map(|coord| {
        (
            coord.plane.index().get(),
            coord.corner.x,
            coord.corner.y,
        )
    }));
    MapReplayInput {
        tiles: keys
            .into_iter()
            .map(|key| (key, current_tile(key)))
            .collect(),
    }
}

/// Prepare a generic map replay projection from exact semantic channels.
///
/// Only channels named by `footprint` are copied from semantic state. Untouched preview channels are
/// preserved from the captured pre-replay value. Object edits are validated against post-replay
/// semantic presence before any worker mutation is returned.
pub(super) fn prepare_map_replay(
    query: &impl MapQuery,
    footprint: &MapReplayFootprint,
    input: MapReplayInput,
) -> Result<PreparedMapReplay, String> {
    let terrain_channels = footprint
        .terrain()
        .iter()
        .map(|change| {
            (
                (
                    change.coord.plane.index().get(),
                    change.coord.tile.x,
                    change.coord.tile.y,
                ),
                change.channels,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let height_keys = footprint
        .heights()
        .iter()
        .map(|coord| {
            (
                coord.plane.index().get(),
                coord.corner.x,
                coord.corner.y,
            )
        })
        .collect::<BTreeSet<_>>();

    let mut tiles = Vec::with_capacity(input.tiles.len());
    for (key, before) in input.tiles {
        let before = before.ok_or_else(|| {
            format!(
                "semantic replay tile ({}, {}, plane {}) is unavailable to the preview adapter",
                key.1, key.2, key.0
            )
        })?;
        let plane = osrs_core::coords::SourcePlane::new(key.0)
            .ok_or_else(|| format!("semantic replay has invalid source plane {}", key.0))?;
        let tile = osrs_core::coords::MapTile::new(key.1, key.2);
        let position = osrs_edit::MapPosition::new(tile, plane);
        let mut after = before;

        if let Some(channels) = terrain_channels.get(&key).copied() {
            let terrain = query.terrain_at(position);
            if channels.contains(TerrainChannels::UNDERLAY) {
                after.underlay = match terrain.underlay {
                    Some(id) => u16::try_from(id.get()).map_err(|_| {
                        format!("underlay id {} does not fit the preview adapter", id.get())
                    })?,
                    None => 0,
                };
            }
            if channels.contains(TerrainChannels::OVERLAY) {
                match terrain.overlay {
                    Some(overlay) => {
                        after.overlay = u16::try_from(overlay.id.get()).map_err(|_| {
                            format!("overlay id {} does not fit the preview adapter", overlay.id.get())
                        })?;
                        after.shape = overlay.shape.get();
                        after.rotation = overlay.rotation.get();
                    }
                    None => {
                        after.overlay = 0;
                        after.shape = 0;
                        after.rotation = 0;
                    }
                }
            }
            if channels.contains(TerrainChannels::FLAGS) {
                after.flags = terrain.flags;
            }
        }

        let height_changed = height_keys.contains(&key);
        if height_changed {
            let coord = osrs_edit::HeightVertexCoord::new(tile, plane);
            after.height = query.height_at(coord).ok_or_else(|| {
                format!(
                    "semantic replay height ({}, {}, plane {}) is missing",
                    key.1, key.2, key.0
                )
            })?;
        }

        tiles.push(PreparedTileReplay {
            key,
            before,
            after,
            height_changed,
        });
    }

    for &placement in footprint.removed_objects() {
        if query.contains_object(placement) {
            return Err(format!(
                "semantic replay expected removed object {} to be absent at ({}, {})",
                placement.object_id.get(),
                placement.position.tile.x,
                placement.position.tile.y
            ));
        }
    }
    for &placement in footprint.added_objects() {
        if !query.contains_object(placement) {
            return Err(format!(
                "semantic replay expected added object {} to be present at ({}, {})",
                placement.object_id.get(),
                placement.position.tile.x,
                placement.position.tile.y
            ));
        }
    }

    let removed = footprint
        .removed_objects()
        .iter()
        .copied()
        .map(spec_from_placement)
        .collect::<Vec<_>>();
    let added = footprint
        .added_objects()
        .iter()
        .copied()
        .map(spec_from_placement)
        .collect::<Vec<_>>();
    let mut object_edits = removed.iter().copied().map(LocEdit::Remove).collect::<Vec<_>>();
    object_edits.extend(added.iter().copied().map(LocEdit::Add));
    let object_count = removed.len().max(added.len());

    Ok(PreparedMapReplay {
        tiles,
        object_edits,
        reselect: added,
        object_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_core::{
        coords::{MapTile, SourcePlane},
        ids::{FloorOverlayId, FloorUnderlayId, ObjectId},
    };
    use osrs_edit::{
        HeightVertexCoord, LocKind, MapDocument, MapOp, MapPatch, MapPosition, ObjectOrientation,
        ObjectPlacement, OverlayRotation, OverlayShape, TerrainOverlay, TerrainTile,
    };

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn position() -> MapPosition {
        MapPosition::new(MapTile::new(3200, 3200), plane())
    }

    fn vertex() -> HeightVertexCoord {
        HeightVertexCoord::new(MapTile::new(3200, 3200), plane())
    }

    fn overlay(id: u32) -> TerrainOverlay {
        TerrainOverlay {
            id: FloorOverlayId::new(id),
            shape: OverlayShape::new(2).unwrap_or_else(|| unreachable!("shape is valid")),
            rotation: OverlayRotation::new(3)
                .unwrap_or_else(|| unreachable!("rotation is valid")),
        }
    }

    fn object() -> ObjectPlacement {
        ObjectPlacement {
            object_id: ObjectId::new(100),
            position: position(),
            kind: LocKind::new(10).unwrap_or_else(|| unreachable!("kind is valid")),
            orientation: ObjectOrientation::new(1)
                .unwrap_or_else(|| unreachable!("orientation is valid")),
        }
    }

    #[test]
    fn terrain_projection_updates_only_named_channels() {
        let before_terrain = TerrainTile::default();
        let after_terrain = TerrainTile {
            underlay: Some(FloorUnderlayId::new(7)),
            flags: 4,
            ..TerrainTile::default()
        };
        let patch = MapPatch::new([MapOp::SetTerrain {
            coord: position(),
            before: before_terrain,
            after: after_terrain,
        }]);
        let footprint = patch.replay_footprint();
        let document = MapDocument::from_parts([(position(), after_terrain)], [], []);
        let current = TileValues {
            overlay: 55,
            shape: 9,
            rotation: 1,
            height: -128,
            ..TileValues::default()
        };
        let input = capture_map_replay_input(&footprint, |_| Some(current));

        let prepared = prepare_map_replay(&document, &footprint, input)
            .unwrap_or_else(|error| unreachable!("valid projection: {error}"));
        let tile = prepared.tiles[0];
        assert_eq!(tile.after.underlay, 7);
        assert_eq!(tile.after.flags, 4);
        assert_eq!(tile.after.overlay, 55);
        assert_eq!(tile.after.shape, 9);
        assert_eq!(tile.after.rotation, 1);
        assert_eq!(tile.after.height, -128);
        assert!(!tile.height_changed);
    }

    #[test]
    fn overlay_and_height_projection_are_independent_channels() {
        let terrain_before = TerrainTile::default();
        let terrain_after = TerrainTile {
            overlay: Some(overlay(9)),
            ..TerrainTile::default()
        };
        let patch = MapPatch::new([
            MapOp::SetTerrain {
                coord: position(),
                before: terrain_before,
                after: terrain_after,
            },
            MapOp::SetHeight {
                coord: vertex(),
                before: 0,
                after: -64,
            },
        ]);
        let footprint = patch.replay_footprint();
        let document = MapDocument::from_parts(
            [(position(), terrain_after)],
            [(vertex(), -64)],
            [],
        );
        let current = TileValues {
            underlay: 12,
            height: 0,
            ..TileValues::default()
        };
        let input = capture_map_replay_input(&footprint, |_| Some(current));

        let prepared = prepare_map_replay(&document, &footprint, input)
            .unwrap_or_else(|error| unreachable!("valid projection: {error}"));
        let tile = prepared.tiles[0];
        assert_eq!(tile.after.underlay, 12);
        assert_eq!(tile.after.overlay, 9);
        assert_eq!(tile.after.shape, 2);
        assert_eq!(tile.after.rotation, 3);
        assert_eq!(tile.after.height, -64);
        assert!(tile.height_changed);
    }

    #[test]
    fn object_projection_comes_directly_from_semantic_mutation_polarity() {
        let placement = object();
        let patch = MapPatch::new([MapOp::AddObject { placement }]);
        let footprint = patch.replay_footprint();
        let document = MapDocument::from_parts([], [], [placement]);
        let input = capture_map_replay_input(&footprint, |_| None);

        let prepared = prepare_map_replay(&document, &footprint, input)
            .unwrap_or_else(|error| unreachable!("valid projection: {error}"));
        assert!(prepared.tiles.is_empty());
        assert_eq!(prepared.object_edits, vec![LocEdit::Add(spec_from_placement(placement))]);
        assert_eq!(prepared.reselect, vec![spec_from_placement(placement)]);
        assert_eq!(prepared.count(), 1);
    }

    #[test]
    fn missing_preview_tile_fails_before_any_projection_is_applied() {
        let patch = MapPatch::new([MapOp::SetHeight {
            coord: vertex(),
            before: 0,
            after: 64,
        }]);
        let footprint = patch.replay_footprint();
        let document = MapDocument::from_parts([], [(vertex(), 64)], []);
        let input = capture_map_replay_input(&footprint, |_| None);

        assert!(prepare_map_replay(&document, &footprint, input).is_err());
    }
}
