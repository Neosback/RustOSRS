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
    interaction.update_spline_preview(&crate::paint::SplineSettings::default(), Some(1));
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
    interaction.update_spline_preview(&settings, None);
    assert_eq!(interaction.spline_status(), SplineStatus::NeedsOverlay);
    // With an overlay the same points are being planned, not waiting for a choice.
    interaction.update_spline_preview(&settings, Some(1));
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
    interaction.update_spline_preview(&settings, Some(1));
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
    let Some(input) = interaction.plan_input(&settings, Some(1)) else {
        unreachable!("two points and an overlay make a plan")
    };
    interaction.spline.cancelled_key = Some(input.key);
    interaction.update_spline_preview(&settings, Some(1));
    #[cfg(not(target_arch = "wasm32"))]
    assert!(interaction.spline.worker.is_none());
    assert!(interaction.spline_preview().is_none());
    assert_eq!(interaction.spline_status(), SplineStatus::Cancelled);
}
