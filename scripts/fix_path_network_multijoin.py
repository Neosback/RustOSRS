from pathlib import Path

path = Path('crates/osrs-editor/src/terrain_path/planner.rs')
text = path.read_text()

helper_start = text.index('/// The point of an existing path nearest to `point` within `radius`, for joining onto it.')
helper_end = text.index('/// How the overlay of one tile is chosen among the profile\'s variants:', helper_start)
helpers = r'''/// The point of an existing path nearest to `point` within `radius`, for joining onto it.
fn snap_target_from<'a>(
    point: PathPoint,
    plane: u8,
    compatible: &PathProfile,
    radius: f64,
    paints: impl IntoIterator<Item = &'a TilePaint>,
    atlas: &OverlayShapeAtlas,
) -> Option<PathPoint> {
    let resolution = atlas.resolution();
    let reach = 0.5_f64.sqrt() + 1.0e-9;
    let mut best_tile = None;
    let mut best_point = None;
    let mut best = radius;
    for paint in paints {
        if paint.plane != plane || !compatible.compatible(paint.overlay_id) {
            continue;
        }
        let centre = PathPoint::new(f64::from(paint.tile_x) + 0.5, f64::from(paint.tile_y) + 0.5);
        if point.distance_to(centre) - reach > best {
            continue;
        }
        for bit in atlas.mask(paint.shape, paint.orientation).iter() {
            let (cx, cy) = (bit % resolution, bit / resolution);
            let sample = PathPoint::new(
                f64::from(paint.tile_x) + (cx as f64 + 0.5) / resolution as f64,
                f64::from(paint.tile_y) + (cy as f64 + 0.5) / resolution as f64,
            );
            let distance = point.distance_to(sample);
            let tile = (paint.tile_x, paint.tile_y);
            let earlier = best_tile.is_none_or(|candidate| tile < candidate);
            if distance < best - 1.0e-9 || ((distance - best).abs() <= 1.0e-9 && earlier) {
                best = distance;
                best_tile = Some(tile);
                best_point = Some(sample);
            }
        }
    }
    best_point
}

fn snap_target(
    point: PathPoint,
    plane: u8,
    compatible: &PathProfile,
    radius: f64,
    existing: &[TilePaint],
    atlas: &OverlayShapeAtlas,
) -> Option<PathPoint> {
    snap_target_from(
        point,
        plane,
        compatible,
        radius,
        existing.iter(),
        atlas,
    )
}

/// Compatible existing path tiles grouped into local connected components. Eight-neighbour tile
/// adjacency intentionally treats diagonal road runs as one existing path instead of producing a
/// connector for every diagonal tile.
fn compatible_path_components(
    plane: u8,
    profile: &PathProfile,
    existing: &[TilePaint],
) -> Vec<Vec<usize>> {
    let mut eligible: Vec<usize> = existing
        .iter()
        .enumerate()
        .filter(|(_, paint)| paint.plane == plane && profile.compatible(paint.overlay_id))
        .map(|(index, _)| index)
        .collect();
    eligible.sort_by_key(|&index| {
        let paint = &existing[index];
        (
            paint.tile_x,
            paint.tile_y,
            paint.overlay_id,
            paint.shape,
            paint.orientation,
            index,
        )
    });

    let mut by_tile: BTreeMap<(i32, i32), Vec<usize>> = BTreeMap::new();
    for &index in &eligible {
        let paint = &existing[index];
        by_tile
            .entry((paint.tile_x, paint.tile_y))
            .or_default()
            .push(index);
    }

    let mut visited = HashSet::new();
    let mut components = Vec::new();
    for seed in eligible {
        if !visited.insert(seed) {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = VecDeque::from([seed]);
        while let Some(index) = queue.pop_front() {
            component.push(index);
            let paint = &existing[index];
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    if let Some(neighbours) = by_tile.get(&(paint.tile_x + dx, paint.tile_y + dy)) {
                        for &neighbour in neighbours {
                            if visited.insert(neighbour) {
                                queue.push_back(neighbour);
                            }
                        }
                    }
                }
            }
        }
        components.push(component);
    }
    components
}

/// In Network mode, branch from the authored line to each separated compatible existing-path
/// component that comes within snap radius. Endpoint snapping is handled separately, so these
/// connectors never bend the authored centre line merely to create junctions.
fn network_branches(
    line: &[PathPoint],
    plane: u8,
    profile: &PathProfile,
    settings: &PathSettings,
    existing: &[TilePaint],
    atlas: &OverlayShapeAtlas,
) -> Vec<Vec<PathPoint>> {
    if settings.auto_connect_policy != AutoConnectPolicy::Network
        || !profile.auto_connect
        || settings.snap_radius <= 0.0
        || line.len() < 3
        || existing.is_empty()
    {
        return Vec::new();
    }

    let first = line[0];
    let last = line[line.len() - 1];
    let endpoint_guard = settings.snap_radius.max(profile.base_width * 0.5);
    let mut along = vec![0.0; line.len()];
    for index in 1..line.len() {
        along[index] = along[index - 1] + line[index - 1].distance_to(line[index]);
    }

    let components = compatible_path_components(plane, profile, existing);
    let mut candidates: Vec<(f64, usize, PathPoint, usize)> = Vec::new();
    for (component_id, component) in components.iter().enumerate() {
        let mut best: Option<(f64, usize, PathPoint)> = None;
        for (index, point) in line
            .iter()
            .copied()
            .enumerate()
            .skip(1)
            .take(line.len() - 2)
        {
            if point.distance_to(first) <= endpoint_guard || point.distance_to(last) <= endpoint_guard {
                continue;
            }
            let Some(target) = snap_target_from(
                point,
                plane,
                profile,
                settings.snap_radius,
                component.iter().map(|&paint_index| &existing[paint_index]),
                atlas,
            ) else {
                continue;
            };
            let distance = point.distance_to(target);
            // If the authored line already touches this existing component, no branch is useful.
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
        if let Some((distance, index, target)) = best {
            candidates.push((distance, index, target, component_id));
        }
    }

    // If two disconnected components both crowd the same bit of authored line, take the shorter
    // branch. Well-separated encounters remain independent network connections.
    candidates.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.3.cmp(&b.3))
    });
    let source_guard = profile
        .base_width
        .max(settings.sample_spacing * 2.0)
        .max(0.75);
    let mut selected: Vec<(f64, usize, PathPoint, usize)> = Vec::new();
    for candidate in candidates {
        if selected.iter().all(|other| {
            (along[candidate.1] - along[other.1]).abs() >= source_guard
        }) {
            selected.push(candidate);
        }
    }
    selected.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.3.cmp(&b.3)));
    selected
        .into_iter()
        .map(|(_, index, target, _)| {
            resample(&[line[index], target], settings.sample_spacing)
        })
        .collect()
}

'''
text = text[:helper_start] + helpers + text[helper_end:]

plan_start = text.index('            let mut branch = network_branch(')
plan_end = text.index('\n        // A junction of three or more edges of one profile gets a round pad.', plan_start)
plan_section = r'''            let branches = network_branches(
                &line,
                plane,
                profile,
                settings,
                &env.existing_paints,
                &atlas,
            );
            let mut connected = Vec::with_capacity(branches.len());
            for connector in branches {
                if edge.auto_route && avoids && profile.reroute {
                    let routed = route(&connector, plane, env, settings, clearance, max_slope);
                    if routed.succeeded() {
                        connected.push(routed.points);
                    } else {
                        warnings.push(PathWarning::general(
                            Severity::Warning,
                            "NETWORK_CONNECT_BLOCKED",
                            format!(
                                "Edge {} could not connect to a nearby path: {}",
                                edge.id, routed.failure
                            ),
                        ));
                    }
                } else {
                    connected.push(connector);
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
            for (branch_index, connector) in connected.into_iter().enumerate() {
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
                    edge_seed
                        ^ 0x004E_4554_574F_524B_i64
                        ^ id_mix(branch_index as u64 + 1),
                    base_width,
                    base_width,
                    mask_blocked,
                    &mut cells,
                );
            }
        }
'''
text = text[:plan_start] + plan_section + text[plan_end:]

test_insert = r'''

    #[test]
    fn network_policy_connects_multiple_separated_existing_paths() {
        let profile = PathProfile::new("default", "Default path", 5).sanitized();
        let mut existing = Vec::new();
        for x in [5, 15] {
            existing.extend((8..=10).map(|y| TilePaint::new(x, y, 0, 5, 0, 0)));
        }
        let env = PathEnvironment {
            bounds: TileBounds::new(0, 0, 24, 20),
            cost_grid: Arc::new(OpenGround),
            existing_paints: existing,
        };
        let points = [PathPoint::new(0.5, 5.5), PathPoint::new(20.5, 5.5)];
        let settings = PathSettings {
            smoothness: 0.0,
            noise_strength: 0.0,
            snap_radius: 4.0,
            auto_connect_policy: AutoConnectPolicy::Network,
            ..PathSettings::default()
        };

        let plan = PathPlanner.plan_stroke(&points, 0, &profile, &settings, &env);

        assert_eq!(plan.lines.len(), 3);
        assert_eq!(
            plan.warnings
                .iter()
                .filter(|warning| warning.code == "NETWORK_CONNECTED")
                .count(),
            2
        );
        let mut targets: Vec<f64> = plan.lines[1..]
            .iter()
            .filter_map(|line| line.last().map(|point| point.x))
            .collect();
        targets.sort_by(f64::total_cmp);
        assert_eq!(targets.len(), 2);
        assert!(targets[0] < 10.0);
        assert!(targets[1] > 10.0);
    }

    #[test]
    fn network_policy_uses_one_connector_for_one_contiguous_existing_path() {
        let profile = PathProfile::new("default", "Default path", 5).sanitized();
        let existing_paints = (5..=15)
            .map(|x| TilePaint::new(x, 8, 0, 5, 0, 0))
            .collect();
        let env = PathEnvironment {
            bounds: TileBounds::new(0, 0, 24, 20),
            cost_grid: Arc::new(OpenGround),
            existing_paints,
        };
        let points = [PathPoint::new(0.5, 5.5), PathPoint::new(20.5, 5.5)];
        let settings = PathSettings {
            smoothness: 0.0,
            noise_strength: 0.0,
            snap_radius: 4.0,
            auto_connect_policy: AutoConnectPolicy::Network,
            ..PathSettings::default()
        };

        let plan = PathPlanner.plan_stroke(&points, 0, &profile, &settings, &env);

        assert_eq!(plan.lines.len(), 2);
        assert_eq!(
            plan.warnings
                .iter()
                .filter(|warning| warning.code == "NETWORK_CONNECTED")
                .count(),
            1
        );
    }
'''
module_end = text.rfind('\n}')
if module_end == -1:
    raise SystemExit('test module end not found')
text = text[:module_end] + test_insert + text[module_end:]

path.write_text(text)
