use super::*;

fn tile() -> TileValues {
    TileValues {
        underlay: 49,
        overlay: 111,
        shape: 4,
        rotation: 2,
        height: -40,
        flags: 0x01,
    }
}

#[test]
fn unarmed_channels_keep_the_tile_exactly() {
    let brush = Brush {
        overlay: IdChannel {
            armed: false,
            fill: Fill::Set(7),
        },
        ..Brush::default()
    };
    assert!(brush.is_empty());
    assert_eq!(brush.apply(tile()), tile());
}

#[test]
fn an_armed_overlay_changes_only_the_overlay() {
    let out = Brush::default().apply(tile());
    assert_eq!(out.overlay, 1);
    assert_eq!((out.underlay, out.shape, out.rotation), (49, 4, 2));
    assert_eq!((out.height, out.flags), (-40, 0x01));
}

#[test]
fn clearing_the_overlay_resets_shape_and_rotation() {
    let mut brush = Brush::default();
    brush.overlay.fill = Fill::Clear;
    let out = brush.apply(tile());
    assert_eq!((out.overlay, out.shape, out.rotation), (0, 0, 0));
    assert_eq!(out.underlay, 49);
}

#[test]
fn shape_needs_an_overlay_to_apply_to() {
    let mut brush = Brush::default();
    brush.overlay.armed = false;
    brush.shape = ShapeChannel {
        armed: true,
        shape: 9,
        rotation: 3,
    };
    let on_overlay = brush.apply(tile());
    assert_eq!((on_overlay.shape, on_overlay.rotation), (9, 3));
    let bare = TileValues {
        overlay: 0,
        ..tile()
    };
    assert_eq!(brush.apply(bare), bare);
}

#[test]
fn height_and_flags_are_plain_values_when_armed() {
    let mut brush = Brush::default();
    brush.overlay.armed = false;
    brush.height = HeightChannel {
        armed: true,
        value: -96,
    };
    brush.flags = FlagChannel {
        armed: true,
        bits: 0x0a,
    };
    let out = brush.apply(tile());
    assert_eq!((out.height, out.flags), (-96, 0x0a));
    assert_eq!(out.overlay, 111);
}

#[test]
fn eyedropper_arms_and_copies_the_tile() {
    let mut brush = Brush::default();
    brush.sample(tile());
    assert_eq!(brush.overlay.fill, Fill::Set(111));
    assert_eq!(brush.underlay.fill, Fill::Set(49));
    assert!(brush.shape.armed);
    assert_eq!(brush.apply(TileValues::default()).overlay, 111);
}

#[test]
fn footprint_is_centered_and_sorted() {
    let brush = Brush {
        radius: 1,
        ..Brush::default()
    };
    let tiles = brush.footprint_tiles((10, 10));
    assert_eq!(tiles.len(), 9);
    assert_eq!(tiles[0], (10, 10));
    let circle = Brush {
        radius: 3,
        footprint: BrushShape::Circle,
        ..Brush::default()
    };
    assert!(circle.footprint_tiles((0, 0)).len() < 49);
}

#[test]
fn elevations_round_to_storable_heights() {
    assert_eq!(height_for_elevation(0), 0);
    assert_eq!(height_for_elevation(11), -8);
    assert_eq!(height_for_elevation(12), -16);
    assert_eq!(height_for_elevation(-50), 0);
    assert_eq!(height_for_elevation(99_999), -MAX_ELEVATION);
    assert_eq!(elevation(-96), 96);
}

#[test]
fn the_retired_flags_tool_loads_as_the_spline_tool() -> Result<(), serde_json::Error> {
    let tool: Tool = serde_json::from_str("\"Flags\"")?;
    assert_eq!(tool, Tool::Spline);
    Ok(())
}
