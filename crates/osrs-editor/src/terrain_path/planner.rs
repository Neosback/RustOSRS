//! The path planner: a network of edges in, the tiles to paint out.
//!
//! Each edge is smoothed and resampled, joined onto nearby paths of the same overlay, routed
//! round obstacles, nudged sideways by noise (so it wanders), and stamped onto a grid of
//! sub-tile samples with a width that varies along its length and an edge that is rough. A tile
//! belongs to the path when enough of it is covered; the shape each tile takes then comes from
//! the road shape grammar (what real maps do with that neighbourhood), and seams between
//! neighbours that do not meet are repaired.

use super::atlas::{BitMask, EAST, NORTH, OverlayShapeAtlas, SOUTH, WEST};
use super::grammar::{self, NEIGHBOURS};
use super::model::*;
use super::noise::{SeededPerlinNoise, mix64};
use super::resample::{resample, smooth_and_resample};
use super::router::{blocked_at, route};
use super::settings::{AutoConnectPolicy, ObstaclePolicy, PathProfile, PathSettings};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

const SIDES: [u8; 4] = [NORTH, EAST, SOUTH, WEST];

type Tile = (i32, i32, u8);

/// Where the stamps of one profile fall on one tile.
struct Cell {
    profile: PathProfile,
    bits: BitMask,
    seed: i64,
    sum_along: f64,
    count: u32,
}

impl Cell {
    fn new(profile: &PathProfile, samples: usize, seed: i64) -> Self {
        Self {
            profile: profile.clone(),
            bits: BitMask::new(samples * samples),
            seed,
            sum_along: 0.0,
            count: 0,
        }
    }
}

type CellKey = (i32, i32, u8, String);

/// A tile that is part of the plan, with the cell that won it.
#[derive(Clone)]
struct Chosen {
    key: CellKey,
    cell_index: usize,
    coverage: f64,
}

fn side_step(tile: Tile, side: u8) -> Tile {
    match side {
        NORTH => (tile.0, tile.1 - 1, tile.2),
        EAST => (tile.0 + 1, tile.1, tile.2),
        SOUTH => (tile.0, tile.1 + 1, tile.2),
        _ => (tile.0 - 1, tile.1, tile.2),
    }
}

fn opposite(side: u8) -> u8 {
    match side {
        NORTH => SOUTH,
        EAST => WEST,
        SOUTH => NORTH,
        _ => EAST,
    }
}

/// Plane, then x, then y.
fn tile_order(tile: &Tile) -> (u8, i32, i32) {
    (tile.2, tile.0, tile.1)
}

/// Two edge masks that both reach their side but never at the same place.
fn seam_mismatch(a: u32, b: u32) -> bool {
    a != 0 && b != 0 && a & b == 0
}

/// Label cardinally connected selected tiles, keeping profiles separate.
fn coverage_component_labels(
    strong: &BTreeMap<(u8, i32, i32), Chosen>,
    weak: &BTreeMap<(u8, i32, i32), Chosen>,
) -> HashMap<Tile, usize> {
    let mut by_tile: HashMap<Tile, String> = HashMap::new();
    for (key, chosen) in strong.iter().chain(weak.iter()) {
        by_tile.insert((key.1, key.2, key.0), chosen.key.3.clone());
    }
    let mut order: Vec<Tile> = by_tile.keys().copied().collect();
    order.sort_by_key(tile_order);
    let mut labels = HashMap::new();
    let mut component = 0;
    for seed in order {
        if labels.contains_key(&seed) {
            continue;
        }
        let profile = by_tile[&seed].clone();
        let mut queue = VecDeque::from([seed]);
        labels.insert(seed, component);
        while let Some(current) = queue.pop_front() {
            for side in SIDES {
                let next = side_step(current, side);
                if labels.contains_key(&next)
                    || by_tile
                        .get(&next)
                        .is_none_or(|candidate| candidate != &profile)
                {
                    continue;
                }
                labels.insert(next, component);
                queue.push_back(next);
            }
        }
        component += 1;
    }
    labels
}

/// Restore only low-coverage tiles that actually join two otherwise separate generated pieces.
/// This runs after the normal Terraini coverage rule, so already-connected parity cases are left
/// untouched.
fn repair_coverage_continuity(
    strong: &BTreeMap<(u8, i32, i32), Chosen>,
    weak: &mut BTreeMap<(u8, i32, i32), Chosen>,
    candidates: &BTreeMap<(u8, i32, i32), Chosen>,
) -> usize {
    let mut added = 0;
    loop {
        let labels = coverage_component_labels(strong, weak);
        let mut best: Option<((u8, i32, i32), Chosen)> = None;
        for (key, chosen) in candidates {
            if strong.contains_key(key) || weak.contains_key(key) {
                continue;
            }
            let tile = (key.1, key.2, key.0);
            let mut components = Vec::new();
            for side in SIDES {
                let next = side_step(tile, side);
                let next_key = tile_order(&next);
                let neighbour = strong.get(&next_key).or_else(|| weak.get(&next_key));
                let Some(neighbour) = neighbour else {
                    continue;
                };
                if neighbour.key.3 != chosen.key.3 {
                    continue;
                }
                if let Some(component) = labels.get(&next)
                    && !components.contains(component)
                {
                    components.push(*component);
                }
            }
            if components.len() < 2 {
                continue;
            }
            let replace = best.as_ref().is_none_or(|(best_key, best_chosen)| {
                chosen.coverage > best_chosen.coverage
                    || (chosen.coverage == best_chosen.coverage && key < best_key)
            });
            if replace {
                best = Some((*key, chosen.clone()));
            }
        }
        let Some((key, chosen)) = best else {
            break;
        };
        weak.insert(key, chosen);
        added += 1;
    }
    added
}

fn shape_component_labels(
    merged: &[(Tile, Chosen)],
    index_of: &HashMap<Tile, usize>,
    shapes: &[u32],
    atlas: &OverlayShapeAtlas,
) -> Vec<usize> {
    let mut labels = vec![usize::MAX; merged.len()];
    let mut component = 0;
    for seed in 0..merged.len() {
        if labels[seed] != usize::MAX {
            continue;
        }
        labels[seed] = component;
        let mut queue = VecDeque::from([seed]);
        while let Some(current) = queue.pop_front() {
            let (tile, chosen) = &merged[current];
            for side in SIDES {
                let next = side_step(*tile, side);
                let Some(&neighbour) = index_of.get(&next) else {
                    continue;
                };
                if labels[neighbour] != usize::MAX || merged[neighbour].1.key.3 != chosen.key.3 {
                    continue;
                }
                let mine = atlas.edge_mask(
                    (shapes[current] >> 2) as u8,
                    (shapes[current] & 3) as u8,
                    side,
                );
                let theirs = atlas.edge_mask(
                    (shapes[neighbour] >> 2) as u8,
                    (shapes[neighbour] & 3) as u8,
                    opposite(side),
                );
                if mine & theirs != 0 {
                    labels[neighbour] = component;
                    queue.push_back(neighbour);
                }
            }
        }
        component += 1;
    }
    labels
}

/// If side-adjacent generated tiles still belong to different shape components after normal seam
/// repair, make the boundary tiles whole. Distant components remain warnings rather than being
/// joined across empty or obstacle-masked space.
fn repair_shape_continuity(
    merged: &[(Tile, Chosen)],
    index_of: &HashMap<Tile, usize>,
    shapes: &mut [u32],
    atlas: &OverlayShapeAtlas,
) -> usize {
    let mut repaired = 0;
    loop {
        let labels = shape_component_labels(merged, index_of, shapes, atlas);
        let mut bridge = None;
        'outer: for (i, (tile, chosen)) in merged.iter().enumerate() {
            for side in [EAST, SOUTH] {
                let next = side_step(*tile, side);
                let Some(&j) = index_of.get(&next) else {
                    continue;
                };
                if merged[j].1.key.3 == chosen.key.3 && labels[i] != labels[j] {
                    bridge = Some((i, j));
                    break 'outer;
                }
            }
        }
        let Some((a, b)) = bridge else {
            break;
        };
        if shapes[a] == 0 && shapes[b] == 0 {
            break;
        }
        shapes[a] = 0;
        shapes[b] = 0;
        repaired += 1;
    }
    repaired
}

fn length_of(points: &[PathPoint]) -> f64 {
    points.windows(2).map(|w| w[0].distance_to(w[1])).sum()
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// The direction an end of the path faces: its position and the unit vector towards the path.
fn cap_direction(from: PathPoint, towards: PathPoint) -> Option<[f64; 4]> {
    let (dx, dy) = (towards.x - from.x, towards.y - from.y);
    let length = dx.hypot(dy);
    (length >= 1.0e-9).then(|| [from.x, from.y, dx / length, dy / length])
}

/// Whether a sample lies behind an end cap (off the end of the path).
fn behind_cap(x: f64, y: f64, cap: Option<[f64; 4]>) -> bool {
    cap.is_some_and(|c| (x - c[0]) * c[2] + (y - c[1]) * c[3] < 0.0)
}

/// Whether the cap can matter to a segment (it lies within `reach` of the end).
fn cap_reaches(cap: Option<[f64; 4]>, a: PathPoint, b: PathPoint, reach: f64) -> bool {
    cap.is_some_and(|c| {
        ((a.x - c[0]) * c[2] + (a.y - c[1]) * c[3]).min((b.x - c[0]) * c[2] + (b.y - c[1]) * c[3])
            <= reach
    })
}

fn masks_overlap_policy(settings: &PathSettings) -> bool {
    matches!(
        settings.obstacle_policy,
        ObstaclePolicy::Mask | ObstaclePolicy::AvoidSolids | ObstaclePolicy::AvoidAll
    )
}

pub struct PathPlanner;

/// A one-edge network along `points`: nodes at the two ends, the rest control points. `ids` are
/// the network, start node, end node and edge ids.
pub fn stroke_network(
    points: &[PathPoint],
    plane: u8,
    profile: &PathProfile,
    seed: i64,
    ids: [u64; 4],
) -> PathNetwork {
    let first = points[0];
    let last = points[points.len() - 1];
    let nodes = vec![
        PathNode::new(ids[1], first.x, first.y, plane),
        PathNode::new(ids[2], last.x, last.y, plane),
    ];
    let middle = if points.len() == 2 {
        Vec::new()
    } else {
        points[1..points.len() - 1].to_vec()
    };
    let edge = PathEdge::new(ids[3], ids[1], ids[2], middle, &profile.id);
    PathNetwork {
        id: ids[0],
        name: "Stroke".to_owned(),
        seed,
        nodes,
        edges: vec![edge],
        profiles: vec![profile.clone()],
    }
}

/// The point of an existing path nearest to `point` within `radius`, for joining onto it.
fn snap_target(
    point: PathPoint,
    plane: u8,
    compatible: &PathProfile,
    radius: f64,
    existing: &[TilePaint],
    atlas: &OverlayShapeAtlas,
) -> Option<PathPoint> {
    let resolution = atlas.resolution();
    let reach = 0.5_f64.sqrt() + 1.0e-9;
    let mut best_paint: Option<&TilePaint> = None;
    let mut best_point = None;
    let mut best = radius;
    for paint in existing {
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
            let earlier =
                best_paint.is_none_or(|b| (paint.tile_x, paint.tile_y) < (b.tile_x, b.tile_y));
            if distance < best - 1.0e-9 || (distance == best && earlier) {
                best = distance;
                best_paint = Some(paint);
                best_point = Some(sample);
            }
        }
    }
    best_point
}

/// In Network mode, find the best interior point from which to branch onto an existing
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
        let Some(target) =
            snap_target(point, plane, profile, settings.snap_radius, existing, atlas)
        else {
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

/// How the overlay of one tile is chosen among the profile's variants: a patch of the path (a
/// stretch along it) gets one material, picked by weight.
fn choose_overlay(profile: &PathProfile, seed: i64, along: f64, plane: u8) -> u16 {
    if profile.material_variants.len() == 1 {
        return profile.material_variants[0].overlay_id;
    }
    let patch = (along / profile.material_patch_length).floor() as i64;
    let hash = mix64(
        seed ^ profile.parameter_signature()
            ^ i64::from(plane).wrapping_mul(1_609_587_929_392_839_161)
            ^ patch.wrapping_mul(-7_046_029_254_386_353_131),
    );
    let draw = ((hash as u64) >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0);
    let total: f64 = profile.material_variants.iter().map(|v| v.weight).sum();
    let mut remaining = draw * total;
    for variant in &profile.material_variants {
        remaining -= variant.weight;
        if remaining < 0.0 {
            return variant.overlay_id;
        }
    }
    profile.primary_overlay_id
}

impl PathPlanner {
    /// A stroke (a line of points) as a one-edge network, planned.
    pub fn plan_stroke(
        &self,
        points: &[PathPoint],
        plane: u8,
        profile: &PathProfile,
        settings: &PathSettings,
        env: &PathEnvironment,
    ) -> PathPlan {
        if points.len() < 2 {
            return PathPlan::empty();
        }
        // Ids come from the stroke itself, so the same stroke plans the same way every time.
        let signature = format!(
            "{points:?}:{plane}:{}:{}",
            settings.seed,
            profile.parameter_signature()
        );
        let id = |salt: &str| fnv(&format!("{signature}:{salt}"));
        let network = stroke_network(
            points,
            plane,
            profile,
            settings.seed,
            [id("network"), id("start"), id("end"), id("edge")],
        );
        self.plan(&network, settings, env)
    }

    pub fn plan(
        &self,
        network: &PathNetwork,
        settings: &PathSettings,
        env: &PathEnvironment,
    ) -> PathPlan {
        if env.bounds.is_empty() || network.edges.is_empty() {
            return PathPlan::empty();
        }
        let samples = settings.supersample as usize;
        let atlas = OverlayShapeAtlas::new(samples);
        let nodes: HashMap<u64, &PathNode> = network.nodes.iter().map(|n| (n.id, n)).collect();
        let profiles: HashMap<&str, &PathProfile> = network
            .profiles
            .iter()
            .map(|p| (p.id.as_str(), p))
            .collect();
        let mut cells: BTreeMap<CellKey, Cell> = BTreeMap::new();
        let mut warnings: Vec<PathWarning> = Vec::new();
        let mut lines: Vec<Vec<PathPoint>> = Vec::new();

        let mut edges: Vec<&PathEdge> = network.edges.iter().collect();
        edges.sort_by_key(|e| e.id.to_string());
        for edge in &edges {
            let (Some(start), Some(end), Some(profile)) = (
                nodes.get(&edge.start),
                nodes.get(&edge.end),
                profiles.get(edge.profile_id.as_str()),
            ) else {
                warnings.push(PathWarning::general(
                    Severity::Error,
                    "INVALID_EDGE",
                    format!("Edge {} references missing path data", edge.id),
                ));
                continue;
            };
            let mut drawn = vec![start.point()];
            drawn.extend_from_slice(&edge.control_points);
            drawn.push(end.point());
            if drawn.iter().any(|p| {
                p.x < f64::from(env.bounds.min_x)
                    || p.x >= f64::from(env.bounds.max_x)
                    || p.y < f64::from(env.bounds.min_y)
                    || p.y >= f64::from(env.bounds.max_y)
            }) {
                warnings.push(PathWarning::general(
                    Severity::Warning,
                    "OUT_OF_BOUNDS",
                    format!(
                        "Part of edge {} lies outside the editable map bounds",
                        edge.id
                    ),
                ));
            }
            let plane = start.plane;
            let mut line =
                smooth_and_resample(&drawn, settings.smoothness, settings.sample_spacing);
            // Join each end onto a path of the same overlay close by.
            if profile.auto_connect
                && settings.auto_connect_policy != AutoConnectPolicy::Off
                && settings.snap_radius != 0.0
                && !env.existing_paints.is_empty()
                && !line.is_empty()
            {
                let first = snap_target(
                    line[0],
                    plane,
                    profile,
                    settings.snap_radius,
                    &env.existing_paints,
                    &atlas,
                );
                let last = snap_target(
                    line[line.len() - 1],
                    plane,
                    profile,
                    settings.snap_radius,
                    &env.existing_paints,
                    &atlas,
                );
                let mut joined = line.clone();
                if let Some(first) = first
                    && first.distance_to(joined[0]) > 1.0e-6
                {
                    joined.insert(0, first);
                }
                if let Some(last) = last
                    && last.distance_to(joined[joined.len() - 1]) > 1.0e-6
                {
                    joined.push(last);
                }
                line = resample(&joined, settings.sample_spacing);
            }
            let base_width = profile.base_width * edge.width_scale * settings.width_scale;
            let start_width = base_width * start.width_scale;
            let end_width = base_width * end.width_scale;
            let widest = start_width.max(end_width);
            let max_slope = profile.maximum_slope.min(settings.maximum_slope);
            let clearance = settings.clearance
                + profile.obstacle_clearance
                + (widest * 0.5 - 0.5).max(0.0)
                + profile.centerline_wander * settings.noise_strength;
            let avoids = matches!(
                settings.obstacle_policy,
                ObstaclePolicy::AvoidSolids | ObstaclePolicy::AvoidAll
            );
            if edge.auto_route && avoids && profile.reroute {
                let routed = route(&line, plane, env, settings, clearance, max_slope);
                if routed.succeeded() {
                    line = routed.points;
                    if routed.rerouted {
                        warnings.push(PathWarning::general(
                            Severity::Info,
                            "REROUTED",
                            format!("Edge {} was rerouted around scene obstacles", edge.id),
                        ));
                    }
                } else {
                    warnings.push(PathWarning::general(
                        Severity::Warning,
                        "ROUTE_BLOCKED",
                        format!(
                            "Edge {} could not be routed around obstacles and follows the authored centerline instead: {}",
                            edge.id, routed.failure
                        ),
                    ));
                }
            } else if edge.auto_route
                && settings.obstacle_policy == ObstaclePolicy::WarnOnly
                && line.iter().any(|p| {
                    blocked_at(
                        p.x.floor() as i32,
                        p.y.floor() as i32,
                        plane,
                        env,
                        ObstaclePolicy::WarnOnly,
                        0.0,
                        max_slope,
                    )
                })
            {
                warnings.push(PathWarning::general(
                    Severity::Warning,
                    "OBSTACLE_CONFLICT",
                    format!(
                        "Edge {} intersects a solid object or excessive slope",
                        edge.id
                    ),
                ));
            }

            let edge_seed = network.seed ^ id_mix(edge.id);
            // The centre line wanders sideways, fading in and out at the ends.
            let wander = profile.centerline_wander * settings.noise_strength;
            if line.len() >= 3 && wander != 0.0 {
                let noise = SeededPerlinNoise::new(mix64(edge_seed ^ settings.seed));
                let scale = profile.wander_scale * settings.noise_scale;
                let total = length_of(&line);
                let mut along = 0.0;
                let mut wandered = vec![line[0]];
                for i in 1..line.len() - 1 {
                    along += line[i - 1].distance_to(line[i]);
                    let (previous, next) = (line[i - 1], line[i + 1]);
                    let (tx, ty) = (next.x - previous.x, next.y - previous.y);
                    let length = tx.hypot(ty);
                    if length < 1.0e-9 {
                        wandered.push(line[i]);
                    } else {
                        let fade = (along.min(total - along) / (scale * 0.5).max(1.0)).min(1.0);
                        let offset =
                            noise.fbm(along / scale, 0.371, profile.noise_octaves) * wander * fade;
                        wandered.push(PathPoint::new(
                            line[i].x - ty / length * offset,
                            line[i].y + tx / length * offset,
                        ));
                    }
                }
                wandered.push(line[line.len() - 1]);
                line = resample(&wandered, settings.sample_spacing);
            }

            let mut branch = network_branch(
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
        }

        // A junction of three or more edges of one profile gets a round pad.
        let mut junctions: BTreeMap<(u64, String), Vec<&PathEdge>> = BTreeMap::new();
        for edge in &edges {
            junctions
                .entry((edge.start, edge.profile_id.clone()))
                .or_default()
                .push(edge);
            junctions
                .entry((edge.end, edge.profile_id.clone()))
                .or_default()
                .push(edge);
        }
        for ((node_id, profile_id), joined) in &junctions {
            if joined.len() < 3 {
                continue;
            }
            let (Some(node), Some(profile)) =
                (nodes.get(node_id), profiles.get(profile_id.as_str()))
            else {
                continue;
            };
            let radius = joined
                .iter()
                .map(|e| {
                    profile.base_width * e.width_scale * node.width_scale * settings.width_scale
                })
                .fold(0.0, f64::max)
                * 0.68
                + 0.9;
            let mask_blocked =
                masks_overlap_policy(settings) && joined.iter().any(|e| e.auto_route);
            let centre = node.point();
            let max_slope = profile.maximum_slope.min(settings.maximum_slope);
            for ty in (centre.y - radius).floor() as i32..(centre.y + radius).ceil() as i32 {
                for tx in (centre.x - radius).floor() as i32..(centre.x + radius).ceil() as i32 {
                    if !env.bounds.contains(tx, ty)
                        || (mask_blocked
                            && blocked_at(
                                tx,
                                ty,
                                node.plane,
                                env,
                                settings.obstacle_policy,
                                0.0,
                                max_slope,
                            ))
                    {
                        continue;
                    }
                    let key = (tx, ty, node.plane, profile.id.clone());
                    let cell = cells
                        .entry(key.clone())
                        .or_insert_with(|| Cell::new(profile, samples, 0));
                    for sy in 0..samples {
                        for sx in 0..samples {
                            let px = f64::from(tx) + (sx as f64 + 0.5) / samples as f64;
                            let py = f64::from(ty) + (sy as f64 + 0.5) / samples as f64;
                            if (px - centre.x).hypot(py - centre.y) <= radius {
                                cell.bits.set(sy * samples + sx);
                            }
                        }
                    }
                    if cell.bits.is_empty() {
                        cells.remove(&key);
                    }
                }
            }
        }

        finish(network, settings, env, &atlas, cells, warnings, lines)
    }
}

/// Stamp one edge's centre line onto the cells it passes over.
#[allow(clippy::too_many_arguments)]
fn stamp(
    line: &[PathPoint],
    plane: u8,
    profile: &PathProfile,
    settings: &PathSettings,
    env: &PathEnvironment,
    edge_seed: i64,
    start_width: f64,
    end_width: f64,
    mask_blocked: bool,
    cells: &mut BTreeMap<CellKey, Cell>,
) {
    if line.len() < 2 {
        return;
    }
    let samples = settings.supersample as usize;
    let width_noise = SeededPerlinNoise::new(mix64(edge_seed ^ 95_972_190_603_340));
    let edge_noise = SeededPerlinNoise::new(mix64(edge_seed ^ 297_498_264_908));
    let falloff_noise = SeededPerlinNoise::new(mix64(edge_seed ^ 6_288_504_276_297_992_005));
    let width_noise_scale = profile.width_noise_scale * settings.noise_scale;
    let edge_noise_scale = profile.edge_noise_scale * settings.noise_scale;
    let total = length_of(line);
    let mut travelled = 0.0;
    let start_cap = cap_direction(line[0], line[1]);
    let end_cap = cap_direction(line[line.len() - 1], line[line.len() - 2]);
    let max_slope = profile.maximum_slope.min(settings.maximum_slope);
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = a.distance_to(b);
        if length < 1.0e-9 {
            continue;
        }
        let t0 = travelled / total;
        let t1 = (travelled + length) / total;
        let width = lerp(start_width, end_width, t0).max(lerp(start_width, end_width, t1))
            * (1.0 + profile.width_variation);
        let falloff = profile.edge_falloff * (0.2 + width * 0.25).min(0.75);
        let reach =
            width * 0.5 + profile.edge_noise * settings.noise_strength + falloff * 0.5 + 0.125;
        let start_cap = cap_reaches(start_cap, a, b, reach)
            .then_some(start_cap)
            .flatten();
        let end_cap = cap_reaches(end_cap, a, b, reach)
            .then_some(end_cap)
            .flatten();
        let (min_x, min_y) = (
            (a.x.min(b.x) - reach).floor() as i32,
            (a.y.min(b.y) - reach).floor() as i32,
        );
        let (max_x, max_y) = (
            (a.x.max(b.x) + reach).ceil() as i32,
            (a.y.max(b.y) + reach).ceil() as i32,
        );
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let length_squared = dx * dx + dy * dy;
        for ty in min_y..max_y {
            for tx in min_x..max_x {
                if !env.bounds.contains(tx, ty) {
                    continue;
                }
                if mask_blocked
                    && blocked_at(tx, ty, plane, env, settings.obstacle_policy, 0.0, max_slope)
                {
                    continue;
                }
                let key = (tx, ty, plane, profile.id.clone());
                let cell = cells
                    .entry(key.clone())
                    .or_insert_with(|| Cell::new(profile, samples, edge_seed));
                for sy in 0..samples {
                    for sx in 0..samples {
                        if cell.bits.get(sy * samples + sx) {
                            continue;
                        }
                        let px = f64::from(tx) + (sx as f64 + 0.5) / samples as f64;
                        let py = f64::from(ty) + (sy as f64 + 0.5) / samples as f64;
                        let u = if length_squared == 0.0 {
                            0.0
                        } else {
                            ((px - a.x) * dx + (py - a.y) * dy) / length_squared
                        };
                        // Past either end the cap decides: nothing is stamped behind it.
                        if (u < 0.0 && behind_cap(px, py, start_cap))
                            || (u > 1.0 && behind_cap(px, py, end_cap))
                        {
                            continue;
                        }
                        let uc = u.clamp(0.0, 1.0);
                        let distance = (px - (a.x + uc * dx)).hypot(py - (a.y + uc * dy));
                        if distance > reach {
                            continue;
                        }
                        let along = travelled + uc * length;
                        let local_width = lerp(start_width, end_width, along / total)
                            * (1.0
                                + profile.width_variation
                                    * width_noise.fbm(
                                        along / width_noise_scale,
                                        0.619,
                                        profile.noise_octaves,
                                    ));
                        let rough = profile.edge_noise
                            * settings.noise_strength
                            * edge_noise.fbm(
                                px / edge_noise_scale,
                                py / edge_noise_scale,
                                profile.noise_octaves,
                            );
                        let radius = (local_width * 0.5 + rough).max(0.05);
                        let band = profile.edge_falloff * (0.2 + local_width * 0.25).min(0.75);
                        let inside = if band < 1.0e-9 {
                            distance <= radius
                        } else {
                            let inner = (radius - band * 0.5).max(0.05);
                            if distance <= inner {
                                true
                            } else {
                                let outer = radius + band * 0.5;
                                if distance > outer {
                                    false
                                } else {
                                    let scale = (edge_noise_scale * 1.5).max(0.75);
                                    let noise = falloff_noise.fbm(
                                        px / scale,
                                        py / scale,
                                        profile.noise_octaves,
                                    );
                                    let strength = settings.noise_strength.min(1.0);
                                    (outer - distance) / band >= 0.5 + noise * 0.3 * strength
                                }
                            }
                        };
                        if inside {
                            cell.bits.set(sy * samples + sx);
                            cell.sum_along += along;
                            cell.count += 1;
                        }
                    }
                }
                if cell.bits.is_empty() {
                    cells.remove(&key);
                }
            }
        }
        travelled += length;
    }
}

/// From the stamped cells to paints: pick the winner per tile, keep what is already there, fill
/// notches, choose shapes with the grammar, repair seams, check the result.
fn finish(
    network: &PathNetwork,
    settings: &PathSettings,
    env: &PathEnvironment,
    atlas: &OverlayShapeAtlas,
    cells: BTreeMap<CellKey, Cell>,
    mut warnings: Vec<PathWarning>,
    lines: Vec<Vec<PathPoint>>,
) -> PathPlan {
    let resolution = atlas.resolution();
    let cell_list: Vec<(CellKey, Cell)> = cells.into_iter().collect();
    // One cell per tile: the one covering most of it (the lower profile id on a tie).
    let mut best: HashMap<Tile, usize> = HashMap::new();
    for (index, (key, cell)) in cell_list.iter().enumerate() {
        let tile = (key.0, key.1, key.2);
        let better = match best.get(&tile) {
            None => true,
            Some(other) => {
                let (other_key, other_cell) = &cell_list[*other];
                let (mine, theirs) = (cell.bits.cardinality(), other_cell.bits.cardinality());
                mine > theirs || (mine == theirs && key.3 < other_key.3)
            }
        };
        if better {
            best.insert(tile, index);
        }
    }
    let existing: HashMap<Tile, &TilePaint> = env
        .existing_paints
        .iter()
        .map(|p| ((p.tile_x, p.tile_y, p.plane), p))
        .collect();
    let mut order: Vec<Tile> = best.keys().copied().collect();
    order.sort_by_key(tile_order);

    let mut strong: BTreeMap<(u8, i32, i32), Chosen> = BTreeMap::new();
    let mut weak: BTreeMap<(u8, i32, i32), Chosen> = BTreeMap::new();
    for tile in &order {
        let index = best[tile];
        let (key, cell) = &cell_list[index];
        if settings.preserve_existing
            && let Some(paint) = existing.get(tile)
        {
            if !cell.profile.compatible(paint.overlay_id) {
                warnings.push(PathWarning::at(
                    Severity::Warning,
                    "EXISTING_OVERLAY",
                    "An incompatible existing overlay was preserved",
                    *tile,
                ));
            }
            continue;
        }
        let coverage = f64::from(cell.bits.cardinality()) / (resolution * resolution) as f64;
        let chosen = Chosen {
            key: key.clone(),
            cell_index: index,
            coverage,
        };
        let target = if coverage < 0.12 + cell.profile.edge_falloff * 0.1 {
            &mut weak
        } else {
            &mut strong
        };
        target.insert(tile_order(tile), chosen);
    }
    // A thinly covered tile only counts when three of its four sides are path (it fills a notch).
    // Keep the discarded candidates around so a single low-coverage bridge can repair an actual
    // break without changing normal connected Terraini output.
    let continuity_candidates = weak.clone();
    let strong_keys: HashSet<Tile> = strong.keys().map(|k| (k.1, k.2, k.0)).collect();
    weak.retain(|k, _| {
        let tile = (k.1, k.2, k.0);
        SIDES
            .iter()
            .filter(|side| strong_keys.contains(&side_step(tile, **side)))
            .count()
            >= 3
    });
    if settings.repair_continuity {
        repair_coverage_continuity(&strong, &mut weak, &continuity_candidates);
    }
    // The plan's tiles, in plane, x, y order.
    let mut merged: Vec<(Tile, Chosen)> = Vec::new();
    for tile in &order {
        let k = tile_order(tile);
        if let Some(chosen) = strong.remove(&k).or_else(|| weak.remove(&k)) {
            merged.push((*tile, chosen));
        }
    }
    let index_of: HashMap<Tile, usize> = merged
        .iter()
        .enumerate()
        .map(|(i, (t, _))| (*t, i))
        .collect();
    let cell_of = |chosen: &Chosen| &cell_list[chosen.cell_index].1;

    // Does the neighbour count as path for this profile?
    let counts = |profile: &PathProfile, tile: Tile| -> bool {
        if let Some(i) = index_of.get(&tile) {
            profile.compatible(cell_of(&merged[*i].1).profile.primary_overlay_id)
        } else {
            existing
                .get(&tile)
                .is_some_and(|paint| profile.compatible(paint.overlay_id))
        }
    };

    // Each tile's eight-neighbour mask, and the tiles of one neighbourhood grouped.
    let mut masks: Vec<u32> = Vec::with_capacity(merged.len());
    let mut groups: BTreeMap<(u32, String), Vec<usize>> = BTreeMap::new();
    for (i, (tile, chosen)) in merged.iter().enumerate() {
        let profile = &cell_of(chosen).profile;
        let mut mask = 0_u32;
        for (bit, (dx, dy)) in NEIGHBOURS.iter().enumerate() {
            if counts(profile, (tile.0 + dx, tile.1 + dy, tile.2)) {
                mask |= 1 << bit;
            }
        }
        masks.push(mask);
        groups
            .entry((grammar::canonical(mask), chosen.key.3.clone()))
            .or_default()
            .push(i);
    }
    // A hash of each tile's sample pattern turned to the neighbourhood's canonical rotation: ties
    // between tiles that look alike are broken by it, the same way every run.
    let hashes: Vec<i64> = merged
        .iter()
        .enumerate()
        .map(|(i, (_, chosen))| {
            let turns = grammar::canonical_turns(masks[i]);
            let bits = &cell_of(chosen).bits;
            let mut words = vec![0_u64; (resolution * resolution).div_ceil(64)];
            for bit in bits.iter() {
                let (mut col, mut row) = (bit % resolution, bit / resolution);
                for _ in 0..turns {
                    let next = resolution - 1 - col;
                    col = row;
                    row = next;
                }
                let index = row * resolution + col;
                words[index >> 6] |= 1 << (index & 63);
            }
            words
                .iter()
                .fold(0_i64, |hash, word| mix64(hash ^ *word as i64))
        })
        .collect();
    let mut shapes: Vec<u32> = vec![0; merged.len()];
    for list in groups.values_mut() {
        list.sort_by(|a, b| {
            merged[*b]
                .1
                .coverage
                .total_cmp(&merged[*a].1.coverage)
                .then(hashes[*a].cmp(&hashes[*b]))
                .then(tile_order(&merged[*a].0).cmp(&tile_order(&merged[*b].0)))
        });
        let count = list.len() as f64;
        for (position, i) in list.iter().enumerate() {
            let draw = (position as f64 + 0.5) / count
                + 0.35 * (grammar::mean_area(masks[*i]) - merged[*i].1.coverage);
            shapes[*i] = grammar::shape_for(masks[*i], draw);
        }
    }

    // Where two tiles' shapes meet at a seam that does not line up, one becomes a whole tile.
    let mut whole: Vec<usize> = Vec::new();
    for (i, (tile, chosen)) in merged.iter().enumerate() {
        let profile = &cell_of(chosen).profile;
        for side in SIDES {
            let next = side_step(*tile, side);
            let neighbour = index_of.get(&next).copied();
            let paint = if neighbour.is_none() {
                existing.get(&next)
            } else {
                None
            };
            let counts_here = match (neighbour, paint) {
                (Some(j), _) => {
                    profile.compatible(cell_of(&merged[j].1).profile.primary_overlay_id)
                }
                (None, Some(p)) => profile.compatible(p.overlay_id),
                _ => false,
            };
            if !counts_here {
                continue;
            }
            let mine = atlas.edge_mask((shapes[i] >> 2) as u8, (shapes[i] & 3) as u8, side);
            let theirs = match (neighbour, paint) {
                (Some(j), _) => atlas.edge_mask(
                    (shapes[j] >> 2) as u8,
                    (shapes[j] & 3) as u8,
                    opposite(side),
                ),
                (None, Some(p)) => atlas.edge_mask(p.shape, p.orientation, opposite(side)),
                _ => 0,
            };
            if seam_mismatch(mine, theirs) {
                let loser = match neighbour {
                    Some(j) => {
                        let (a, b) = (merged[i].1.coverage, merged[j].1.coverage);
                        let keep_mine = if a != b {
                            a >= b
                        } else {
                            tile_order(tile) <= tile_order(&merged[j].0)
                        };
                        if keep_mine { j } else { i }
                    }
                    None => i,
                };
                whole.push(loser);
            }
        }
    }
    for i in whole {
        shapes[i] = 0;
    }
    if settings.repair_continuity {
        repair_shape_continuity(&merged, &index_of, &mut shapes, atlas);
    }

    // The paints.
    let mut paints: Vec<TilePaint> = Vec::with_capacity(merged.len());
    for (i, (tile, chosen)) in merged.iter().enumerate() {
        let cell = cell_of(chosen);
        let seed = settings.seed ^ network.seed ^ cell.seed;
        let along = if cell.count == 0 {
            0.0
        } else {
            cell.sum_along / f64::from(cell.count)
        };
        paints.push(TilePaint {
            tile_x: tile.0,
            tile_y: tile.1,
            plane: tile.2,
            overlay_id: choose_overlay(&cell.profile, seed, along, tile.2),
            shape: (shapes[i] >> 2) as u8,
            orientation: (shapes[i] & 3) as u8,
            coverage: chosen.coverage,
            profile_id: chosen.key.3.clone(),
        });
    }
    paints.sort_by(|a, b| {
        (a.plane, a.tile_x, a.tile_y, &a.profile_id).cmp(&(
            b.plane,
            b.tile_x,
            b.tile_y,
            &b.profile_id,
        ))
    });
    check_topology(&paints, atlas, &mut warnings);

    let affected = if paints.is_empty() {
        TileBounds::empty()
    } else {
        TileBounds {
            min_x: paints.iter().map(|p| p.tile_x).min().unwrap_or(0),
            min_y: paints.iter().map(|p| p.tile_y).min().unwrap_or(0),
            max_x: paints.iter().map(|p| p.tile_x).max().unwrap_or(0) + 1,
            max_y: paints.iter().map(|p| p.tile_y).max().unwrap_or(0) + 1,
        }
    };
    PathPlan {
        paints,
        warnings,
        affected,
        lines,
    }
}

/// Warn about seams that do not line up and pieces that are not joined to the rest.
fn check_topology(
    paints: &[TilePaint],
    atlas: &OverlayShapeAtlas,
    warnings: &mut Vec<PathWarning>,
) {
    let by_tile: HashMap<Tile, &TilePaint> = paints
        .iter()
        .map(|p| ((p.tile_x, p.tile_y, p.plane), p))
        .collect();
    for paint in paints {
        let tile = (paint.tile_x, paint.tile_y, paint.plane);
        for (side, towards) in [(EAST, WEST), (SOUTH, NORTH)] {
            let next = side_step(tile, side);
            if let Some(other) = by_tile.get(&next)
                && other.profile_id == paint.profile_id
                && seam_mismatch(
                    atlas.edge_mask(paint.shape, paint.orientation, side),
                    atlas.edge_mask(other.shape, other.orientation, towards),
                )
            {
                warnings.push(PathWarning::at(
                    Severity::Warning,
                    "TOPOLOGY_SEAM",
                    "Adjacent generated overlay shapes have a mismatched seam",
                    tile,
                ));
            }
        }
    }
    // Pieces: tiles of one profile and plane joined side to side where their shapes meet.
    let mut groups: BTreeMap<(String, u8), Vec<Tile>> = BTreeMap::new();
    for paint in paints {
        groups
            .entry((paint.profile_id.clone(), paint.plane))
            .or_default()
            .push((paint.tile_x, paint.tile_y, paint.plane));
    }
    for tiles in groups.values_mut() {
        tiles.sort_by_key(tile_order);
        let mut remaining: Vec<Tile> = tiles.clone();
        let mut first = true;
        while let Some(&seed) = remaining.first() {
            if !first {
                warnings.push(PathWarning::at(
                    Severity::Warning,
                    "TOPOLOGY_COMPONENT",
                    "Generated path contains a separate or corner-only component",
                    seed,
                ));
            }
            first = false;
            let mut queue = VecDeque::new();
            remaining.retain(|t| *t != seed);
            queue.push_back(seed);
            while let Some(current) = queue.pop_front() {
                let paint = by_tile[&current];
                for side in SIDES {
                    let next = side_step(current, side);
                    let Some(other) = by_tile.get(&next) else {
                        continue;
                    };
                    if other.profile_id != paint.profile_id {
                        continue;
                    }
                    let meets = atlas.edge_mask(paint.shape, paint.orientation, side)
                        & atlas.edge_mask(other.shape, other.orientation, opposite(side))
                        != 0;
                    if meets && let Some(position) = remaining.iter().position(|t| *t == next) {
                        remaining.remove(position);
                        queue.push_back(next);
                    }
                }
            }
        }
    }
}

/// A stable 64-bit hash of a string (ids are derived from content so a path plans the same way
/// each time).
pub fn fnv(text: &str) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325_u64;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01B3);
    }
    hash
}

/// What an edge id contributes to its noise seed (the reference folds the two halves of a UUID
/// into one number; ids here are that number).
fn id_mix(id: u64) -> i64 {
    id as i64
}

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
    fn continuity_repair_promotes_a_single_weak_cardinal_bridge() {
        let chosen = |x: i32, y: i32, coverage: f64| Chosen {
            key: (x, y, 0, "p".to_owned()),
            cell_index: 0,
            coverage,
        };
        let mut strong = BTreeMap::new();
        strong.insert((0, 0, 0), chosen(0, 0, 1.0));
        strong.insert((0, 1, 1), chosen(1, 1, 1.0));
        let weak = BTreeMap::new();
        let mut candidates = BTreeMap::new();
        candidates.insert((0, 1, 0), chosen(1, 0, 0.08));
        candidates.insert((0, 0, 1), chosen(0, 1, 0.06));

        let mut repaired = weak;
        assert_eq!(
            repair_coverage_continuity(&strong, &mut repaired, &candidates),
            1
        );
        assert!(repaired.contains_key(&(0, 1, 0)));
        let labels = coverage_component_labels(&strong, &repaired);
        let unique: HashSet<_> = labels.values().copied().collect();
        assert_eq!(unique.len(), 1);
    }

    #[test]
    fn shape_continuity_repair_joins_side_adjacent_disconnected_shapes() {
        let atlas = OverlayShapeAtlas::new(8);
        let mut pair = None;
        'find: for a_shape in 0..12_u8 {
            for a_orientation in 0..4_u8 {
                for b_shape in 0..12_u8 {
                    for b_orientation in 0..4_u8 {
                        if atlas.edge_mask(a_shape, a_orientation, EAST)
                            & atlas.edge_mask(b_shape, b_orientation, WEST)
                            == 0
                        {
                            pair = Some((a_shape, a_orientation, b_shape, b_orientation));
                            break 'find;
                        }
                    }
                }
            }
        }
        let (a_shape, a_orientation, b_shape, b_orientation) = pair.expect("shape pair");
        let chosen = |x: i32| Chosen {
            key: (x, 0, 0, "p".to_owned()),
            cell_index: 0,
            coverage: 1.0,
        };
        let merged = vec![((0, 0, 0), chosen(0)), ((1, 0, 0), chosen(1))];
        let index_of = HashMap::from([((0, 0, 0), 0), ((1, 0, 0), 1)]);
        let mut shapes = vec![
            (u32::from(a_shape) << 2) | u32::from(a_orientation),
            (u32::from(b_shape) << 2) | u32::from(b_orientation),
        ];
        let before = shape_component_labels(&merged, &index_of, &shapes, &atlas);
        assert_ne!(before[0], before[1]);
        assert_eq!(
            repair_shape_continuity(&merged, &index_of, &mut shapes, &atlas),
            1
        );
        let after = shape_component_labels(&merged, &index_of, &shapes, &atlas);
        assert_eq!(after[0], after[1]);
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
