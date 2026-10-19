use super::*;
use crate::app::edit_runtime::EditRuntime;
use crate::session::EditorSession;
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::{FloorUnderlayId, ObjectId},
};
use osrs_edit::{
    HeightVertexCoord, LocKind, MapDocument, MapOp, MapPatch, MapPosition, ObjectOrientation,
    ObjectPlacement, TerrainTile,
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

fn wall() -> ObjectPlacement {
    ObjectPlacement {
        object_id: ObjectId::new(1500),
        position: position(),
        kind: LocKind::new(0).unwrap_or_else(|| unreachable!("valid loc kind")),
        orientation: ObjectOrientation::new(1)
            .unwrap_or_else(|| unreachable!("valid orientation")),
    }
}

fn building_patch() -> MapPatch {
    MapPatch::new([
        MapOp::SetTerrain {
            coord: position(),
            before: TerrainTile::default(),
            after: TerrainTile {
                underlay: Some(FloorUnderlayId::new(7)),
                ..TerrainTile::default()
            },
        },
        MapOp::SetHeight {
            coord: vertex(),
            before: 0,
            after: -64,
        },
        MapOp::AddObject { placement: wall() },
    ])
}

fn runtime_with_building() -> EditRuntime {
    let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [],
        [(vertex(), 0)],
        [],
    )));
    runtime
        .commit(building_patch())
        .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
    runtime
}

#[test]
fn a_missing_tile_fails_the_composite_projection_and_restores_the_session() {
    let mut runtime = runtime_with_building();
    let before = runtime.clone();
    let descriptor = runtime
        .replay_descriptor(EditReplayDirection::Undo)
        .unwrap_or_else(|| unreachable!("undo descriptor exists"));
    let input = match &descriptor {
        EditReplayDescriptor::Map(footprint) => {
            DescriptorProjectionInput::Map(capture_map_replay_input(footprint, |_| None))
        }
        other => unreachable!("expected map descriptor, got {other:?}"),
    };

    let result = runtime.replay(EditReplayDirection::Undo, |document| {
        prepare_descriptor_projection(document, &descriptor, input)
    });

    assert!(matches!(result, Err(EditExecutionError::Projection(_))));
    assert_eq!(runtime, before);
    assert!(runtime.can_undo());
    assert!(!runtime.can_redo());
}

#[test]
fn composite_undo_and_redo_replay_terrain_height_and_object_together() {
    let mut runtime = runtime_with_building();
    let undo_descriptor = runtime
        .replay_descriptor(EditReplayDirection::Undo)
        .unwrap_or_else(|| unreachable!("undo descriptor exists"));
    let built = crate::paint::TileValues {
        underlay: 7,
        height: -64,
        ..Default::default()
    };
    let undo_input = match &undo_descriptor {
        EditReplayDescriptor::Map(footprint) => DescriptorProjectionInput::Map(
            capture_map_replay_input(footprint, |_| Some(built)),
        ),
        other => unreachable!("expected map descriptor, got {other:?}"),
    };

    let undo = runtime
        .replay(EditReplayDirection::Undo, |document| {
            prepare_descriptor_projection(document, &undo_descriptor, undo_input)
        })
        .unwrap_or_else(|error| unreachable!("valid descriptor undo: {error:?}"));
    let EditExecution::Committed {
        projection: DescriptorProjection::Map(undo_projection),
        ..
    } = undo
    else {
        unreachable!("map undo commits with a map projection")
    };

    assert_eq!(undo_projection.tiles.len(), 1);
    assert_eq!(undo_projection.tiles[0].after.underlay, 0);
    assert_eq!(undo_projection.tiles[0].after.height, 0);
    assert!(undo_projection.tiles[0].height_changed);
    assert_eq!(undo_projection.object_edits.len(), 1);
    assert!(matches!(
        undo_projection.object_edits[0],
        osrs_world::LocEdit::Remove(_)
    ));
    assert!(!runtime.document().contains_object(wall()));
    assert_eq!(runtime.document().height_at(vertex()), Some(0));

    let redo_descriptor = runtime
        .replay_descriptor(EditReplayDirection::Redo)
        .unwrap_or_else(|| unreachable!("redo descriptor exists"));
    let undone = crate::paint::TileValues::default();
    let redo_input = match &redo_descriptor {
        EditReplayDescriptor::Map(footprint) => DescriptorProjectionInput::Map(
            capture_map_replay_input(footprint, |_| Some(undone)),
        ),
        other => unreachable!("expected map descriptor, got {other:?}"),
    };

    let redo = runtime
        .replay(EditReplayDirection::Redo, |document| {
            prepare_descriptor_projection(document, &redo_descriptor, redo_input)
        })
        .unwrap_or_else(|error| unreachable!("valid descriptor redo: {error:?}"));
    let EditExecution::Committed {
        projection: DescriptorProjection::Map(redo_projection),
        ..
    } = redo
    else {
        unreachable!("map redo commits with a map projection")
    };

    assert_eq!(redo_projection.tiles.len(), 1);
    assert_eq!(redo_projection.tiles[0].after.underlay, 7);
    assert_eq!(redo_projection.tiles[0].after.height, -64);
    assert!(redo_projection.tiles[0].height_changed);
    assert_eq!(redo_projection.object_edits.len(), 1);
    assert!(matches!(
        redo_projection.object_edits[0],
        osrs_world::LocEdit::Add(_)
    ));
    assert!(runtime.document().contains_object(wall()));
    assert_eq!(runtime.document().height_at(vertex()), Some(-64));
}
