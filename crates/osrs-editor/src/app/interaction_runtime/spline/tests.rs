use super::*;
use crate::camera::FlyCamera;

fn interaction() -> InteractionRuntime {
    InteractionRuntime::new(FlyCamera::new(0.0, 0.0, 0.0))
}

#[test]
fn spline_point_lifecycle_appends_inserts_and_updates_only_active_point() {
    let mut interaction = interaction();
    interaction.append_spline_point_and_drag((1.0, 2.0));
    assert_eq!(interaction.spline_points(), &[(1.0, 2.0)]);
    assert_eq!(interaction.spline.drag, Some(0));

    interaction.finish_spline_drag();
    interaction.append_spline_point_and_drag((5.0, 6.0));
    interaction.finish_spline_drag();
    interaction.insert_spline_point_and_drag(1, (3.0, 4.0));
    assert_eq!(
        interaction.spline_points(),
        &[(1.0, 2.0), (3.0, 4.0), (5.0, 6.0)]
    );
    assert_eq!(interaction.spline.drag, Some(1));

    interaction.update_spline_drag((3.5, 4.5));
    assert_eq!(
        interaction.spline_points(),
        &[(1.0, 2.0), (3.5, 4.5), (5.0, 6.0)]
    );
}

#[test]
fn spline_drag_finish_and_cancel_clear_capture_without_changing_points() {
    let mut interaction = interaction();
    interaction.append_spline_point_and_drag((1.0, 2.0));
    interaction.finish_spline_drag();
    assert_eq!(interaction.spline.drag, None);
    assert_eq!(interaction.spline_points(), &[(1.0, 2.0)]);

    interaction.begin_spline_drag(0);
    interaction.cancel_spline_drag();
    interaction.update_spline_drag((9.0, 9.0));
    assert_eq!(interaction.spline.drag, None);
    assert_eq!(interaction.spline_points(), &[(1.0, 2.0)]);
}

#[test]
fn spline_remove_pop_and_clear_mutate_points_through_runtime() {
    let mut interaction = interaction();
    interaction.append_spline_point_and_drag((1.0, 1.0));
    interaction.finish_spline_drag();
    interaction.append_spline_point_and_drag((2.0, 2.0));
    interaction.finish_spline_drag();
    interaction.append_spline_point_and_drag((3.0, 3.0));
    interaction.finish_spline_drag();

    interaction.remove_spline_point(1);
    assert_eq!(interaction.spline_points(), &[(1.0, 1.0), (3.0, 3.0)]);
    interaction.pop_spline_point();
    assert_eq!(interaction.spline_points(), &[(1.0, 1.0)]);
    interaction.clear_spline_points();
    assert!(interaction.spline_points().is_empty());
}

#[test]
fn removing_a_point_cancels_its_captured_drag_instead_of_moving_a_successor() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(1.0, 1.0), (2.0, 2.0)]);
    interaction.begin_spline_drag(0);
    interaction.remove_spline_point(0);
    interaction.update_spline_drag((9.0, 9.0));
    assert_eq!(interaction.spline_points(), &[(2.0, 2.0)]);
    assert_eq!(interaction.spline_drag(), None);
}

#[test]
fn moving_a_point_invalidates_ready_preview_before_next_worker_poll() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(1.0, 1.0), (5.0, 5.0)]);
    interaction.plan_spline_now(Some(1));
    assert!(matches!(
        interaction.spline_status(),
        SplineStatus::Ready { .. }
    ));
    interaction.move_spline_point(0, (2.0, 3.0));
    assert!(interaction.current_spline_preview().is_none());
    interaction.clear_spline_points();
    assert_eq!(interaction.spline_status(), SplineStatus::NeedsPoints);
    assert!(interaction.spline_preview().is_none());
}

#[test]
fn non_finite_script_points_fail_without_starting_a_fit() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(f32::NAN, 1.0), (5.0, 5.0)]);
    interaction.update_spline_preview(Some(1), None);
    assert!(matches!(
        interaction.spline_status(),
        SplineStatus::Failed(_)
    ));
    assert!(interaction.current_spline_preview().is_none());
}

#[test]
fn deleting_a_point_preserves_selection_identity_and_cancels_drag() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(1.0, 1.0), (2.0, 2.0), (3.0, 3.0)]);
    interaction.begin_spline_drag(2);
    interaction.remove_spline_point(0);
    assert_eq!(interaction.selected_spline_point(), Some(1));
    assert_eq!(interaction.spline_drag(), None);
    interaction.remove_spline_point(1);
    assert_eq!(interaction.selected_spline_point(), None);
}

#[test]
fn missing_overlay_and_running_plan_are_distinct_states() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(1.0, 1.0), (9.0, 9.0)]);
    interaction.update_spline_preview(None, None);
    assert_eq!(interaction.spline_status(), SplineStatus::NeedsOverlay);
    // With an overlay the same points are being planned, not waiting for a choice.
    interaction.update_spline_preview(Some(1), None);
    assert_ne!(interaction.spline_status(), SplineStatus::NeedsOverlay);
    assert!(matches!(
        interaction.spline_status(),
        SplineStatus::Updating | SplineStatus::Ready { .. }
    ));
}

#[test]
fn cancelling_a_running_plan_holds_until_an_edit_or_resume() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(1.0, 1.0), (9.0, 9.0)]);
    // Force the plan to be "running": a desired key with no matching preview.
    interaction.spline.desired_key = Some(7);
    interaction.spline.preview = None;
    assert_eq!(interaction.spline_status(), SplineStatus::Updating);
    interaction.cancel_spline_plan();
    assert_eq!(interaction.spline_status(), SplineStatus::Cancelled);

    // Cancelling is only meaningful while updating: a second call changes nothing.
    interaction.cancel_spline_plan();
    assert_eq!(interaction.spline_status(), SplineStatus::Cancelled);

    // Moving a point changes the plan key, which lifts the cancellation.
    interaction.move_spline_point(0, (2.0, 2.0));
    interaction.update_spline_preview(Some(1), None);
    assert_ne!(interaction.spline_status(), SplineStatus::Cancelled);

    // Resume lifts it for unchanged points too.
    interaction.plan_spline_now(Some(1));
    interaction.spline.preview = None;
    let key = interaction.spline.desired_key;
    interaction.spline.cancelled_key = key;
    assert_eq!(interaction.spline_status(), SplineStatus::Cancelled);
    interaction.resume_spline_plan();
    assert_ne!(interaction.spline_status(), SplineStatus::Cancelled);
}

#[test]
fn a_cancelled_plan_never_starts_a_worker_for_the_same_key() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(1.0, 1.0), (9.0, 9.0)]);
    let Some(input) = interaction.plan_input(Some(1), None) else {
        unreachable!("two points and an overlay make a plan")
    };
    interaction.spline.cancelled_key = Some(input.key);
    interaction.update_spline_preview(Some(1), None);
    #[cfg(not(target_arch = "wasm32"))]
    assert!(interaction.spline.worker.is_none());
    assert!(interaction.spline_preview().is_none());
    assert_eq!(interaction.spline_status(), SplineStatus::Cancelled);
}

fn input_for(interaction: &InteractionRuntime, pending: Option<(f32, f32)>) -> PlanInput {
    let Some(input) = interaction.plan_input(Some(1), pending) else {
        unreachable!("these points make a plan")
    };
    input
}

#[test]
fn each_point_has_its_own_width_and_corner_and_both_replan() {
    let mut interaction = interaction();
    interaction.set_spline_default_width(3.0);
    interaction.replace_spline_points(vec![(3200.5, 3200.5), (3206.5, 3204.5), (3212.5, 3200.5)]);
    let before = input_for(&interaction, None);
    assert!(
        interaction
            .spline_attrs()
            .iter()
            .all(|a| a.width == 3.0 && !a.corner)
    );

    interaction.set_spline_point_width(1, 5.0);
    let wider = input_for(&interaction, None);
    assert_ne!(before.key, wider.key, "a width change replans");
    assert_eq!(interaction.spline_controls()[1].width, 5.0);
    assert_eq!(interaction.spline_controls()[0].width, 3.0);

    interaction.toggle_spline_corner(1);
    let corner = input_for(&interaction, None);
    assert_ne!(wider.key, corner.key, "a corner replans");
    interaction.toggle_spline_corner(1);
    assert_eq!(
        input_for(&interaction, None).key,
        wider.key,
        "and toggling back restores the key"
    );
}

#[test]
fn widths_stay_with_their_points_through_insert_remove_and_pop() {
    let mut interaction = interaction();
    interaction.set_spline_default_width(1.0);
    interaction.append_spline_point_and_drag((0.5, 0.5));
    interaction.finish_spline_drag();
    interaction.set_spline_default_width(5.0);
    interaction.append_spline_point_and_drag((8.5, 0.5));
    interaction.finish_spline_drag();
    // An inserted point takes the average width of the stretch it splits.
    interaction.insert_spline_point_and_drag(1, (4.5, 0.5));
    interaction.finish_spline_drag();
    let widths: Vec<f32> = interaction.spline_attrs().iter().map(|a| a.width).collect();
    assert_eq!(widths, [1.0, 3.0, 5.0]);
    interaction.remove_spline_point(0);
    let widths: Vec<f32> = interaction.spline_attrs().iter().map(|a| a.width).collect();
    assert_eq!(widths, [3.0, 5.0]);
    interaction.pop_spline_point();
    assert_eq!(interaction.spline_attrs().len(), 1);
    interaction.clear_spline_points();
    assert!(interaction.spline_attrs().is_empty());
}

#[test]
fn a_width_preset_applies_to_the_selected_point_or_to_all_points() {
    let mut interaction = interaction();
    interaction.set_spline_default_width(3.0);
    interaction.replace_spline_points(vec![(0.5, 0.5), (5.5, 0.5), (10.5, 0.5)]);
    interaction.select_spline_point(1);
    interaction.apply_spline_width(5.0);
    let widths: Vec<f32> = interaction.spline_attrs().iter().map(|a| a.width).collect();
    assert_eq!(widths, [3.0, 5.0, 3.0], "only the selected point");
    interaction.clear_spline_points();
    interaction.replace_spline_points(vec![(0.5, 0.5), (5.5, 0.5)]);
    interaction.apply_spline_width(1.0);
    assert!(
        interaction.spline_attrs().iter().all(|a| a.width == 1.0),
        "no selection: all points"
    );
    // Nudging is bounded.
    interaction.nudge_spline_width(-5.0);
    assert!(
        interaction
            .spline_attrs()
            .iter()
            .all(|a| a.width == MIN_WIDTH)
    );
    interaction.nudge_spline_width(100.0);
    assert!(
        interaction
            .spline_attrs()
            .iter()
            .all(|a| a.width == MAX_WIDTH)
    );
}

#[test]
fn the_cursor_extends_the_preview_but_never_the_placed_key() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(3200.5, 3200.5), (3206.5, 3200.5)]);
    let plain = input_for(&interaction, None);
    let ahead = input_for(&interaction, Some((3212.5, 3204.5)));
    assert_eq!(plain.key, ahead.key, "the placed path is the same");
    assert_ne!(
        plain.request_key, ahead.request_key,
        "but the request differs"
    );
    let (plain, ahead) = (plain.make(None), ahead.make(None));
    assert_eq!(
        plain.paints, ahead.paints,
        "the placed fit ignores the cursor"
    );
    assert!(plain.ahead.is_none());
    let Some(extension) = ahead.ahead else {
        unreachable!("the cursor produces an extension")
    };
    assert!(
        extension.paints.len() > ahead.paints.len(),
        "the extension covers more tiles"
    );
}

#[test]
fn one_placed_point_and_the_cursor_preview_the_first_segment_without_being_a_path() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(3200.5, 3200.5)]);
    assert!(interaction.plan_input(Some(1), None).is_none());
    let preview = input_for(&interaction, Some((3208.5, 3200.5))).make(None);
    assert!(preview.paints.is_empty(), "one point lays nothing");
    assert!(preview.ahead.is_some_and(|a| !a.paints.is_empty()));
    interaction.update_spline_preview(Some(1), Some((3208.5, 3200.5)));
    assert_eq!(interaction.spline_status(), SplineStatus::NeedsPoints);
}

#[test]
fn the_placed_fit_is_reused_when_only_the_cursor_moves() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(3200.5, 3200.5), (3206.5, 3203.5)]);
    let first = input_for(&interaction, Some((3212.5, 3200.5))).make(None);
    let moved = input_for(&interaction, Some((3212.5, 3206.5))).make(Some(&first));
    assert_eq!(first.paints, moved.paints);
    assert_eq!(first.line, moved.line);
    assert_ne!(
        first.ahead.map(|a| a.pending),
        moved.ahead.map(|a| a.pending)
    );
}

#[test]
fn a_moved_cursor_replans_while_the_placed_preview_stays_ready() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(3200.5, 3200.5), (3206.5, 3203.5)]);
    interaction.plan_spline_now(Some(1));
    assert!(matches!(
        interaction.spline_status(),
        SplineStatus::Ready { .. }
    ));
    interaction.update_spline_preview(Some(1), Some((3212.5, 3200.5)));
    assert!(matches!(
        interaction.spline_status(),
        SplineStatus::Ready { .. }
    ));
}

#[test]
fn tiles_with_other_overlays_are_left_alone_reported_and_change_the_key() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(3200.5, 3210.5), (3220.5, 3210.5)]);
    let open = input_for(&interaction, None);
    let blocked: std::collections::BTreeSet<(i32, i32)> = (3208..=3212)
        .flat_map(|x| (3205..=3215).map(move |y| (x, y)))
        .collect();
    interaction.set_spline_blocked(blocked, ((3190, 3200), (3230, 3220)), 1, 1, 0);
    assert!(interaction.spline_blocked_covers(((3195, 3205), (3225, 3215)), 1, 1, 0));
    assert!(
        !interaction.spline_blocked_covers(((3195, 3205), (3225, 3215)), 2, 1, 0),
        "terrain changed"
    );
    assert!(
        !interaction.spline_blocked_covers(((3195, 3205), (3225, 3215)), 1, 2, 0),
        "overlay changed"
    );
    let walled = input_for(&interaction, None);
    assert_ne!(open.key, walled.key, "a changed wall replans");
    let (open, walled) = (open.make(None), walled.make(None));
    assert!(walled.paints.len() < open.paints.len());
    assert!(
        walled
            .paints
            .iter()
            .all(|t| !(3208..=3212).contains(&t.tile_x))
    );
    assert!(!walled.clipped.is_empty());
    assert!(open.clipped.is_empty());
}

#[test]
fn wander_and_ground_apply_to_the_selection_or_all_and_become_the_defaults() {
    use crate::terrain_path::Wander;
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(0.5, 0.5), (5.5, 0.5), (10.5, 0.5)]);
    let seeds: Vec<u32> = interaction.spline_attrs().iter().map(|a| a.seed).collect();
    assert!(
        seeds[0] != seeds[1] && seeds[1] != seeds[2] && seeds[0] != seeds[2],
        "every point has its own seed: {seeds:?}"
    );
    let before = input_for(&interaction, None).key;

    interaction.select_spline_point(1);
    interaction.sync_spline_defaults(Wander::Natural, false);
    let wanders: Vec<Wander> = interaction
        .spline_attrs()
        .iter()
        .map(|a| a.wander)
        .collect();
    assert_eq!(
        wanders,
        [Wander::Off, Wander::Natural, Wander::Off],
        "selected point only"
    );
    assert_ne!(before, input_for(&interaction, None).key, "wander replans");

    // Syncing unchanged toggles does not overwrite a point edited by hand.
    interaction.set_spline_point_wander(2, Wander::Gentle);
    interaction.sync_spline_defaults(Wander::Natural, false);
    assert_eq!(interaction.spline_attrs()[2].wander, Wander::Gentle);

    // A point added afterwards starts with the toggles' setting; an inserted one with its neighbour's.
    interaction.append_spline_point_and_drag((15.5, 0.5));
    interaction.finish_spline_drag();
    assert_eq!(interaction.spline_attrs()[3].wander, Wander::Natural);
    interaction.insert_spline_point_and_drag(3, (13.5, 0.5));
    interaction.finish_spline_drag();
    assert_eq!(interaction.spline_attrs()[3].wander, Wander::Gentle);

    // With nothing selected a changed toggle applies to every stretch.
    interaction.spline.selected = None;
    interaction.sync_spline_defaults(Wander::Off, true);
    assert!(
        interaction
            .spline_attrs()
            .iter()
            .all(|a| a.wander == Wander::Off && a.smooth_ground)
    );

    // The ground flag of one point can be flipped on its own and replans.
    let key = input_for(&interaction, None).key;
    interaction.toggle_spline_point_ground(0);
    assert!(!interaction.spline_attrs()[0].smooth_ground);
    assert_ne!(key, input_for(&interaction, None).key);
}
