use super::model::{OpenGround, TileBounds};
use super::{
    AutoConnectPolicy, ObstaclePolicy, PathEnvironment, PathPlanner, PathPoint, PathProfile,
    PathSettings,
};
use std::sync::Arc;

fn stress_environment(bounds: TileBounds) -> PathEnvironment {
    PathEnvironment {
        bounds,
        cost_grid: Arc::new(OpenGround),
        existing_paints: Vec::new(),
    }
}

fn stress_settings() -> PathSettings {
    PathSettings {
        sample_spacing: 0.2,
        smoothness: 0.75,
        supersample: 16,
        obstacle_policy: ObstaclePolicy::Ignore,
        auto_connect_policy: AutoConnectPolicy::Off,
        noise_strength: 0.0,
        repair_continuity: true,
        ..PathSettings::default()
    }
}

#[test]
fn max_supersample_long_wide_path_is_deterministic_and_bounded() {
    let bounds = TileBounds::new(0, 0, 192, 64);
    let env = stress_environment(bounds);
    let mut profile = PathProfile::new("stress", "Stress path", 5);
    profile.base_width = 10.0;
    profile.centerline_wander = 0.0;
    profile.edge_noise = 0.0;
    profile.noise_octaves = 1;
    let profile = profile.sanitized();
    let settings = PathSettings {
        width_scale: 1.5,
        ..stress_settings()
    };
    let points = [
        PathPoint::new(8.5, 8.5),
        PathPoint::new(58.5, 27.5),
        PathPoint::new(119.5, 10.5),
        PathPoint::new(183.5, 42.5),
    ];

    let first = PathPlanner.plan_stroke(&points, 0, &profile, &settings, &env);
    let second = PathPlanner.plan_stroke(&points, 0, &profile, &settings, &env);

    assert_eq!(first.paints, second.paints);
    assert_eq!(first.warnings, second.warnings);
    assert_eq!(first.lines, second.lines);
    assert!(!first.paints.is_empty());
    assert_eq!(first.lines.len(), 1);
    assert!(first.lines[0].len() < 5_000);
    assert!(first.paints.len() <= 192 * 64);
    assert!(first.paints.iter().all(|paint| {
        bounds.contains(paint.tile_x, paint.tile_y)
            && paint.coverage.is_finite()
            && (0.0..=1.0).contains(&paint.coverage)
            && paint.shape < 12
            && paint.orientation < 4
    }));
}

#[test]
fn max_supersample_wide_path_clips_cleanly_to_editable_bounds() {
    let bounds = TileBounds::new(0, 0, 96, 24);
    let env = stress_environment(bounds);
    let mut profile = PathProfile::new("edge", "Boundary stress path", 5);
    profile.base_width = 18.0;
    profile.centerline_wander = 0.0;
    profile.width_variation = 0.0;
    profile.edge_noise = 0.0;
    profile.edge_falloff = 0.0;
    profile.noise_octaves = 1;
    let profile = profile.sanitized();
    let settings = PathSettings {
        sample_spacing: 0.25,
        smoothness: 0.0,
        ..stress_settings()
    };
    let points = [PathPoint::new(2.5, 1.5), PathPoint::new(93.5, 1.5)];

    let plan = PathPlanner.plan_stroke(&points, 0, &profile, &settings, &env);

    assert!(!plan.paints.is_empty());
    assert_eq!(plan.lines.len(), 1);
    assert!(plan.lines[0].len() < 1_000);
    assert!(plan.paints.len() <= 96 * 24);
    assert!(plan
        .paints
        .iter()
        .all(|paint| bounds.contains(paint.tile_x, paint.tile_y)));
    assert!(!plan
        .warnings
        .iter()
        .any(|warning| warning.code == "OUT_OF_BOUNDS"));
}

#[test]
fn sanitization_caps_extreme_supersampling_at_supported_maximum() {
    let settings = PathSettings {
        supersample: u32::MAX,
        ..PathSettings::default()
    }
    .sanitized();

    assert_eq!(settings.supersample, 16);
}
