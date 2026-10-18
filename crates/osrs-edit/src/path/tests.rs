use super::*;
use crate::{HeightVertexCoord, MapDocument};
use osrs_core::ids::FloorUnderlayId;

fn plane() -> SourcePlane {
    SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
}

fn position(tile: (i32, i32)) -> MapPosition {
    MapPosition::new(MapTile::new(tile.0, tile.1), plane())
}

fn intent(tile: (i32, i32), overlay: u16, shape: u8, rotation: u8) -> TerrainPathIntent {
    TerrainPathIntent {
        tile: MapTile::new(tile.0, tile.1),
        overlay: Some(overlay),
        shape,
        rotation,
    }
}

#[test]
fn path_patch_sets_the_overlay_and_keeps_everything_else() {
    let tile = (3200, 3200);
    let before = TerrainTile {
        underlay: Some(FloorUnderlayId::new(3)),
        overlay: None,
        flags: 1,
    };
    let vertex = HeightVertexCoord::new(MapTile::new(tile.0, tile.1), plane());
    let mut document = MapDocument::from_parts([(position(tile), before)], [(vertex, 32)], []);

    let patch = build_terrain_path_patch(&document, plane(), &[intent(tile, 7, 4, 2)])
        .unwrap_or_else(|error| unreachable!("valid path patch: {error:?}"));
    assert_eq!(patch.len(), 1);
    assert!(document.apply_patch(&patch).is_ok());

    let after = document.terrain_at(position(tile));
    assert_eq!(after.underlay, before.underlay);
    assert_eq!(after.flags, 1);
    let overlay = after
        .overlay
        .unwrap_or_else(|| unreachable!("path should set an overlay"));
    assert_eq!(overlay.id.get(), 7);
    assert_eq!(overlay.shape.get(), 4);
    assert_eq!(overlay.rotation.get(), 2);
    assert_eq!(document.height_at(vertex), Some(32));
    // One undo restores the tile exactly.
    assert!(document.apply_patch(&patch.inverse()).is_ok());
    assert_eq!(document.terrain_at(position(tile)), before);
}

#[test]
fn duplicate_tile_intents_collapse_to_one_final_terrain_op() {
    let tile = (3200, 3200);
    let mut document = MapDocument::new();
    let path = [intent(tile, 7, 1, 0), intent(tile, 8, 2, 3)];

    let patch = build_terrain_path_patch(&document, plane(), &path)
        .unwrap_or_else(|error| unreachable!("valid path patch: {error:?}"));
    assert_eq!(patch.len(), 1);
    assert!(document.apply_patch(&patch).is_ok());
    let overlay = document
        .terrain_at(position(tile))
        .overlay
        .unwrap_or_else(|| unreachable!("path should set an overlay"));
    assert_eq!(overlay.id.get(), 8);
    assert_eq!(overlay.shape.get(), 2);
    assert_eq!(overlay.rotation.get(), 3);
}

#[test]
fn semantic_no_op_produces_an_empty_patch() {
    let tile = (3200, 3200);
    let overlay = TerrainOverlay {
        id: FloorOverlayId::new(7),
        shape: OverlayShape::new(4).unwrap_or_else(|| unreachable!("overlay shape four is valid")),
        rotation: OverlayRotation::new(2)
            .unwrap_or_else(|| unreachable!("overlay rotation two is valid")),
    };
    let document = MapDocument::from_parts(
        [(
            position(tile),
            TerrainTile {
                underlay: None,
                overlay: Some(overlay),
                flags: 0,
            },
        )],
        [],
        [],
    );

    let patch = build_terrain_path_patch(&document, plane(), &[intent(tile, 7, 4, 2)])
        .unwrap_or_else(|error| unreachable!("valid path patch: {error:?}"));
    assert!(patch.is_empty());
}

#[test]
fn invalid_shape_and_rotation_are_rejected() {
    let document = MapDocument::new();
    let tile = MapTile::new(1, 1);
    assert_eq!(
        build_terrain_path_patch(&document, plane(), &[intent((1, 1), 7, 12, 0)]),
        Err(PathCommitError::InvalidOverlayShape { tile, shape: 12 })
    );
    assert_eq!(
        build_terrain_path_patch(&document, plane(), &[intent((1, 1), 7, 1, 4)]),
        Err(PathCommitError::InvalidOverlayRotation { tile, rotation: 4 })
    );
}

#[test]
fn companion_underlay_sets_underlay_and_reverses_on_undo() {
    let tile = (3200, 3200);
    let before = TerrainTile {
        underlay: Some(FloorUnderlayId::new(2)),
        overlay: None,
        flags: 0,
    };
    let mut document = MapDocument::from_parts([(position(tile), before)], [], []);
    let patch = build_terrain_path_patch_with_underlay(
        &document,
        plane(),
        &[intent(tile, 7, 4, 1)],
        Some(9),
    )
    .expect("valid companion path patch");
    assert_eq!(patch.len(), 1);
    document.apply_patch(&patch).expect("apply patch");
    let after = document.terrain_at(position(tile));
    assert_eq!(after.underlay, Some(FloorUnderlayId::new(9)));
    assert_eq!(after.overlay.map(|o| o.id.get()), Some(7));
    document.apply_patch(&patch.inverse()).expect("undo patch");
    assert_eq!(document.terrain_at(position(tile)), before);
}
