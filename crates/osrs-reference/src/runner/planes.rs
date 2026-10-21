//! Adapt normalized link-below fixtures to the existing scene owner.

use super::{FixtureRunError, failure};
use crate::schema::{
    NormalizedExpectedCase, NormalizedPlaneGameObject, NormalizedPlaneTile,
    NormalizedStoredPlaneTile,
};
use osrs_core::coords::{SceneTile, SourcePlane, StoragePlane};
use osrs_core::ids::ObjectId;
use osrs_scene::{SceneGameObject, SceneGrid, SemanticTile};

pub(super) fn execute_plane_link_below(
    fixture_id: &str,
    x: i32,
    y: i32,
    source_tiles: Vec<NormalizedPlaneTile>,
) -> Result<NormalizedExpectedCase, FixtureRunError> {
    let scene_x = u32::try_from(x)
        .map_err(|_| failure(fixture_id, "plane fixture x must be non-negative"))?;
    let scene_y = u32::try_from(y)
        .map_err(|_| failure(fixture_id, "plane fixture y must be non-negative"))?;
    let width = scene_x
        .checked_add(1)
        .ok_or_else(|| failure(fixture_id, "plane fixture width overflow"))?;
    let height = scene_y
        .checked_add(1)
        .ok_or_else(|| failure(fixture_id, "plane fixture height overflow"))?;
    let anchor = SceneTile::new(scene_x, scene_y);
    let mut grid = SceneGrid::new(width, height, 4)
        .map_err(|error| failure(fixture_id, format!("construct scene grid: {error}")))?;
    let mut tile_labels = vec![None::<String>; 4];
    let mut object_labels = Vec::<String>::new();

    for source_tile in source_tiles {
        let NormalizedPlaneTile {
            label,
            plane,
            game_objects,
        } = source_tile;
        let plane_value = normalized_plane_value(fixture_id, plane, "source tile")?;
        let source_plane = SourcePlane::new(plane_value).ok_or_else(|| {
            failure(
                fixture_id,
                format!("source tile plane {plane_value} cannot map to source plane"),
            )
        })?;
        let storage_plane = StoragePlane::new(plane_value).ok_or_else(|| {
            failure(
                fixture_id,
                format!("source tile plane {plane_value} cannot map to storage plane"),
            )
        })?;
        let label_slot = &mut tile_labels[usize::from(plane_value)];
        if label_slot.is_some() {
            return Err(failure(
                fixture_id,
                format!("duplicate source tile plane {plane_value}"),
            ));
        }
        *label_slot = Some(label);

        let mut tile = SemanticTile::new(Some(source_plane), storage_plane);
        for object in game_objects {
            let object_plane = normalized_plane_value(fixture_id, object.plane, "game object")?;
            let start_x = u32::try_from(object.start_x).map_err(|_| {
                failure(
                    fixture_id,
                    "plane fixture game-object start_x must be non-negative",
                )
            })?;
            let start_y = u32::try_from(object.start_y).map_err(|_| {
                failure(
                    fixture_id,
                    "plane fixture game-object start_y must be non-negative",
                )
            })?;
            let object_id = u32::try_from(object_labels.len())
                .map_err(|_| failure(fixture_id, "too many plane fixture game objects"))?;
            let object_storage_plane = StoragePlane::new(object_plane).ok_or_else(|| {
                failure(
                    fixture_id,
                    format!("game-object plane {object_plane} cannot map to storage plane"),
                )
            })?;
            object_labels.push(object.id);
            tile.push_game_object(SceneGameObject::new(
                ObjectId::new(object_id),
                object.tag_type,
                SceneTile::new(start_x, start_y),
                object_storage_plane,
            ));
        }
        grid.set_tile(storage_plane, anchor, tile)
            .map_err(|error| failure(fixture_id, format!("store source tile: {error}")))?;
    }

    grid.set_link_below(anchor)
        .map_err(|error| failure(fixture_id, format!("apply link-below: {error}")))?;

    let mut stored_tiles = Vec::with_capacity(4);
    for plane_value in 0..=3 {
        let storage_plane = StoragePlane::new(plane_value).ok_or_else(|| {
            failure(
                fixture_id,
                format!("output plane {plane_value} cannot map to storage plane"),
            )
        })?;
        let Some(tile) = grid.tile(storage_plane, anchor) else {
            stored_tiles.push(None);
            continue;
        };
        let label = normalized_tile_label(fixture_id, tile, &tile_labels)?;
        let game_objects = tile
            .game_objects()
            .iter()
            .map(|object| normalize_scene_game_object(fixture_id, object, &object_labels))
            .collect::<Result<Vec<_>, _>>()?;
        stored_tiles.push(Some(NormalizedStoredPlaneTile {
            storage_plane: plane_value,
            label,
            plane: i32::from(tile.storage_plane().index().get()),
            game_objects,
        }));
    }

    let plane0 = StoragePlane::new(0)
        .ok_or_else(|| failure(fixture_id, "plane zero cannot map to storage plane"))?;
    let plane3 = StoragePlane::new(3)
        .ok_or_else(|| failure(fixture_id, "plane three cannot map to storage plane"))?;
    let linked_below = grid
        .tile(plane0, anchor)
        .and_then(SemanticTile::linked_below)
        .ok_or_else(|| {
            failure(
                fixture_id,
                "link-below fixture produced no linked-below tile",
            )
        })?;
    let linked_below_label = normalized_tile_label(fixture_id, linked_below, &tile_labels)?;
    let top_slot_cleared = grid.tile(plane3, anchor).is_none();

    Ok(NormalizedExpectedCase::PlaneLinkBelow {
        stored_tiles,
        linked_below_label,
        top_slot_cleared,
    })
}

fn normalized_plane_value(
    fixture_id: &str,
    value: i32,
    subject: &str,
) -> Result<u8, FixtureRunError> {
    let value = u8::try_from(value).map_err(|_| {
        failure(
            fixture_id,
            format!("{subject} plane {value} is outside 0..=3"),
        )
    })?;
    if value > 3 {
        return Err(failure(
            fixture_id,
            format!("{subject} plane {value} is outside 0..=3"),
        ));
    }
    Ok(value)
}

fn normalized_tile_label(
    fixture_id: &str,
    tile: &SemanticTile,
    labels: &[Option<String>],
) -> Result<String, FixtureRunError> {
    let source_plane = tile
        .source_plane()
        .ok_or_else(|| failure(fixture_id, "fixture tile has no source-plane identity"))?;
    labels
        .get(usize::from(source_plane.index().get()))
        .and_then(Option::as_ref)
        .cloned()
        .ok_or_else(|| failure(fixture_id, "fixture tile source label is missing"))
}

fn normalize_scene_game_object(
    fixture_id: &str,
    object: &SceneGameObject,
    labels: &[String],
) -> Result<NormalizedPlaneGameObject, FixtureRunError> {
    let label_index = usize::try_from(object.object_id().get())
        .map_err(|_| failure(fixture_id, "fixture object ID cannot index label table"))?;
    let id = labels
        .get(label_index)
        .cloned()
        .ok_or_else(|| failure(fixture_id, "fixture object label is missing"))?;
    let start = object.start();
    let start_x = i32::try_from(start.x)
        .map_err(|_| failure(fixture_id, "fixture object start_x exceeds i32"))?;
    let start_y = i32::try_from(start.y)
        .map_err(|_| failure(fixture_id, "fixture object start_y exceeds i32"))?;
    Ok(NormalizedPlaneGameObject {
        id,
        tag_type: object.tag_type(),
        start_x,
        start_y,
        plane: i32::from(object.plane().index().get()),
    })
}
