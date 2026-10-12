use super::model::{OpenGround, TileBounds};
use super::{
    AutoConnectPolicy, PathEnvironment, PathPlanner, PathPoint, PathProfile, PathSettings,
    TilePaint,
};
use std::sync::Arc;

fn network_settings() -> PathSettings {
    PathSettings {
        smoothness: 0.0,
        noise_strength: 0.0,
        sample_spacing: 0.5,
        snap_radius: 4.0,
        auto_connect_policy: AutoConnectPolicy::Network,
        ..PathSettings::default()
    }
}

fn environment(existing_paints: Vec<TilePaint>) -> PathEnvironment {
    PathEnvironment {
        bounds: TileBounds::new(0, 0, 28, 20),
        cost_grid: Arc::new(OpenGround),
        existing_paints,
    }
}

fn connected_warning_count(plan: &super::model::PathPlan) -> usize {
    plan.warnings
        .iter()
        .filter(|warning| warning.code == "NETWORK_CONNECTED")
        .count()
}

#[test]
fn diagonal_existing_run_is_one_network_component() {
    let profile = PathProfile::new("default", "Default path", 5).sanitized();
    let existing = [(9, 8), (10, 9), (11, 10)]
        .into_iter()
        .map(|(x, y)| TilePaint::new(x, y, 0, 5, 0, 0))
        .collect();
    let env = environment(existing);
    let points = [PathPoint::new(1.5, 5.5), PathPoint::new(20.5, 5.5)];

    let plan = PathPlanner.plan_stroke(&points, 0, &profile, &network_settings(), &env);

    assert_eq!(plan.lines.len(), 2);
    assert_eq!(connected_warning_count(&plan), 1);
}

#[test]
fn incompatible_overlay_and_other_plane_do_not_create_network_branches() {
    let profile = PathProfile::new("default", "Default path", 5).sanitized();
    let mut existing = Vec::new();
    existing.extend((8..=10).map(|y| TilePaint::new(6, y, 0, 5, 0, 0)));
    existing.extend((8..=10).map(|y| TilePaint::new(13, y, 0, 9, 0, 0)));
    existing.extend((8..=10).map(|y| TilePaint::new(20, y, 1, 5, 0, 0)));
    let env = environment(existing);
    let points = [PathPoint::new(0.5, 5.5), PathPoint::new(24.5, 5.5)];

    let plan = PathPlanner.plan_stroke(&points, 0, &profile, &network_settings(), &env);

    assert_eq!(plan.lines.len(), 2);
    assert_eq!(connected_warning_count(&plan), 1);
    let target = plan.lines[1].last().copied().expect("connector target");
    assert!(target.x < 10.0);
}

#[test]
fn existing_path_already_touching_authored_line_gets_no_redundant_branch() {
    let profile = PathProfile::new("default", "Default path", 5).sanitized();
    let existing = (4..=8).map(|y| TilePaint::new(10, y, 0, 5, 0, 0)).collect();
    let env = environment(existing);
    let points = [PathPoint::new(2.5, 5.5), PathPoint::new(18.5, 5.5)];

    let plan = PathPlanner.plan_stroke(&points, 0, &profile, &network_settings(), &env);

    assert_eq!(plan.lines.len(), 1);
    assert_eq!(connected_warning_count(&plan), 0);
}

#[test]
fn existing_paint_order_does_not_change_network_junctions() {
    let profile = PathProfile::new("default", "Default path", 5).sanitized();
    let mut existing = Vec::new();
    for x in [5, 15] {
        existing.extend((8..=10).map(|y| TilePaint::new(x, y, 0, 5, 0, 0)));
    }
    let mut reversed = existing.clone();
    reversed.reverse();
    let points = [PathPoint::new(0.5, 5.5), PathPoint::new(20.5, 5.5)];
    let settings = network_settings();

    let forward = PathPlanner.plan_stroke(&points, 0, &profile, &settings, &environment(existing));
    let backward = PathPlanner.plan_stroke(&points, 0, &profile, &settings, &environment(reversed));

    assert_eq!(forward.lines, backward.lines);
    assert_eq!(forward.paints, backward.paints);
    assert_eq!(connected_warning_count(&forward), 2);
    assert_eq!(connected_warning_count(&backward), 2);
}
