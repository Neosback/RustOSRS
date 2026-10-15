use super::*;

fn values(
    underlay: u16,
    overlay: u16,
    shape: u8,
    rotation: u8,
    height: i32,
    flags: u8,
) -> TileValues {
    TileValues {
        underlay,
        overlay,
        shape,
        rotation,
        height,
        flags,
    }
}

#[test]
fn an_empty_selection_has_no_clipboard() {
    assert_eq!(TileClipboard::from_tiles(Vec::new()), None);
}

#[test]
fn tiles_are_stored_relative_to_the_minimum_corner_in_a_stable_order() {
    let clipboard = TileClipboard::from_tiles([
        ((3202, 3201), values(1, 0, 0, 0, 0, 0)),
        ((3200, 3200), values(2, 0, 0, 0, 0, 0)),
        ((3201, 3200), values(3, 0, 0, 0, 0, 0)),
    ])
    .unwrap_or_else(|| unreachable!("non-empty"));
    assert_eq!(clipboard.len(), 3);
    assert_eq!(
        clipboard.tiles_at((100, 50)),
        vec![(100, 50), (101, 50), (102, 51)]
    );
}

#[test]
fn intents_set_every_channel_at_the_target_offset() {
    let clipboard = TileClipboard::from_tiles([
        ((10, 10), values(7, 12, 4, 2, -64, 5)),
        ((11, 10), values(0, 0, 9, 3, 0, 0)),
    ])
    .unwrap_or_else(|| unreachable!("non-empty"));
    let intents = clipboard.intents_at((3200, 3300), |_, height| height - 8);

    assert_eq!(intents.len(), 2);
    let first = &intents[0];
    assert_eq!(first.tile, osrs_core::coords::MapTile::new(3200, 3300));
    assert_eq!(first.underlay, Some(Some(FloorUnderlayId::new(7))));
    assert_eq!(first.overlay, Some(Some(FloorOverlayId::new(12))));
    assert_eq!(
        first.height,
        Some(-72),
        "heights go through the target's quantizer"
    );
    assert_eq!(first.flags, Some(5));
    let (shape, rotation) = first
        .overlay_shape
        .unwrap_or_else(|| unreachable!("has an overlay"));
    assert_eq!((shape.get(), rotation.get()), (4, 2));

    // A tile without an overlay clears the target's overlay and carries no shape.
    let second = &intents[1];
    assert_eq!(second.tile, osrs_core::coords::MapTile::new(3201, 3300));
    assert_eq!(second.underlay, Some(None));
    assert_eq!(second.overlay, Some(None));
    assert_eq!(second.overlay_shape, None);
}

#[test]
fn out_of_range_shape_and_rotation_are_clamped_not_rejected() {
    let clipboard = TileClipboard::from_tiles([((0, 0), values(0, 5, 200, 7, 0, 0))])
        .unwrap_or_else(|| unreachable!("non-empty"));
    let intents = clipboard.intents_at((0, 0), |_, h| h);
    let (shape, rotation) = intents[0]
        .overlay_shape
        .unwrap_or_else(|| unreachable!("overlay"));
    assert_eq!((shape.get(), rotation.get()), (OverlayShape::MAX, 3));
}
