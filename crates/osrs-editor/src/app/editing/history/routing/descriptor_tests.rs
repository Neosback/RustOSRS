use super::*;
use crate::app::edit_runtime::{EditFootprint, EditRuntime};
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
fn authoritative_descriptor_drives_composite_replay_projection() {
    let mut runtime = runtime_with_building();
    let descriptor = runtime
        .replay_descriptor(EditReplayDirection::Undo)
        .unwrap_or_else(|| unreachable!("undo descriptor exists"));
    let built = crate::paint::TileValues {
        underlay: 7,
        height: -64,
        ..Default::default()
    };
    let input = match &descriptor {
        EditReplayDescriptor::Map(footprint) => DescriptorProjectionInput::Map(
            capture_map_replay_input(footprint, |_| Some(built)),
        ),
        other => unreachable!("expected map descriptor, got {other:?}"),
    };

    let result = runtime
        .replay(EditReplayDirection::Undo, EditFootprint::default(), |document| {
            prepare_descriptor_projection(document, &descriptor, input)
        })
        .unwrap_or_else(|error| unreachable!("valid descriptor replay: {error:?}"));
    let EditExecution::Committed { projection, .. } = result else {
        unreachable!("undo commits");
    };
    let DescriptorProjection::Map(projection) = projection else {
        unreachable!("map history produces a map projection");
    };

    assert_eq!(projection.tiles.len(), 1);
    assert_eq!(projection.tiles[0].after.underlay, 0);
    assert_eq!(projection.tiles[0].after.height, 0);
    assert!(projection.tiles[0].height_changed);
    assert_eq!(projection.object_edits.len(), 1);
    assert!(matches!(projection.object_edits[0], osrs_world::LocEdit::Remove(_)));
    assert!(!runtime.document().contains_object(wall()));
    assert_eq!(runtime.document().height_at(vertex()), Some(0));
}

#[test]
fn descriptor_projection_failure_restores_semantic_replay_exactly() {
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

    let result = runtime.replay(
        EditReplayDirection::Undo,
        EditFootprint::default(),
        |document| prepare_descriptor_projection(document, &descriptor, input),
    );

    assert!(matches!(result, Err(EditExecutionError::Projection(_))));
    assert_eq!(runtime, before);
    assert!(runtime.can_undo());
    assert!(!runtime.can_redo());
}
