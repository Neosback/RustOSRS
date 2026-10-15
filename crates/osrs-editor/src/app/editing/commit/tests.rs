use super::*;
use crate::app::edit_runtime::{EditExecution, EditExecutionError, EditRuntime};
use crate::session::EditorSession;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_core::ids::FloorOverlayId;
use osrs_edit::{HeightVertexCoord, MapDocument, MapOp, MapPatch, MapPosition};

fn plane() -> SourcePlane {
    SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
}

fn vertex(tile: (i32, i32)) -> HeightVertexCoord {
    HeightVertexCoord::new(MapTile::new(tile.0, tile.1), plane())
}

fn position(tile: (i32, i32)) -> MapPosition {
    MapPosition::new(MapTile::new(tile.0, tile.1), plane())
}

fn height_patch(coord: HeightVertexCoord, before: i32, after: i32) -> MapPatch {
    MapPatch::new([MapOp::SetHeight {
        coord,
        before,
        after,
    }])
}

fn runtime_with_redo() -> (EditRuntime, HeightVertexCoord) {
    let prior = vertex((3200, 3200));
    let edited = vertex((3264, 3200));
    let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [],
        [(prior, 0), (edited, 0)],
        [],
    )));
    runtime
        .commit(height_patch(prior, 0, 32))
        .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
    runtime
        .undo()
        .unwrap_or_else(|error| unreachable!("valid setup undo: {error:?}"));
    assert!(runtime.can_redo());
    (runtime, edited)
}

fn paint_patch(runtime: &EditRuntime, tile: (i32, i32), brush: &Brush) -> MapPatch {
    let before = TileValues::default();
    let after = brush.apply(before);
    build_paint_stroke_patch(
        runtime.document(),
        brush,
        &[((0, tile.0, tile.1), before, after)],
    )
    .unwrap_or_else(|error| unreachable!("valid Paint patch: {error:?}"))
}

#[test]
fn failed_paint_projection_restores_exact_pre_stroke_session() {
    let (mut runtime, edited) = runtime_with_redo();
    let before = runtime.clone();
    let tile = (edited.corner.x, edited.corner.y);
    let brush = Brush::default();
    let patch = paint_patch(&runtime, tile, &brush);
    let invalid_key = (u8::MAX, tile.0, tile.1);
    let result = runtime.execute(patch, |document| {
        assert_eq!(
            document
                .terrain_at(position(tile))
                .overlay
                .map(|overlay| overlay.id),
            Some(FloorOverlayId::new(1))
        );
        plan_paint_projection(document, &brush, &[(0, invalid_key, TileValues::default())])
    });

    assert_eq!(
        result,
        Err(EditExecutionError::Projection(
            "a Paint preview tile could not be projected from semantic state"
        ))
    );
    assert_eq!(runtime, before);
}

#[test]
fn successful_paint_projection_uses_post_commit_state_and_finalizes_once() {
    let (mut runtime, edited) = runtime_with_redo();
    let revision_before = runtime.revision();
    let tile = (edited.corner.x, edited.corner.y);
    let key = (0, tile.0, tile.1);
    let brush = Brush::default();
    let patch = paint_patch(&runtime, tile, &brush);
    let result = runtime
        .execute(patch, |document| {
            assert_eq!(
                document
                    .terrain_at(position(tile))
                    .overlay
                    .map(|overlay| overlay.id),
                Some(FloorOverlayId::new(1))
            );
            plan_paint_projection(document, &brush, &[(0, key, TileValues::default())])
        })
        .unwrap_or_else(|error| unreachable!("Paint gateway execution succeeds: {error:?}"));

    let EditExecution::Committed { projection, .. } = result else {
        unreachable!("non-empty Paint patch commits")
    };
    assert_eq!(
        projection,
        vec![(
            0,
            key,
            TileValues {
                overlay: 1,
                ..TileValues::default()
            }
        )]
    );
    assert_eq!(runtime.revision(), revision_before + 1);
    assert_eq!(
        runtime
            .document()
            .terrain_at(position(tile))
            .overlay
            .map(|overlay| overlay.id),
        Some(FloorOverlayId::new(1))
    );
    assert!(runtime.can_undo());
    assert!(!runtime.can_redo());
}

#[test]
fn failed_height_projection_restores_exact_pre_stroke_session() {
    let (mut runtime, edited) = runtime_with_redo();
    let before = runtime.clone();
    let missing_key = (0, edited.corner.x + 1, edited.corner.y);
    let current = TileValues::default();
    let result = runtime.execute(height_patch(edited, 0, 64), |document| {
        assert_eq!(document.height_at(edited), Some(64));
        plan_height_projection(document, &[(0, missing_key, current)])
    });

    assert_eq!(
        result,
        Err(EditExecutionError::Projection(
            "a Height preview vertex is missing from semantic state"
        ))
    );
    assert_eq!(runtime, before);
}

#[test]
fn successful_height_projection_uses_post_commit_state_and_finalizes_once() {
    let (mut runtime, edited) = runtime_with_redo();
    let revision_before = runtime.revision();
    let key = (0, edited.corner.x, edited.corner.y);
    let current = TileValues::default();
    let result = runtime
        .execute(height_patch(edited, 0, 64), |document| {
            assert_eq!(document.height_at(edited), Some(64));
            plan_height_projection(document, &[(0, key, current)])
        })
        .unwrap_or_else(|error| unreachable!("Height gateway execution succeeds: {error:?}"));

    let EditExecution::Committed { projection, .. } = result else {
        unreachable!("non-empty Height patch commits")
    };
    assert_eq!(
        projection,
        vec![(
            0,
            key,
            TileValues {
                height: 64,
                ..current
            }
        )]
    );
    assert_eq!(runtime.revision(), revision_before + 1);
    assert_eq!(runtime.document().height_at(edited), Some(64));
    assert!(runtime.can_undo());
    assert!(!runtime.can_redo());
}
