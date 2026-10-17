use super::*;
use crate::app::edit_runtime::{EditFootprint, EditRuntime};
use crate::session::EditorSession;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{HeightVertexCoord, MapDocument, MapOp, MapPatch};
use osrs_world::LocSpec;

fn plane() -> SourcePlane {
    SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
}

fn coord() -> HeightVertexCoord {
    HeightVertexCoord::new(MapTile::new(3200, 3200), plane())
}

fn spec(object_id: u32, tile: (i32, i32), plane: u8, loc_type: u8, orientation: u8) -> LocSpec {
    LocSpec {
        object_id,
        tile,
        plane,
        loc_type,
        orientation,
    }
}

fn height_patch(coord: HeightVertexCoord, before: i32, after: i32) -> MapPatch {
    MapPatch::new([MapOp::SetHeight {
        coord,
        before,
        after,
    }])
}

fn height_footprint() -> EditFootprint {
    EditFootprint::new([], [coord()], [])
}

fn applied_runtime() -> EditRuntime {
    let coord = coord();
    let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [],
        [(coord, 0)],
        [],
    )));
    runtime
        .commit(height_patch(coord, 0, 64))
        .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
    runtime
}

fn marker() -> EditStep {
    EditStep::semantic_height(vec![(0, 3200, 3200)])
}

#[test]
fn object_history_footprint_reverses_exact_mutation_polarity_for_undo() {
    let source = spec(100, (3200, 3200), 2, 10, 1);
    let moved = spec(100, (3264, 3201), 3, 10, 2);
    let route = Route::Object(ObjectHistoryRoute::new(vec![source], vec![moved]));

    let redo = route.footprint(Direction::Redo);
    assert_eq!(redo.object_removed.len(), 1);
    assert_eq!(redo.object_added.len(), 1);
    assert_eq!(
        redo.object_removed[0].position.tile,
        MapTile::new(3200, 3200)
    );
    assert_eq!(redo.object_removed[0].position.plane.index().get(), 2);
    assert_eq!(redo.object_removed[0].object_id.get(), 100);
    assert_eq!(redo.object_removed[0].kind.get(), 10);
    assert_eq!(redo.object_removed[0].orientation.get(), 1);
    assert_eq!(redo.object_added[0].position.tile, MapTile::new(3264, 3201));
    assert_eq!(redo.object_added[0].position.plane.index().get(), 3);
    assert_eq!(redo.object_added[0].orientation.get(), 2);
    assert_eq!(
        redo.object_tiles,
        vec![MapTile::new(3200, 3200), MapTile::new(3264, 3201)]
    );

    let undo = route.footprint(Direction::Undo);
    assert_eq!(undo.object_removed, redo.object_added);
    assert_eq!(undo.object_added, redo.object_removed);
    assert_eq!(undo.object_tiles, redo.object_tiles);
}

#[test]
fn failed_semantic_undo_projection_restores_runtime_and_history_stack() {
    let mut runtime = applied_runtime();
    let before = runtime.clone();
    let mut history = vec![marker()];
    let mut redo_stack = Vec::new();
    let step = history
        .pop()
        .unwrap_or_else(|| unreachable!("Undo marker exists"));

    let result = runtime.replay(EditReplayDirection::Undo, height_footprint(), |_| {
        Err::<(), _>("injected Undo projection failure".to_owned())
    });
    assert!(matches!(result, Err(EditExecutionError::Projection(_))));
    Direction::Undo.restore(&mut history, &mut redo_stack, step);

    assert_eq!(runtime, before);
    assert_eq!(history.len(), 1);
    assert!(history[0].semantic_height.is_some());
    assert!(redo_stack.is_empty());
}

#[test]
fn failed_semantic_redo_projection_restores_runtime_and_redo_stack() {
    let mut runtime = applied_runtime();
    runtime
        .undo()
        .unwrap_or_else(|error| unreachable!("valid setup Undo: {error:?}"));
    let before = runtime.clone();
    let mut history = Vec::new();
    let mut redo_stack = vec![marker()];
    let step = redo_stack
        .pop()
        .unwrap_or_else(|| unreachable!("Redo marker exists"));

    let result = runtime.replay(EditReplayDirection::Redo, height_footprint(), |_| {
        Err::<(), _>("injected Redo projection failure".to_owned())
    });
    assert!(matches!(result, Err(EditExecutionError::Projection(_))));
    Direction::Redo.restore(&mut history, &mut redo_stack, step);

    assert_eq!(runtime, before);
    assert!(history.is_empty());
    assert_eq!(redo_stack.len(), 1);
    assert!(redo_stack[0].semantic_height.is_some());
}

#[test]
fn successful_semantic_undo_finalizes_once_and_moves_history_marker() {
    let mut runtime = applied_runtime();
    let revision_before = runtime.revision();
    let mut history = vec![marker()];
    let mut redo_stack = Vec::new();
    let step = history
        .pop()
        .unwrap_or_else(|| unreachable!("Undo marker exists"));

    let result = runtime
        .replay(EditReplayDirection::Undo, height_footprint(), |_| {
            Ok::<_, String>(1)
        })
        .unwrap_or_else(|error| unreachable!("valid Undo replay: {error:?}"));
    assert!(matches!(result, EditExecution::Committed { .. }));
    Direction::Undo.settle(&mut history, &mut redo_stack, step);

    assert_eq!(runtime.revision(), revision_before + 1);
    assert_eq!(runtime.document().height_at(coord()), Some(0));
    assert!(runtime.can_redo());
    assert!(history.is_empty());
    assert_eq!(redo_stack.len(), 1);
    assert!(redo_stack[0].semantic_height.is_some());
}

#[test]
fn successful_semantic_redo_finalizes_once_and_moves_redo_marker() {
    let mut runtime = applied_runtime();
    runtime
        .undo()
        .unwrap_or_else(|error| unreachable!("valid setup Undo: {error:?}"));
    let revision_before = runtime.revision();
    let mut history = Vec::new();
    let mut redo_stack = vec![marker()];
    let step = redo_stack
        .pop()
        .unwrap_or_else(|| unreachable!("Redo marker exists"));

    let result = runtime
        .replay(EditReplayDirection::Redo, height_footprint(), |_| {
            Ok::<_, String>(1)
        })
        .unwrap_or_else(|error| unreachable!("valid Redo replay: {error:?}"));
    assert!(matches!(result, EditExecution::Committed { .. }));
    Direction::Redo.settle(&mut history, &mut redo_stack, step);

    assert_eq!(runtime.revision(), revision_before + 1);
    assert_eq!(runtime.document().height_at(coord()), Some(64));
    assert!(runtime.can_undo());
    assert_eq!(history.len(), 1);
    assert!(history[0].semantic_height.is_some());
    assert!(redo_stack.is_empty());
}

// ------------------------------------------------------------------------- composite steps

fn building_patch(placement: osrs_edit::ObjectPlacement) -> MapPatch {
    MapPatch::new([
        MapOp::SetTerrain {
            coord: osrs_edit::MapPosition::new(MapTile::new(3200, 3200), plane()),
            before: osrs_edit::TerrainTile::default(),
            after: osrs_edit::TerrainTile {
                underlay: Some(osrs_core::ids::FloorUnderlayId::new(7)),
                ..osrs_edit::TerrainTile::default()
            },
        },
        MapOp::SetHeight {
            coord: coord(),
            before: 0,
            after: -64,
        },
        MapOp::AddObject { placement },
    ])
}

fn wall() -> osrs_edit::ObjectPlacement {
    osrs_edit::ObjectPlacement {
        object_id: osrs_core::ids::ObjectId::new(1500),
        position: osrs_edit::MapPosition::new(MapTile::new(3200, 3200), plane()),
        kind: osrs_edit::LocKind::new(0).unwrap_or_else(|| unreachable!("valid kind")),
        orientation: osrs_edit::ObjectOrientation::new(1)
            .unwrap_or_else(|| unreachable!("valid orientation")),
    }
}

fn composite_route() -> Route {
    let key = (0, 3200, 3200);
    Route::Composite(
        SemanticPathHistory {
            keys: vec![key],
            include_height: true,
        },
        ObjectHistoryRoute::new(Vec::new(), vec![spec(1500, (3200, 3200), 0, 0, 1)]),
    )
}

#[test]
fn a_step_with_terrain_and_objects_routes_as_one_composite() {
    let step = EditStep::semantic_composite(
        vec![(0, 3200, 3200)],
        true,
        Some(ObjectHistoryRoute::new(
            Vec::new(),
            vec![spec(1, (3200, 3200), 0, 0, 1)],
        )),
    );
    assert!(matches!(Route::of(&step), Some(Route::Composite(..))));
    // Either half alone keeps its own route.
    let terrain_only = EditStep::semantic_composite(vec![(0, 3200, 3200)], true, None);
    assert!(matches!(Route::of(&terrain_only), Some(Route::Path(_))));
    let objects_only = EditStep::semantic_composite(
        Vec::new(),
        false,
        Some(ObjectHistoryRoute::new(
            Vec::new(),
            vec![spec(1, (3200, 3200), 0, 0, 1)],
        )),
    );
    assert!(matches!(Route::of(&objects_only), Some(Route::Object(_))));
}

#[test]
fn composite_footprint_names_tiles_vertices_and_flips_objects_for_undo() {
    let route = composite_route();
    let redo = route.footprint(Direction::Redo);
    assert_eq!(redo.object_added.len(), 1);
    assert!(redo.object_removed.is_empty());
    assert_eq!(redo.terrain_positions.len(), 1);
    assert_eq!(redo.height_vertices.len(), 1);
    let undo = route.footprint(Direction::Undo);
    assert_eq!(undo.object_removed, redo.object_added);
    assert!(undo.object_added.is_empty());
    assert_eq!(undo.height_vertices, redo.height_vertices);
}

#[test]
fn composite_undo_and_redo_replay_terrain_height_and_object_together() {
    let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [],
        [(coord(), 0)],
        [],
    )));
    runtime
        .commit(building_patch(wall()))
        .unwrap_or_else(|error| unreachable!("valid commit: {error:?}"));
    let route = composite_route();
    let key = (0, 3200, 3200);
    let built = crate::paint::TileValues {
        underlay: 7,
        height: -64,
        ..Default::default()
    };

    // Undo: the tile goes back to nothing at height 0 and the wall is removed from the worker.
    let input = ProjectionInput::TilesAndObject(vec![(key, Some(built))]);
    let result = runtime
        .replay(
            EditReplayDirection::Undo,
            route.footprint(Direction::Undo),
            |document| route.prepare_projection(document, Direction::Undo, input),
        )
        .unwrap_or_else(|error| unreachable!("valid undo: {error:?}"));
    let EditExecution::Committed { projection, .. } = result else {
        unreachable!("undo commits");
    };
    let PreparedProjection::Composite {
        projected,
        edits,
        mark_height,
        ..
    } = projection
    else {
        unreachable!("composite route prepares a composite projection");
    };
    assert!(mark_height);
    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0].2.underlay, 0);
    assert_eq!(projected[0].2.height, 0);
    assert_eq!(edits.len(), 1);
    assert!(matches!(edits[0], LocEdit::Remove(_)));
    assert!(!runtime.document().contains_object(wall()));

    // Redo restores all three.
    let input =
        ProjectionInput::TilesAndObject(vec![(key, Some(crate::paint::TileValues::default()))]);
    let result = runtime
        .replay(
            EditReplayDirection::Redo,
            route.footprint(Direction::Redo),
            |document| route.prepare_projection(document, Direction::Redo, input),
        )
        .unwrap_or_else(|error| unreachable!("valid redo: {error:?}"));
    let EditExecution::Committed { projection, .. } = result else {
        unreachable!("redo commits");
    };
    let PreparedProjection::Composite {
        projected, edits, ..
    } = projection
    else {
        unreachable!("composite projection");
    };
    assert_eq!(projected[0].2.underlay, 7);
    assert_eq!(projected[0].2.height, -64);
    assert!(matches!(edits[0], LocEdit::Add(_)));
    assert!(runtime.document().contains_object(wall()));
    assert_eq!(runtime.document().height_at(coord()), Some(-64));
}

#[test]
fn a_missing_tile_fails_the_composite_projection_and_restores_the_session() {
    let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [],
        [(coord(), 0)],
        [],
    )));
    runtime
        .commit(building_patch(wall()))
        .unwrap_or_else(|error| unreachable!("valid commit: {error:?}"));
    let before = runtime.clone();
    let route = composite_route();
    let input = ProjectionInput::TilesAndObject(vec![((0, 3200, 3200), None)]);
    let result = runtime.replay(
        EditReplayDirection::Undo,
        route.footprint(Direction::Undo),
        |document| route.prepare_projection(document, Direction::Undo, input),
    );
    assert!(matches!(result, Err(EditExecutionError::Projection(_))));
    assert_eq!(runtime, before);
}
