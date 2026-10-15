use super::*;
use crate::app::edit_runtime::{EditFootprint, EditRuntime};
use crate::session::EditorSession;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{HeightVertexCoord, MapDocument, MapOp, MapPatch};

fn plane() -> SourcePlane {
    SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
}

fn coord() -> HeightVertexCoord {
    HeightVertexCoord::new(MapTile::new(3200, 3200), plane())
}

fn height_patch(coord: HeightVertexCoord, before: i32, after: i32) -> MapPatch {
    MapPatch::new([MapOp::SetHeight {
        coord,
        before,
        after,
    }])
}

fn height_footprint() -> EditFootprint {
    EditFootprint::new([], [MapTile::new(3200, 3200)], [])
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
