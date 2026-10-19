use super::plan::build_paint_stroke_patch;
use super::*;
use crate::heights::{Falloff, HeightOp, HeightTool};
use crate::paint::Fill;
use crate::session::EditorSession;
use osrs_core::ids::{FloorOverlayId, FloorUnderlayId};
use osrs_edit::{MapDocument, TerrainOverlay, TerrainTile};
use osrs_edit::{MapPosition, OverlayRotation, OverlayShape};

fn semantic_terrain(values: TileValues) -> Option<TerrainTile> {
    let underlay = (values.underlay != 0).then(|| FloorUnderlayId::new(u32::from(values.underlay)));
    let overlay = if values.overlay == 0 {
        None
    } else {
        Some(TerrainOverlay {
            id: FloorOverlayId::new(u32::from(values.overlay)),
            shape: OverlayShape::new(values.shape)?,
            rotation: OverlayRotation::new(values.rotation)?,
        })
    };
    Some(TerrainTile {
        underlay,
        overlay,
        flags: values.flags,
    })
}

fn plane() -> SourcePlane {
    SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
}

fn position(tile: (i32, i32)) -> MapPosition {
    MapPosition::new(MapTile::new(tile.0, tile.1), plane())
}

fn vertex(tile: (i32, i32)) -> HeightVertexCoord {
    HeightVertexCoord::new(MapTile::new(tile.0, tile.1), plane())
}

fn height_spec() -> HeightStrokeSpec {
    HeightStrokeSpec::new(
        plane(),
        HeightTool {
            op: HeightOp::Raise,
            strength: 16,
            falloff: Falloff::Flat,
            flatten_to: None,
        },
        0,
        crate::paint::BrushShape::Square,
        0,
        7,
    )
}

#[test]
fn an_interior_tile_rebuilds_only_its_own_region() {
    assert_eq!(affected_regions((3222, 3218)), vec![(50, 50)]);
}

#[test]
fn a_tile_near_a_region_corner_rebuilds_the_neighbours_too() {
    let regions = affected_regions((3200, 3200));
    assert_eq!(regions.len(), 4);
    assert!(regions.contains(&(49, 49)) && regions.contains(&(50, 50)));
    assert_eq!(affected_regions((3263, 3230)).len(), 2);
}

#[test]
fn one_paint_stroke_is_one_semantic_session_history_entry() {
    let brush = Brush::default();
    let before = TileValues::default();
    let after = brush.apply(before);
    let changes = vec![
        ((0, 3200, 3200), before, after),
        ((1, 3200, 3200), before, after),
    ];
    let mut session = EditorSession::new(MapDocument::new());
    let patch = build_paint_stroke_patch(session.document(), &brush, &changes)
        .unwrap_or_else(|error| unreachable!("valid Paint stroke: {error:?}"));

    assert_eq!(patch.len(), 2);
    assert!(
        session
            .commit(patch)
            .unwrap_or_else(|error| unreachable!("valid semantic commit: {error:?}"))
            .is_some()
    );
    assert_eq!(session.revision(), 1);
    assert!(session.can_undo());

    for plane in [0, 1] {
        let source_plane =
            SourcePlane::new(plane).unwrap_or_else(|| unreachable!("test plane is valid"));
        let position = MapPosition::new(MapTile::new(3200, 3200), source_plane);
        assert_eq!(
            session
                .document()
                .terrain_at(position)
                .overlay
                .map(|overlay| overlay.id),
            Some(FloorOverlayId::new(1))
        );
    }

    assert!(
        session
            .undo()
            .unwrap_or_else(|error| unreachable!("valid semantic undo: {error:?}"))
            .is_some()
    );
    assert_eq!(session.revision(), 2);
    for plane in [0, 1] {
        let source_plane =
            SourcePlane::new(plane).unwrap_or_else(|| unreachable!("test plane is valid"));
        let position = MapPosition::new(MapTile::new(3200, 3200), source_plane);
        assert_eq!(
            session.document().terrain_at(position),
            TerrainTile::default()
        );
    }
}

#[test]
fn one_height_stroke_is_one_semantic_session_history_entry() {
    let tile = (3200, 3200);
    let map_tile = MapTile::new(tile.0, tile.1);
    let document = MapDocument::from_parts([], [(vertex(tile), 0)], []);
    let mut session = EditorSession::new(document);
    let patch =
        build_terrain_height_patch(session.document(), &height_spec(), &[map_tile, map_tile])
            .unwrap_or_else(|error| unreachable!("valid Height stroke: {error:?}"));

    assert_eq!(patch.len(), 1);
    assert!(
        session
            .commit(patch)
            .unwrap_or_else(|error| unreachable!("valid semantic commit: {error:?}"))
            .is_some()
    );
    assert_eq!(session.revision(), 1);
    assert!(session.can_undo());
    assert_eq!(session.document().height_at(vertex(tile)), Some(-32));
}

#[test]
fn semantic_height_projection_tracks_undo_redo_and_preserves_other_channels() {
    let tile = (3200, 3200);
    let map_tile = MapTile::new(tile.0, tile.1);
    let current = TileValues {
        underlay: 4,
        overlay: 7,
        shape: 3,
        rotation: 2,
        height: 0,
        flags: 1,
    };
    let document = MapDocument::from_parts(
        [(
            position(tile),
            semantic_terrain(current).unwrap_or_default(),
        )],
        [(vertex(tile), current.height)],
        [],
    );
    let mut session = EditorSession::new(document);
    let patch = build_terrain_height_patch(session.document(), &height_spec(), &[map_tile])
        .unwrap_or_else(|error| unreachable!("valid Height patch: {error:?}"));
    assert!(session.commit(patch).is_ok());

    let committed = project_height_values(session.document(), (0, tile.0, tile.1), current)
        .unwrap_or_else(|| unreachable!("semantic Height state is projectable"));
    assert_eq!(committed.height, -16);
    assert_eq!(
        TileValues {
            height: current.height,
            ..committed
        },
        current
    );

    assert!(session.undo().is_ok());
    let undone = project_height_values(session.document(), (0, tile.0, tile.1), committed)
        .unwrap_or_else(|| unreachable!("semantic Height undo is projectable"));
    assert_eq!(undone, current);

    assert!(session.redo().is_ok());
    let redone = project_height_values(session.document(), (0, tile.0, tile.1), undone)
        .unwrap_or_else(|| unreachable!("semantic Height redo is projectable"));
    assert_eq!(redone, committed);
}

#[test]
fn paint_and_height_share_one_semantic_history_order() {
    let tile = (3200, 3200);
    let map_tile = MapTile::new(tile.0, tile.1);
    let before = TileValues::default();
    let brush = Brush::default();
    let painted = brush.apply(before);
    let document = MapDocument::from_parts(
        [(position(tile), semantic_terrain(before).unwrap_or_default())],
        [(vertex(tile), 0)],
        [],
    );
    let mut session = EditorSession::new(document);

    let paint_patch = build_paint_stroke_patch(
        session.document(),
        &brush,
        &[((0, tile.0, tile.1), before, painted)],
    )
    .unwrap_or_else(|error| unreachable!("valid Paint patch: {error:?}"));
    assert!(session.commit(paint_patch).is_ok());

    let height_patch = build_terrain_height_patch(session.document(), &height_spec(), &[map_tile])
        .unwrap_or_else(|error| unreachable!("valid Height patch: {error:?}"));
    assert!(session.commit(height_patch).is_ok());
    assert_eq!(session.document().height_at(vertex(tile)), Some(-16));
    assert_eq!(
        session
            .document()
            .terrain_at(position(tile))
            .overlay
            .map(|overlay| overlay.id),
        Some(FloorOverlayId::new(1))
    );

    assert!(session.undo().is_ok());
    assert_eq!(session.document().height_at(vertex(tile)), Some(0));
    assert_eq!(
        session
            .document()
            .terrain_at(position(tile))
            .overlay
            .map(|overlay| overlay.id),
        Some(FloorOverlayId::new(1))
    );

    assert!(session.undo().is_ok());
    assert_eq!(
        session.document().terrain_at(position(tile)),
        TerrainTile::default()
    );
    assert!(session.redo().is_ok());
    assert_eq!(
        session
            .document()
            .terrain_at(position(tile))
            .overlay
            .map(|overlay| overlay.id),
        Some(FloorOverlayId::new(1))
    );
    assert!(session.redo().is_ok());
    assert_eq!(session.document().height_at(vertex(tile)), Some(-16));
}

#[test]
fn semantic_paint_projection_tracks_undo_redo_and_preserves_unarmed_height() {
    let tile = (3200, 3200);
    let before = TileValues {
        underlay: 4,
        overlay: 7,
        shape: 3,
        rotation: 2,
        height: -96,
        flags: 1,
    };
    let brush = Brush::default();
    let after = brush.apply(before);
    let document = MapDocument::from_parts(
        [(position(tile), semantic_terrain(before).unwrap_or_default())],
        [(vertex(tile), before.height)],
        [],
    );
    let mut session = EditorSession::new(document);
    let patch = build_paint_stroke_patch(
        session.document(),
        &brush,
        &[((0, tile.0, tile.1), before, after)],
    )
    .unwrap_or_else(|error| unreachable!("valid Paint patch: {error:?}"));
    assert!(session.commit(patch).is_ok());

    let committed = project_paint_values(session.document(), &brush, (0, tile.0, tile.1), before)
        .unwrap_or_else(|| unreachable!("semantic Paint state is projectable"));
    assert_eq!(committed.overlay, 1);
    assert_eq!(committed.height, before.height);

    assert!(session.undo().is_ok());
    let undone = project_paint_values(session.document(), &brush, (0, tile.0, tile.1), committed)
        .unwrap_or_else(|| unreachable!("semantic undo is projectable"));
    assert_eq!(undone, before);

    assert!(session.redo().is_ok());
    let redone = project_paint_values(session.document(), &brush, (0, tile.0, tile.1), undone)
        .unwrap_or_else(|| unreachable!("semantic redo is projectable"));
    assert_eq!(redone, committed);
}

#[test]
fn semantic_paint_no_op_creates_no_history() {
    let tile = (3200, 3200);
    let brush = Brush::default();
    let before = TileValues {
        overlay: 1,
        ..TileValues::default()
    };
    let after = brush.apply(before);
    assert_eq!(after, before);
    let document = MapDocument::from_parts(
        [(position(tile), semantic_terrain(before).unwrap_or_default())],
        [(vertex(tile), before.height)],
        [],
    );
    let mut session = EditorSession::new(document);
    let patch = build_paint_stroke_patch(
        session.document(),
        &brush,
        &[((0, tile.0, tile.1), before, after)],
    )
    .unwrap_or_else(|error| unreachable!("valid no-op Paint patch: {error:?}"));

    assert!(patch.is_empty());
    assert_eq!(session.commit(patch), Ok(None));
    assert_eq!(session.revision(), 0);
    assert!(!session.can_undo());
    assert!(!session.can_redo());
}

#[test]
fn new_semantic_paint_commit_invalidates_semantic_redo() {
    let tile = (3200, 3200);
    let before = TileValues::default();
    let first_brush = Brush::default();
    let first_after = first_brush.apply(before);
    let mut session = EditorSession::new(MapDocument::new());
    let first = build_paint_stroke_patch(
        session.document(),
        &first_brush,
        &[((0, tile.0, tile.1), before, first_after)],
    )
    .unwrap_or_else(|error| unreachable!("valid first Paint patch: {error:?}"));
    assert!(session.commit(first).is_ok());
    assert!(session.undo().is_ok());
    assert!(session.can_redo());

    let mut second_brush = Brush::default();
    second_brush.overlay.fill = Fill::Set(2);
    let second_after = second_brush.apply(before);
    let second = build_paint_stroke_patch(
        session.document(),
        &second_brush,
        &[((0, tile.0, tile.1), before, second_after)],
    )
    .unwrap_or_else(|error| unreachable!("valid replacement Paint patch: {error:?}"));
    assert!(session.commit(second).is_ok());

    assert!(!session.can_redo());
    assert_eq!(
        session
            .document()
            .terrain_at(position(tile))
            .overlay
            .map(|overlay| overlay.id),
        Some(FloorOverlayId::new(2))
    );
}

#[test]
fn clearing_an_overlay_is_one_semantic_session_history_entry() {
    let tile = (3200, 3200);
    let painted = TerrainTile {
        underlay: Some(FloorUnderlayId::new(3)),
        overlay: Some(TerrainOverlay {
            id: FloorOverlayId::new(5),
            shape: OverlayShape::new(2).unwrap_or_else(|| unreachable!("valid shape")),
            rotation: OverlayRotation::new(1).unwrap_or_else(|| unreachable!("valid rotation")),
        }),
        flags: 1,
    };
    let mut session =
        EditorSession::new(MapDocument::from_parts([(position(tile), painted)], [], []));
    let brush = overlay_clear_brush();
    let before = TileValues {
        underlay: 3,
        overlay: 5,
        shape: 2,
        rotation: 1,
        flags: 1,
        ..TileValues::default()
    };
    let after = brush.apply(before);
    assert_eq!((after.overlay, after.shape, after.rotation), (0, 0, 0));

    let patch = build_paint_stroke_patch(
        session.document(),
        &brush,
        &[((0, tile.0, tile.1), before, after)],
    )
    .unwrap_or_else(|error| unreachable!("valid clear stroke: {error:?}"));
    assert_eq!(patch.len(), 1);
    session
        .commit(patch)
        .unwrap_or_else(|error| unreachable!("valid commit: {error:?}"));

    let cleared = session.document().terrain_at(position(tile));
    assert_eq!(cleared.overlay, None);
    assert_eq!(
        cleared.underlay, painted.underlay,
        "unarmed channels are untouched"
    );
    assert_eq!(cleared.flags, painted.flags);
    assert_eq!(session.revision(), 1);

    session
        .undo()
        .unwrap_or_else(|error| unreachable!("valid undo: {error:?}"));
    assert_eq!(session.document().terrain_at(position(tile)), painted);
}

#[test]
fn clearing_an_empty_overlay_makes_no_semantic_history() {
    let tile = (3200, 3200);
    let mut session = EditorSession::new(MapDocument::new());
    let brush = overlay_clear_brush();
    let values = TileValues::default();
    let patch = build_paint_stroke_patch(
        session.document(),
        &brush,
        &[((0, tile.0, tile.1), values, brush.apply(values))],
    )
    .unwrap_or_else(|error| unreachable!("valid clear stroke: {error:?}"));
    assert!(
        session
            .commit(patch)
            .unwrap_or_else(|error| unreachable!("valid commit: {error:?}"))
            .is_none()
    );
    assert!(!session.can_undo());
}
