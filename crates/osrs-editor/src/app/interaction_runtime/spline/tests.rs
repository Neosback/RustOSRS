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
