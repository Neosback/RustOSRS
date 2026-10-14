use super::*;
use crate::app::edit_runtime::{EditExecution, EditExecutionError, EditRuntime};
use crate::session::EditorSession;
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::ObjectId,
};
use osrs_edit::{
    LocKind, MapDocument, MapOp, MapPatch, MapPosition, ObjectOrientation, ObjectPlacement,
};

fn spec(object_id: u32, tile: (i32, i32)) -> LocSpec {
    LocSpec {
        object_id,
        tile,
        plane: 0,
        loc_type: 10,
        orientation: 0,
    }
}

fn placement(spec: LocSpec) -> ObjectPlacement {
    ObjectPlacement {
        object_id: ObjectId::new(spec.object_id),
        position: MapPosition::new(
            MapTile::new(spec.tile.0, spec.tile.1),
            SourcePlane::new(spec.plane).unwrap_or_else(|| unreachable!("valid plane")),
        ),
        kind: LocKind::new(spec.loc_type).unwrap_or_else(|| unreachable!("valid loc kind")),
        orientation: ObjectOrientation::new(spec.orientation)
            .unwrap_or_else(|| unreachable!("valid orientation")),
    }
}

fn runtime_with_redo(source: LocSpec) -> EditRuntime {
    let prior = spec(200, (3199, 3200));
    let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [],
        [],
        [placement(source)],
    )));
    runtime
        .commit(MapPatch::new([MapOp::AddObject {
            placement: placement(prior),
        }]))
        .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
    runtime
        .undo()
        .unwrap_or_else(|error| unreachable!("valid setup undo: {error:?}"));
    assert!(runtime.can_redo());
    runtime
}

#[test]
fn successful_object_gateway_projection_uses_post_commit_state_and_finalizes_once() {
    let source = spec(100, (3200, 3200));
    let moved = spec(100, (3201, 3200));
    let mut runtime = runtime_with_redo(source);
    let revision_before = runtime.revision();
    let plan = build_object_commit_plan(runtime.document(), &[source], &[moved])
        .unwrap_or_else(|error| unreachable!("valid move: {error:?}"));
    let route = ObjectHistoryRoute::new(plan.removed.clone(), plan.added.clone());

    let result = runtime
        .execute(plan.patch, |document| {
            build_object_history_projection(document, &route, true)
        })
        .unwrap_or_else(|error| unreachable!("object gateway succeeds: {error:?}"));

    let EditExecution::Committed { projection, .. } = result else {
        unreachable!("non-empty object move commits")
    };
    assert_eq!(
        projection.edits,
        vec![
            osrs_world::LocEdit::Remove(source),
            osrs_world::LocEdit::Add(moved),
        ]
    );
    assert_eq!(projection.reselect, vec![moved]);
    assert_eq!(runtime.revision(), revision_before + 1);
    assert!(!runtime.document().contains_object(placement(source)));
    assert!(runtime.document().contains_object(placement(moved)));
    assert!(runtime.can_undo());
    assert!(!runtime.can_redo());
}

#[test]
fn failed_object_gateway_projection_restores_exact_pre_transaction_state() {
    let source = spec(100, (3200, 3200));
    let moved = spec(100, (3201, 3200));
    let impossible = spec(100, (3202, 3200));
    let mut runtime = runtime_with_redo(source);
    let before = runtime.clone();
    let plan = build_object_commit_plan(runtime.document(), &[source], &[moved])
        .unwrap_or_else(|error| unreachable!("valid move: {error:?}"));
    let wrong_route = ObjectHistoryRoute::new(plan.removed.clone(), vec![impossible]);

    let result = runtime.execute(plan.patch, |document| {
        build_object_history_projection(document, &wrong_route, true)
    });

    assert!(matches!(result, Err(EditExecutionError::Projection(_))));
    assert_eq!(runtime, before);
    assert!(runtime.can_redo());
}
