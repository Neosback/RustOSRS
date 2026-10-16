use super::*;
use crate::camera::FlyCamera;

fn interaction() -> InteractionRuntime {
    InteractionRuntime::new(FlyCamera::new(0.0, 0.0, 0.0))
}

const OPTIONS: PathOptions = PathOptions {
    width: 3.0,
    smooth_curve: true,
    edge_smoothing: 1.5,
};

fn input_for(interaction: &InteractionRuntime, pending: Option<(f32, f32)>) -> PlanInput {
    let Some(input) = interaction.plan_input(Some(1), pending, OPTIONS) else {
        unreachable!("these points make a plan")
    };
    input
}

#[test]
fn clicking_adds_points_but_not_twice_in_a_row_on_one_tile() {
    let mut interaction = interaction();
    interaction.add_spline_point((10.5, 20.5));
    interaction.add_spline_point((10.5, 20.5));
    interaction.add_spline_point((14.5, 20.5));
    interaction.add_spline_point((10.5, 20.5));
    assert_eq!(
        interaction.spline_points(),
        &[(10.5, 20.5), (14.5, 20.5), (10.5, 20.5)],
        "a repeat of the last point is ignored; going back to an earlier one is not"
    );
}

#[test]
fn remove_last_and_clear_reset_the_preview_state() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(1.5, 1.5), (5.5, 5.5)]);
    interaction.plan_spline_now(Some(1), None, OPTIONS);
    assert!(matches!(
        interaction.spline_status(),
        SplineStatus::Ready { .. }
    ));
    interaction.pop_spline_point();
    assert_eq!(interaction.spline_points(), &[(1.5, 1.5)]);
    assert!(
        interaction.current_spline_preview().is_none(),
        "the edit stales the preview"
    );
    interaction.clear_spline_points();
    assert!(interaction.spline_points().is_empty());
    assert_eq!(interaction.spline_status(), SplineStatus::NeedsPoints);
    assert!(interaction.spline_preview().is_none());
    // Nothing to remove is not an edit.
    interaction.pop_spline_point();
}

#[test]
fn the_cursor_alone_previews_a_disc_and_one_point_is_a_path() {
    let mut interaction = interaction();
    // No points yet: the cursor's tile is still previewed (a path of one point).
    let preview = input_for(&interaction, Some((3200.5, 3200.5))).make();
    assert!(!preview.paints.is_empty());
    assert_eq!(interaction.spline_status(), SplineStatus::NeedsPoints);
    assert!(!interaction.spline_can_lay());
    // With one placed point there is something to lay.
    interaction.replace_spline_points(vec![(3200.5, 3200.5)]);
    interaction.update_spline_preview(Some(1), None, OPTIONS);
    assert!(interaction.spline_can_lay());
    // No cursor and no point: nothing to draw.
    interaction.clear_spline_points();
    assert!(interaction.plan_input(Some(1), None, OPTIONS).is_none());
}

#[test]
fn laying_needs_an_overlay_and_a_point_but_not_a_finished_preview() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(1.5, 1.5), (9.5, 9.5)]);
    interaction.update_spline_preview(None, None, OPTIONS);
    assert_eq!(interaction.spline_status(), SplineStatus::NeedsOverlay);
    assert!(!interaction.spline_can_lay());
    // With an overlay the same points are being planned, and can be laid at once.
    interaction.update_spline_preview(Some(1), None, OPTIONS);
    assert_ne!(interaction.spline_status(), SplineStatus::NeedsOverlay);
    assert!(interaction.spline_can_lay());
}

#[test]
fn every_input_of_the_fit_is_part_of_the_plan_key() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(3200.5, 3200.5), (3206.5, 3203.5)]);
    let base = input_for(&interaction, None).key;
    let changed = |input: Option<PlanInput>| input.map(|i| i.key);
    for options in [
        PathOptions {
            width: 5.0,
            ..OPTIONS
        },
        PathOptions {
            smooth_curve: false,
            ..OPTIONS
        },
        PathOptions {
            edge_smoothing: 0.0,
            ..OPTIONS
        },
    ] {
        assert_ne!(
            Some(base),
            changed(interaction.plan_input(Some(1), None, options)),
            "{options:?}"
        );
    }
    assert_ne!(
        Some(base),
        changed(interaction.plan_input(Some(2), None, OPTIONS)),
        "overlay"
    );
    assert_ne!(
        Some(base),
        changed(interaction.plan_input(Some(1), Some((3210.5, 3203.5)), OPTIONS)),
        "cursor"
    );
    interaction.replace_spline_points(vec![(3200.5, 3200.5), (3207.5, 3203.5)]);
    assert_ne!(base, input_for(&interaction, None).key, "a point");
    // Unchanged inputs give the same key.
    interaction.replace_spline_points(vec![(3200.5, 3200.5), (3206.5, 3203.5)]);
    assert_eq!(base, input_for(&interaction, None).key);
}

#[test]
fn the_cursor_is_part_of_the_preview_line_but_the_placed_fit_ignores_it() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(3200.5, 3200.5), (3206.5, 3200.5)]);
    let plain = input_for(&interaction, None).make();
    let ahead = input_for(&interaction, Some((3212.5, 3204.5))).make();
    assert!(
        ahead.paints.len() > plain.paints.len(),
        "the preview reaches the cursor"
    );
    let (_, placed) = fit_points(interaction.spline_points(), None, OPTIONS);
    assert_eq!(
        placed, plain.paints,
        "what is laid is the placed points only"
    );
}

#[test]
fn editing_stales_a_ready_preview_before_the_worker_answers() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(1.5, 1.5), (5.5, 5.5)]);
    interaction.plan_spline_now(Some(1), None, OPTIONS);
    assert!(interaction.spline_preview_is_current());
    interaction.add_spline_point((9.5, 5.5));
    assert!(interaction.current_spline_preview().is_none());
    assert!(
        interaction.spline_preview().is_some(),
        "the old preview stays visible meanwhile"
    );
}

#[test]
fn cancelling_a_running_plan_holds_until_an_edit_or_resume() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(1.5, 1.5), (9.5, 9.5)]);
    // Force the plan to be "running": a desired key with no matching preview.
    interaction.spline.desired_key = Some(7);
    interaction.spline.preview = None;
    assert_eq!(interaction.spline_status(), SplineStatus::Updating);
    interaction.cancel_spline_plan();
    assert_eq!(interaction.spline_status(), SplineStatus::Cancelled);

    // Cancelling is only meaningful while updating: a second call changes nothing.
    interaction.cancel_spline_plan();
    assert_eq!(interaction.spline_status(), SplineStatus::Cancelled);

    // Adding a point changes the plan key, which lifts the cancellation.
    interaction.add_spline_point((12.5, 9.5));
    interaction.update_spline_preview(Some(1), None, OPTIONS);
    assert_ne!(interaction.spline_status(), SplineStatus::Cancelled);

    // Resume lifts it for unchanged points too.
    interaction.plan_spline_now(Some(1), None, OPTIONS);
    interaction.spline.preview = None;
    interaction.spline.cancelled_key = interaction.spline.desired_key;
    assert_eq!(interaction.spline_status(), SplineStatus::Cancelled);
    interaction.resume_spline_plan();
    assert_ne!(interaction.spline_status(), SplineStatus::Cancelled);
}

#[test]
fn a_cancelled_plan_never_starts_a_worker_for_the_same_key() {
    let mut interaction = interaction();
    interaction.replace_spline_points(vec![(1.5, 1.5), (9.5, 9.5)]);
    let key = input_for(&interaction, None).key;
    interaction.spline.cancelled_key = Some(key);
    interaction.update_spline_preview(Some(1), None, OPTIONS);
    #[cfg(not(target_arch = "wasm32"))]
    assert!(interaction.spline.worker.is_none());
    assert!(interaction.spline_preview().is_none());
    assert_eq!(interaction.spline_status(), SplineStatus::Cancelled);
}

#[test]
fn the_line_is_smoothed_only_when_asked_and_only_from_three_points() {
    let points = [(0.5, 0.5), (10.5, 10.5), (20.5, 0.5)];
    assert_eq!(path_line(&points, None, false).len(), 3);
    assert_eq!(path_line(&points, None, true).len(), 2 * 12 + 1);
    assert_eq!(
        path_line(&points[..2], None, true).len(),
        2,
        "two points stay straight"
    );
    assert_eq!(
        path_line(&points[..2], Some((20.5, 0.5)), true).len(),
        2 * 12 + 1,
        "the cursor counts"
    );
}
