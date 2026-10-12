from pathlib import Path

path = Path('crates/osrs-editor/src/terrain_path/planner.rs')
text = path.read_text()

anchor = '''/// How the overlay of one tile is chosen among the profile's variants: a patch of the path (a
/// stretch along it) gets one material, picked by weight.
fn choose_overlay(profile: &PathProfile, seed: i64, along: f64, plane: u8) -> u16 {
'''
insert = '''/// In Network mode, find the best interior point from which to branch onto an existing
/// compatible path. Endpoint snapping is handled separately so this connector does not bend the
/// authored centre line just to create a network junction.
fn network_branch(
    line: &[PathPoint],
    plane: u8,
    profile: &PathProfile,
    settings: &PathSettings,
    existing: &[TilePaint],
    atlas: &OverlayShapeAtlas,
) -> Option<Vec<PathPoint>> {
    if settings.auto_connect_policy != AutoConnectPolicy::Network
        || !profile.auto_connect
        || settings.snap_radius <= 0.0
        || line.len() < 3
        || existing.is_empty()
    {
        return None;
    }
    let first = line[0];
    let last = line[line.len() - 1];
    let endpoint_guard = settings.snap_radius.max(profile.base_width * 0.5);
    let mut best: Option<(f64, usize, PathPoint)> = None;
    for (index, point) in line.iter().copied().enumerate().skip(1).take(line.len() - 2) {
        if point.distance_to(first) <= endpoint_guard || point.distance_to(last) <= endpoint_guard {
            continue;
        }
        let Some(target) = snap_target(
            point,
            plane,
            profile,
            settings.snap_radius,
            existing,
            atlas,
        ) else {
            continue;
        };
        let distance = point.distance_to(target);
        // If the line already touches the existing path there is nothing useful to branch.
        if distance <= 0.35 {
            continue;
        }
        let replace = best.is_none_or(|(best_distance, best_index, _)| {
            distance < best_distance - 1.0e-9
                || ((distance - best_distance).abs() <= 1.0e-9 && index < best_index)
        });
        if replace {
            best = Some((distance, index, target));
        }
    }
    let (_, index, target) = best?;
    Some(resample(&[line[index], target], settings.sample_spacing))
}

'''
assert anchor in text, 'choose_overlay anchor changed unexpectedly'
text = text.replace(anchor, insert + anchor, 1)

old = '''            lines.push(line.clone());
            // Stamp the path onto the sample grids of the tiles it passes over.
            let mask_blocked = edge.auto_route && masks_overlap_policy(settings);
            stamp(
                &line,
                plane,
                profile,
                settings,
                env,
                edge_seed,
                start_width,
                end_width,
                mask_blocked,
                &mut cells,
            );
'''
new = '''            let mut branch = network_branch(
                &line,
                plane,
                profile,
                settings,
                &env.existing_paints,
                &atlas,
            );
            if let Some(connector) = &mut branch
                && edge.auto_route
                && avoids
                && profile.reroute
            {
                let routed = route(connector, plane, env, settings, clearance, max_slope);
                if routed.succeeded() {
                    *connector = routed.points;
                } else {
                    warnings.push(PathWarning::general(
                        Severity::Warning,
                        "NETWORK_CONNECT_BLOCKED",
                        format!(
                            "Edge {} could not connect to the nearby path: {}",
                            edge.id, routed.failure
                        ),
                    ));
                    branch = None;
                }
            }

            lines.push(line.clone());
            // Stamp the path onto the sample grids of the tiles it passes over.
            let mask_blocked = edge.auto_route && masks_overlap_policy(settings);
            stamp(
                &line,
                plane,
                profile,
                settings,
                env,
                edge_seed,
                start_width,
                end_width,
                mask_blocked,
                &mut cells,
            );
            if let Some(connector) = branch {
                warnings.push(PathWarning::general(
                    Severity::Info,
                    "NETWORK_CONNECTED",
                    format!("Edge {} joined a nearby path from its interior", edge.id),
                ));
                lines.push(connector.clone());
                stamp(
                    &connector,
                    plane,
                    profile,
                    settings,
                    env,
                    edge_seed ^ 0x4E45_5457_4F52_4B_i64,
                    base_width,
                    base_width,
                    mask_blocked,
                    &mut cells,
                );
            }
'''
assert old in text, 'main stamp block changed unexpectedly'
text = text.replace(old, new, 1)

append = r'''

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn existing_vertical_path() -> Vec<TilePaint> {
        (9..=12)
            .map(|y| TilePaint::new(10, y, 0, 5, 0, 0))
            .collect()
    }

    #[test]
    fn network_policy_adds_an_interior_connector_without_bending_the_main_line() {
        let profile = PathProfile::new("default", "Default path", 5).sanitized();
        let env = PathEnvironment {
            bounds: TileBounds::new(0, 0, 24, 20),
            cost_grid: Arc::new(OpenGround),
            existing_paints: existing_vertical_path(),
        };
        let points = [PathPoint::new(2.5, 5.5), PathPoint::new(18.5, 5.5)];
        let endpoints = PathSettings {
            smoothness: 0.0,
            noise_strength: 0.0,
            snap_radius: 4.0,
            auto_connect_policy: AutoConnectPolicy::Endpoints,
            ..PathSettings::default()
        };
        let network = PathSettings {
            auto_connect_policy: AutoConnectPolicy::Network,
            ..endpoints
        };

        let endpoint_plan = PathPlanner.plan_stroke(&points, 0, &profile, &endpoints, &env);
        let network_plan = PathPlanner.plan_stroke(&points, 0, &profile, &network, &env);

        assert_eq!(endpoint_plan.lines.len(), 1);
        assert_eq!(network_plan.lines.len(), 2);
        assert_eq!(network_plan.lines[0][0], endpoint_plan.lines[0][0]);
        assert_eq!(
            network_plan.lines[0][network_plan.lines[0].len() - 1],
            endpoint_plan.lines[0][endpoint_plan.lines[0].len() - 1]
        );
        assert!(
            network_plan
                .warnings
                .iter()
                .any(|warning| warning.code == "NETWORK_CONNECTED")
        );
        assert!(
            network_plan
                .paints
                .iter()
                .any(|paint| paint.tile_x == 10 && paint.tile_y >= 7)
        );
    }
}
'''
assert '#[cfg(test)]\nmod tests {' not in text, 'planner tests module already exists'
text = text.rstrip() + append
path.write_text(text)
