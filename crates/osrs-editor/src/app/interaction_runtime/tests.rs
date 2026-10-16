use super::InteractionRuntime;
use crate::camera::FlyCamera;
use crate::paint::Tool;
use crate::pick::{ObjectPick, Pick, TilePick};
use eframe::egui::{Key, Pos2, Rect};
use osrs_world::LocSpec;
use std::time::Duration;
use transform_gizmo_egui::math::{DQuat, DVec3, Transform};
use web_time::Instant;

fn runtime() -> InteractionRuntime {
    InteractionRuntime::new(FlyCamera::new(0.0, 0.0, 0.0))
}

fn object(region: (i32, i32), index: usize) -> ObjectPick {
    ObjectPick { region, index }
}

fn loc(tile: (i32, i32)) -> LocSpec {
    LocSpec {
        object_id: 1,
        tile,
        plane: 0,
        loc_type: 10,
        orientation: 0,
    }
}

fn transform(x: f64, z: f64) -> Transform {
    Transform::from_scale_rotation_translation(DVec3::ONE, DQuat::IDENTITY, DVec3::new(x, 0.0, z))
}

#[test]
fn hover_bookkeeping_latches_last_tile_and_clear_preserves_it() {
    let mut interaction = runtime();
    interaction.set_hover(Pick {
        object: Some(object((1, 2), 3)),
        tile: Some(TilePick { tile: (10, 11) }),
    });

    assert_eq!(interaction.last_hover_tile, Some((10, 11)));
    assert_eq!(interaction.hover.tile.map(|pick| pick.tile), Some((10, 11)));

    interaction.clear_hover();

    assert!(interaction.hover.object.is_none());
    assert!(interaction.hover.tile.is_none());
    assert_eq!(interaction.last_hover_tile, Some((10, 11)));
}

#[test]
fn context_pick_captures_hover_snapshot() {
    let mut interaction = runtime();
    interaction.set_hover(Pick {
        object: Some(object((1, 2), 3)),
        tile: Some(TilePick { tile: (10, 11) }),
    });
    interaction.capture_context_pick();
    interaction.set_hover(Pick {
        object: Some(object((4, 5), 6)),
        tile: Some(TilePick { tile: (12, 13) }),
    });

    assert_eq!(
        interaction
            .context_pick
            .object
            .as_ref()
            .map(|object| (object.region, object.index)),
        Some(((1, 2), 3))
    );
    assert_eq!(
        interaction.context_pick.tile.map(|pick| pick.tile),
        Some((10, 11))
    );
}

#[test]
fn object_selection_transitions_keep_last_selected_primary() {
    let mut interaction = runtime();
    let first = object((1, 2), 3);
    let second = object((4, 5), 6);

    interaction.add_object(first.clone(), Some((10, 11)));
    interaction.add_object(first.clone(), Some((10, 11)));
    assert_eq!(interaction.selection_objects.len(), 1);
    assert!(interaction.is_primary_object(&first));

    interaction.add_object(second.clone(), Some((12, 13)));
    assert_eq!(interaction.selection_objects.len(), 2);
    assert!(interaction.is_primary_object(&second));
    assert_eq!(interaction.inspect_tile, Some((12, 13)));

    interaction.toggle_object(second.clone(), Some((99, 99)));
    assert_eq!(interaction.selection_objects.len(), 1);
    assert!(interaction.is_primary_object(&first));
    assert_eq!(interaction.inspect_tile, Some((12, 13)));

    interaction.toggle_object(second.clone(), Some((14, 15)));
    assert_eq!(interaction.selection_objects.len(), 2);
    assert!(interaction.is_primary_object(&second));
    assert_eq!(interaction.inspect_tile, Some((14, 15)));
}

#[test]
fn replacing_objects_tracks_the_last_object_without_changing_inspector() {
    let mut interaction = runtime();
    interaction.inspect_tile = Some((7, 8));
    let first = object((1, 1), 1);
    let second = object((2, 2), 2);

    interaction.replace_objects(vec![first, second.clone()]);

    assert!(interaction.is_primary_object(&second));
    assert_eq!(interaction.inspect_tile, Some((7, 8)));
}

#[test]
fn tile_selection_updates_inspector_only_when_a_tile_is_added_as_focus() {
    let mut interaction = runtime();
    interaction.add_tile((1, 2));
    assert_eq!(interaction.inspect_tile, None);

    interaction.select_tile((3, 4));
    assert_eq!(interaction.inspect_tile, Some((3, 4)));

    interaction.toggle_tile((3, 4));
    assert!(!interaction.selection_tiles.contains(&(3, 4)));
    assert_eq!(interaction.inspect_tile, Some((3, 4)));

    interaction.toggle_tile((5, 6));
    assert!(interaction.selection_tiles.contains(&(5, 6)));
    assert_eq!(interaction.inspect_tile, Some((5, 6)));
}

#[test]
fn clear_selection_clears_all_selection_and_inspector_state() {
    let mut interaction = runtime();
    interaction.add_object(object((1, 2), 3), Some((10, 11)));
    interaction.add_tile((12, 13));

    interaction.clear_selection();

    assert!(interaction.selected.is_none());
    assert!(interaction.selection_objects.is_empty());
    assert!(interaction.selection_tiles.is_empty());
    assert!(interaction.inspect_tile.is_none());
}

#[test]
fn pending_reselect_requires_all_targets_before_replacing_selection() {
    let mut interaction = runtime();
    let original = object((1, 1), 1);
    interaction.select_only_object(original.clone(), Some((7, 8)));
    interaction.set_pending_reselect(vec![loc((10, 20)), loc((11, 20))]);

    assert_eq!(interaction.pending_reselect().len(), 2);
    assert!(!interaction.complete_reselect(vec![object((2, 2), 2)]));
    assert_eq!(interaction.pending_reselect().len(), 2);
    assert!(interaction.is_primary_object(&original));
    assert_eq!(interaction.inspect_tile, Some((7, 8)));

    let first = object((2, 2), 2);
    let second = object((3, 3), 3);
    assert!(interaction.complete_reselect(vec![first, second.clone()]));
    assert!(interaction.pending_reselect().is_empty());
    assert_eq!(interaction.selection_objects.len(), 2);
    assert!(interaction.is_primary_object(&second));
    assert_eq!(interaction.inspect_tile, Some((7, 8)));
}

#[test]
fn empty_pending_reselect_cannot_replace_selection() {
    let mut interaction = runtime();
    let original = object((1, 2), 3);
    interaction.select_only_object(original.clone(), None);

    assert!(!interaction.complete_reselect(Vec::new()));
    assert!(interaction.is_primary_object(&original));
    assert_eq!(interaction.selection_objects.len(), 1);
}

#[test]
fn marquee_lifecycle_clamps_pointer_and_latches_modifiers() {
    let mut interaction = runtime();
    let bounds = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 100.0));

    interaction.begin_marquee(Pos2::new(10.0, 20.0), false, false);
    interaction.update_marquee(Some(Pos2::new(150.0, -5.0)), bounds, true, false);
    interaction.update_marquee(None, bounds, false, true);

    let finished = interaction.finish_marquee().map(|marquee| {
        (
            marquee.start,
            marquee.current,
            marquee.additive,
            marquee.toggle,
        )
    });
    assert_eq!(
        finished,
        Some((Pos2::new(10.0, 20.0), Pos2::new(100.0, 0.0), true, true,))
    );
    assert!(interaction.marquee().is_none());
}

#[test]
fn cancel_marquee_discards_in_progress_gesture() {
    let mut interaction = runtime();
    interaction.begin_marquee(Pos2::new(10.0, 20.0), true, false);

    interaction.cancel_marquee();

    assert!(interaction.finish_marquee().is_none());
}

#[test]
fn tool_key_press_latches_return_tool_and_repress_cancels_peek() {
    let mut interaction = runtime();
    let now = Instant::now();

    let target = interaction.press_tool_key(Tool::Select, Tool::Paint, Some(Key::P), now);
    assert_eq!(target, Tool::Paint);
    assert_eq!(interaction.peek_key(), Some(Key::P));

    let target = interaction.press_tool_key(Tool::Paint, Tool::Paint, Some(Key::P), now);
    assert_eq!(target, Tool::Select);
    assert_eq!(interaction.peek_key(), None);
}

#[test]
fn tool_key_release_returns_only_after_peek_threshold() {
    let mut interaction = runtime();
    let started = Instant::now();
    let threshold = Duration::from_millis(280);
    interaction.press_tool_key(Tool::Select, Tool::Paint, Some(Key::P), started);

    let held_until = started + Duration::from_millis(400);
    assert_eq!(
        interaction.release_tool_key(true, held_until, threshold),
        None
    );
    assert_eq!(interaction.peek_key(), Some(Key::P));
    assert_eq!(
        interaction.release_tool_key(false, held_until, threshold),
        Some(Tool::Select)
    );
    assert_eq!(interaction.peek_key(), None);

    interaction.press_tool_key(Tool::Select, Tool::Height, Some(Key::H), started);
    let tapped_at = started + Duration::from_millis(100);
    assert_eq!(
        interaction.release_tool_key(false, tapped_at, threshold),
        None
    );
    assert_eq!(interaction.peek_key(), None);
}

#[test]
fn object_drag_lifecycle_tracks_delta_and_finishes_once() {
    let mut interaction = runtime();
    interaction.begin_object_drag((10, 20), vec![loc((10, 20)), loc((11, 20))]);

    interaction.update_object_drag((14, 17));
    let active = interaction
        .object_drag()
        .map(|drag| (drag.anchor, drag.delta, drag.specs.len()));
    assert_eq!(active, Some(((10, 20), (4, -3), 2)));

    let finished = interaction
        .finish_object_drag()
        .map(|drag| (drag.anchor, drag.delta, drag.specs.len()));
    assert_eq!(finished, Some(((10, 20), (4, -3), 2)));
    assert!(interaction.object_drag().is_none());
    assert!(interaction.finish_object_drag().is_none());
}

#[test]
fn cancel_object_drag_discards_in_progress_gesture() {
    let mut interaction = runtime();
    interaction.begin_object_drag((5, 6), vec![loc((5, 6))]);
    interaction.update_object_drag((8, 9));

    interaction.cancel_object_drag();

    assert!(interaction.object_drag().is_none());
    assert!(interaction.finish_object_drag().is_none());
}

#[test]
fn gizmo_drag_lifecycle_latches_start_specs_and_finishes_once() {
    let mut interaction = runtime();
    let start = transform(0.0, 0.0);
    let start_translation = start.translation;
    let moved = transform(128.0, 256.0);
    let moved_translation = moved.translation;

    let drag = interaction.update_gizmo_drag(start, moved, vec![loc((3, 4))]);
    assert_eq!(drag.start().translation, start_translation);
    assert_eq!(drag.current().translation, moved_translation);
    assert_eq!(drag.specs().len(), 1);
    assert_eq!(drag.specs()[0].tile, (3, 4));

    interaction.update_gizmo_busy(false);
    assert!(interaction.gizmo_busy);

    let later = transform(256.0, 384.0);
    let later_translation = later.translation;
    let drag = interaction.update_gizmo_drag(transform(999.0, 999.0), later, vec![loc((9, 9))]);
    assert_eq!(drag.start().translation, start_translation);
    assert_eq!(drag.current().translation, later_translation);
    assert_eq!(drag.specs()[0].tile, (3, 4));

    let finished = interaction
        .finish_gizmo_drag()
        .map(|drag| (drag.start().translation, drag.current().translation));
    assert_eq!(finished, Some((start_translation, later_translation)));
    assert!(!interaction.gizmo_busy);
    assert!(interaction.gizmo_drag().is_none());
    assert!(interaction.finish_gizmo_drag().is_none());
}

#[test]
fn cancel_gizmo_drag_clears_capture_state() {
    let mut interaction = runtime();
    interaction.update_gizmo_drag(
        transform(0.0, 0.0),
        transform(128.0, 0.0),
        vec![loc((1, 2))],
    );
    interaction.update_gizmo_busy(true);

    interaction.cancel_gizmo_drag();

    assert!(interaction.gizmo_drag().is_none());
    assert!(!interaction.gizmo_busy);
}
