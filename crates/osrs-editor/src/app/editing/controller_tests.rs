use super::*;

fn gesture_fixture() -> (TerrainController, TileKey, TileValues) {
    let key = (0, 3200, 3200);
    let before = TileValues {
        underlay: 3,
        height: -32,
        ..TileValues::default()
    };
    let mut controller = TerrainController::default();
    controller.edited.insert(key, before);
    controller
        .redo_stack
        .push(EditStep::semantic_height(vec![key]));
    (controller, key, before)
}

#[test]
fn cancelling_paint_preview_restores_channels_without_consuming_redo() {
    let (mut controller, key, before) = gesture_fixture();
    controller.begin_stroke();
    let mut stroke = controller.stroke.take().unwrap_or_else(|| unreachable!());
    let after = TileValues {
        overlay: 7,
        flags: 2,
        ..before
    };
    assert!(controller.record(
        &crate::pick::Infos::new(),
        &mut stroke,
        key,
        after,
        &mut Vec::new()
    ));
    controller.stroke = Some(stroke);
    let restored = controller.cancel_stroke(&crate::pick::Infos::new());
    assert_eq!(
        controller.current_tile(&crate::pick::Infos::new(), key),
        Some(before)
    );
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].overlay, 0);
    assert_eq!(restored[0].flags, 0);
    assert!(controller.history.is_empty());
    assert_eq!(controller.redo_stack.len(), 1);
    assert!(
        controller
            .cancel_stroke(&crate::pick::Infos::new())
            .is_empty()
    );
}

#[test]
fn cancelling_height_preview_restores_source_height_encoding_and_existing_overrides() {
    for existing_override in [false, true] {
        let (mut controller, key, before) = gesture_fixture();
        if existing_override {
            controller.height_edited.insert(key);
        }
        controller.begin_stroke();
        let mut stroke = controller.stroke.take().unwrap_or_else(|| unreachable!());
        let after = TileValues {
            height: -64,
            ..before
        };
        assert!(controller.record(
            &crate::pick::Infos::new(),
            &mut stroke,
            key,
            after,
            &mut Vec::new()
        ));
        controller.stroke = Some(stroke);
        let restored = controller.cancel_stroke(&crate::pick::Infos::new());
        assert_eq!(restored[0].height, existing_override.then_some(4));
        assert_eq!(controller.height_edited.contains(&key), existing_override);
        assert_eq!(
            controller.current_tile(&crate::pick::Infos::new(), key),
            Some(before)
        );
        assert!(controller.history.is_empty());
        assert_eq!(controller.redo_stack.len(), 1);
    }
}
