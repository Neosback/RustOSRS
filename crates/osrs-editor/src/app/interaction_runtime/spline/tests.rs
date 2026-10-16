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
    let settings = crate::paint::SplineSettings::default();
    interaction.replace_spline_points(vec![(1.0, 1.0), (5.0, 5.0)]);
    interaction.plan_spline_now(&settings, Some(1));
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
    interaction.update_spline_preview(&crate::paint::SplineSettings::default(), Some(1), None);
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
    let settings = crate::paint::SplineSettings::default();
    interaction.replace_spline_points(vec![(1.0, 1.0), (9.0, 9.0)]);
    interaction.update_spline_preview(&settings, None, None);
    assert_eq!(interaction.spline_status(), SplineStatus::NeedsOverlay);
    // With an overlay the same points are being planned, not waiting for a choice.
    interaction.update_spline_preview(&settings, Some(1), None);
    assert_ne!(interaction.spline_status(), SplineStatus::NeedsOverlay);
    assert!(matches!(
        interaction.spline_status(),
        SplineStatus::Updating | SplineStatus::Ready { .. }
    ));
}

#[test]
fn cancelling_a_running_plan_holds_until_an_edit_or_resume() {
    let mut interaction = interaction();
    let settings = crate::paint::SplineSettings::default();
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
    interaction.update_spline_preview(&settings, Some(1), None);
    assert_ne!(interaction.spline_status(), SplineStatus::Cancelled);

    // Resume lifts it for unchanged points too.
    interaction.plan_spline_now(&settings, Some(1));
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
    let settings = crate::paint::SplineSettings::default();
    interaction.replace_spline_points(vec![(1.0, 1.0), (9.0, 9.0)]);
    let Some(input) = interaction.plan_input(&settings, Some(1), None) else {
        unreachable!("two points and an overlay make a plan")
    };
    interaction.spline.cancelled_key = Some(input.key);
    interaction.update_spline_preview(&settings, Some(1), None);
    #[cfg(not(target_arch = "wasm32"))]
    assert!(interaction.spline.worker.is_none());
    assert!(interaction.spline_preview().is_none());
    assert_eq!(interaction.spline_status(), SplineStatus::Cancelled);
}

fn input_for(
    interaction: &InteractionRuntime,
    settings: &crate::paint::SplineSettings,
    pending: Option<(f32, f32)>,
) -> PlanInput {
    let Some(input) = interaction.plan_input(settings, Some(1), pending) else {
        unreachable!("these points make a plan")
    };
    input
}

#[test]
fn the_curve_option_changes_the_plan_key_and_the_fitted_line() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(3200.5, 3200.5), (3206.5, 3204.5), (3212.5, 3200.5)]);
    let smooth = crate::paint::SplineSettings::default();
    let straight = crate::paint::SplineSettings {
        curve: crate::paint::PathCurve::Straight,
        ..smooth.clone()
    };
    let (a, b) = (
        input_for(&interaction, &smooth, None),
        input_for(&interaction, &straight, None),
    );
    assert_ne!(a.key, b.key, "switching the curve replans");
    let (smooth_line, straight_line) = (a.make(None).lines, b.make(None).lines);
    assert_eq!(
        straight_line[0].len(),
        3,
        "straight keeps the control points"
    );
    assert!(smooth_line[0].len() > 3, "smooth is a dense curve");
}

#[test]
fn the_cursor_extends_the_preview_but_never_the_placed_key() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(3200.5, 3200.5), (3206.5, 3200.5)]);
    let settings = crate::paint::SplineSettings::default();
    let plain = input_for(&interaction, &settings, None);
    let ahead = input_for(&interaction, &settings, Some((3212.5, 3204.5)));
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
    let settings = crate::paint::SplineSettings::default();
    interaction.replace_spline_points(vec![(3200.5, 3200.5)]);
    assert!(interaction.plan_input(&settings, Some(1), None).is_none());
    let input = input_for(&interaction, &settings, Some((3208.5, 3200.5)));
    let preview = input.make(None);
    assert!(preview.paints.is_empty(), "one point lays nothing");
    assert!(preview.ahead.is_some_and(|a| !a.paints.is_empty()));
    interaction.update_spline_preview(&settings, Some(1), Some((3208.5, 3200.5)));
    assert_eq!(interaction.spline_status(), SplineStatus::NeedsPoints);
}

#[test]
fn following_the_cursor_can_be_switched_off() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(3200.5, 3200.5)]);
    let off = crate::paint::SplineSettings {
        follow_cursor: false,
        ..Default::default()
    };
    assert!(
        interaction
            .plan_input(&off, Some(1), Some((3208.5, 3200.5)))
            .is_none()
    );
    interaction.replace_spline_points(vec![(3200.5, 3200.5), (3206.5, 3200.5)]);
    assert!(
        input_for(&interaction, &off, Some((3212.5, 3200.5)))
            .pending
            .is_none()
    );
}

#[test]
fn the_placed_fit_is_reused_when_only_the_cursor_moves() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(3200.5, 3200.5), (3206.5, 3203.5)]);
    let settings = crate::paint::SplineSettings::default();
    let first = input_for(&interaction, &settings, Some((3212.5, 3200.5))).make(None);
    let moved = input_for(&interaction, &settings, Some((3212.5, 3206.5))).make(Some(&first));
    assert_eq!(first.paints, moved.paints);
    assert_eq!(first.lines, moved.lines);
    assert_ne!(
        first.ahead.map(|a| a.pending),
        moved.ahead.map(|a| a.pending)
    );
}

#[test]
fn a_moved_cursor_replans_while_the_placed_preview_stays_ready() {
    let mut interaction = interaction();
    let settings = crate::paint::SplineSettings::default();
    interaction.replace_spline_points(vec![(3200.5, 3200.5), (3206.5, 3203.5)]);
    interaction.plan_spline_now(&settings, Some(1));
    assert!(matches!(
        interaction.spline_status(),
        SplineStatus::Ready { .. }
    ));
    // The cursor moves: the placed preview is still current (Lay path stays enabled).
    interaction.update_spline_preview(&settings, Some(1), Some((3212.5, 3200.5)));
    assert!(matches!(
        interaction.spline_status(),
        SplineStatus::Ready { .. }
    ));
}
