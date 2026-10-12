//! Routing a path round obstacles: A* over tiles inside a corridor round the drawn line, then the
//! route is pulled straight wherever it can see along it.

use super::model::{Occupancy, PathEnvironment, PathPoint, TileBounds};
use super::resample::resample;
use super::settings::{ObstaclePolicy, PathSettings};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

type Cell = (i32, i32);

const STEPS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// What routing a path came to.
pub struct RouteResult {
    pub points: Vec<PathPoint>,
    pub rerouted: bool,
    pub failure: String,
}

impl RouteResult {
    pub fn succeeded(&self) -> bool {
        !self.points.is_empty()
    }
}

fn cell_of(point: PathPoint) -> Cell {
    (point.x.floor() as i32, point.y.floor() as i32)
}

/// Whether a tile (and the clearance round it) cannot hold the path.
pub fn blocked_at(
    x: i32,
    y: i32,
    plane: u8,
    env: &PathEnvironment,
    policy: ObstaclePolicy,
    clearance: f64,
    max_slope: f64,
) -> bool {
    if !env.bounds.contains(x, y) {
        return true;
    }
    let reach = clearance.max(0.0).ceil() as i32;
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            if f64::from(dx).hypot(f64::from(dy)) > clearance + 0.75 {
                continue;
            }
            let (tx, ty) = (x + dx, y + dy);
            if !env.bounds.contains(tx, ty) {
                return true;
            }
            let occupancy = env.cost_grid.occupancy(tx, ty, plane);
            let blocks = match policy {
                ObstaclePolicy::AvoidAll => occupancy != Occupancy::Clear,
                ObstaclePolicy::AvoidSolids | ObstaclePolicy::Mask | ObstaclePolicy::WarnOnly => {
                    occupancy == Occupancy::Solid
                }
                ObstaclePolicy::Ignore => false,
            };
            if blocks || env.cost_grid.slope(tx, ty, plane) > max_slope {
                return true;
            }
        }
    }
    false
}

struct Router<'a> {
    env: &'a PathEnvironment,
    settings: &'a PathSettings,
    plane: u8,
    clearance: f64,
    max_slope: f64,
    blocked: HashMap<Cell, bool>,
}

impl Router<'_> {
    fn blocked(&mut self, cell: Cell) -> bool {
        if let Some(known) = self.blocked.get(&cell) {
            return *known;
        }
        let value = blocked_at(
            cell.0,
            cell.1,
            self.plane,
            self.env,
            self.settings.obstacle_policy,
            self.clearance,
            self.max_slope,
        );
        self.blocked.insert(cell, value);
        value
    }

    fn edge_blocked(&self, a: Cell, b: Cell) -> bool {
        let grid = &self.env.cost_grid;
        grid.edge_blocked(a.0, a.1, b.0, b.1, self.plane)
            || grid.edge_blocked(b.0, b.1, a.0, a.1, self.plane)
    }

    /// Whether a single step between neighbouring tiles is allowed (no wall in the way, and for a
    /// diagonal, neither tile it cuts across is blocked).
    fn step_allowed(&mut self, from: Cell, to: Cell) -> bool {
        if self.edge_blocked(from, to) {
            return false;
        }
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        if dx != 0 && dy != 0 {
            let a = (from.0 + dx, from.1);
            let b = (from.0, from.1 + dy);
            if self.blocked(a) || self.blocked(b) {
                return false;
            }
            let grid = &self.env.cost_grid;
            if grid.edge_blocked(from.0, from.1, a.0, a.1, self.plane)
                || grid.edge_blocked(from.0, from.1, b.0, b.1, self.plane)
            {
                return false;
            }
        }
        true
    }

    /// Whether the straight line between two points crosses only free tiles by allowed steps.
    fn line_of_sight(&mut self, a: PathPoint, b: PathPoint) -> bool {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let steps = (dx.abs().max(dy.abs()) * 4.0).ceil() as i32 + 1;
        let mut previous = cell_of(a);
        for s in 1..=steps {
            let t = f64::from(s) / f64::from(steps);
            let cell = cell_of(PathPoint::new(a.x + dx * t, a.y + dy * t));
            if cell != previous {
                if (cell.0 - previous.0).abs() > 1 || (cell.1 - previous.1).abs() > 1 {
                    return false;
                }
                if self.blocked(cell) || !self.step_allowed(previous, cell) {
                    return false;
                }
                previous = cell;
            }
        }
        true
    }
}

fn octile(a: Cell, b: Cell) -> f64 {
    let dx = f64::from((a.0 - b.0).abs());
    let dy = f64::from((a.1 - b.1).abs());
    dx.max(dy) + (std::f64::consts::SQRT_2 - 1.0) * dx.min(dy)
}

/// Distance from a tile's centre to the line of `points`.
fn corridor_distance(cell: Cell, points: &[PathPoint]) -> f64 {
    let (cx, cy) = (f64::from(cell.0) + 0.5, f64::from(cell.1) + 0.5);
    let mut best = f64::INFINITY;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let gap_x = 0.0_f64.max((a.x.min(b.x) - cx).max(cx - a.x.max(b.x)));
        let gap_y = 0.0_f64.max((a.y.min(b.y) - cy).max(cy - a.y.max(b.y)));
        if gap_x > best || gap_y > best || gap_x.hypot(gap_y) > best {
            continue;
        }
        let (sx, sy) = (b.x - a.x, b.y - a.y);
        let length_squared = sx * sx + sy * sy;
        let distance = if length_squared == 0.0 {
            (cx - a.x).hypot(cy - a.y)
        } else {
            let t = (((cx - a.x) * sx + (cy - a.y) * sy) / length_squared).clamp(0.0, 1.0);
            (cx - (a.x + sx * t)).hypot(cy - (a.y + sy * t))
        };
        best = best.min(distance);
    }
    best
}

#[derive(PartialEq)]
struct Open {
    f: f64,
    cell: Cell,
}

impl Eq for Open {}

impl Ord for Open {
    /// A max-heap, so the order is reversed: lowest `f`, then lowest y, then lowest x first.
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f
            .total_cmp(&self.f)
            .then_with(|| other.cell.1.cmp(&self.cell.1))
            .then_with(|| other.cell.0.cmp(&self.cell.0))
    }
}

impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Route `desired` round whatever blocks it. The part of the line that is blocked is replaced; the
/// rest is kept as drawn.
pub fn route(
    desired: &[PathPoint],
    plane: u8,
    env: &PathEnvironment,
    settings: &PathSettings,
    clearance: f64,
    max_slope: f64,
) -> RouteResult {
    let keep = |rerouted: bool| RouteResult {
        points: desired.to_vec(),
        rerouted,
        failure: String::new(),
    };
    let failed = |reason: &str| RouteResult {
        points: Vec::new(),
        rerouted: false,
        failure: reason.to_owned(),
    };
    if desired.len() < 2 {
        return keep(false);
    }
    let mut router = Router {
        env,
        settings,
        plane,
        clearance,
        max_slope,
        blocked: HashMap::new(),
    };
    // The stretch of the line that is blocked, or crosses a wall.
    let mut first: Option<usize> = None;
    let mut last = 0_usize;
    let mut previous = cell_of(desired[0]);
    for (i, point) in desired.iter().enumerate() {
        let cell = cell_of(*point);
        let is_blocked = router.blocked(cell);
        if i > 0 && cell != previous && router.edge_blocked(previous, cell) {
            first.get_or_insert(i - 1);
            last = i;
        }
        if is_blocked {
            first.get_or_insert(i);
            last = i;
        }
        previous = cell;
    }
    let Some(mut first) = first else {
        return keep(false);
    };
    // Widen it to the nearest free points on either side.
    while first > 0 && router.blocked(cell_of(desired[first])) {
        first -= 1;
    }
    while last < desired.len() - 1 && router.blocked(cell_of(desired[last])) {
        last += 1;
    }
    let start = cell_of(desired[first]);
    let goal = cell_of(desired[last]);
    if router.blocked(start) || router.blocked(goal) {
        return failed("A path endpoint is blocked");
    }
    let stretch = &desired[first..=last];
    let corridor = settings.route_corridor;
    let min_x = stretch.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let min_y = stretch.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let max_x = stretch.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
    let max_y = stretch.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    let area = TileBounds::new(
        (min_x - corridor).floor() as i32,
        (min_y - corridor).floor() as i32,
        ((max_x + corridor).ceil() + 1.0) as i32,
        ((max_y + corridor).ceil() + 1.0) as i32,
    )
    .intersect(&env.bounds);

    let mut corridor_cache: HashMap<Cell, f64> = HashMap::new();
    let mut open = BinaryHeap::new();
    let mut cost: HashMap<Cell, f64> = HashMap::new();
    let mut came: HashMap<Cell, Cell> = HashMap::new();
    let mut closed: HashSet<Cell> = HashSet::new();
    cost.insert(start, 0.0);
    open.push(Open {
        f: octile(start, goal),
        cell: start,
    });
    while let Some(Open { cell: current, .. }) = open.pop() {
        if !closed.insert(current) {
            continue;
        }
        if current == goal {
            let mut cells = vec![current];
            let mut at = current;
            while let Some(from) = came.get(&at) {
                at = *from;
                cells.push(at);
            }
            cells.reverse();
            let mut points: Vec<PathPoint> = cells
                .iter()
                .map(|c| PathPoint::new(f64::from(c.0) + 0.5, f64::from(c.1) + 0.5))
                .collect();
            if points.len() < 2 {
                return keep(false);
            }
            points[0] = desired[first];
            let end = points.len() - 1;
            points[end] = desired[last];
            let pulled = pull_straight(&mut router, &points);
            let mut joined = Vec::with_capacity(desired.len() + pulled.len());
            joined.extend_from_slice(&desired[..first]);
            joined.extend(pulled);
            joined.extend_from_slice(&desired[last + 1..]);
            return RouteResult {
                points: resample(&joined, settings.sample_spacing),
                rerouted: true,
                failure: String::new(),
            };
        }
        for (dx, dy) in STEPS {
            let next = (current.0 + dx, current.1 + dy);
            if !area.contains(next.0, next.1) || closed.contains(&next) {
                continue;
            }
            let away = *corridor_cache
                .entry(next)
                .or_insert_with(|| corridor_distance(next, stretch));
            if away > corridor || router.blocked(next) || !router.step_allowed(current, next) {
                continue;
            }
            let step = if dx != 0 && dy != 0 {
                std::f64::consts::SQRT_2
            } else {
                1.0
            };
            let traversal = env.cost_grid.traversal_cost(next.0, next.1, plane);
            if !traversal.is_finite() || traversal < 0.0 {
                continue;
            }
            let slope = env.cost_grid.slope(next.0, next.1, plane).max(0.0);
            let through = cost[&current]
                + step * traversal.max(0.01)
                + slope * settings.slope_penalty
                + away * 0.12;
            if through + 1.0e-9 < cost.get(&next).copied().unwrap_or(f64::INFINITY) {
                cost.insert(next, through);
                came.insert(next, current);
                open.push(Open {
                    f: through + octile(next, goal),
                    cell: next,
                });
            }
        }
    }
    failed("No route exists inside the configured corridor")
}

/// Drop the route's corners wherever the line between the points either side can be walked.
fn pull_straight(router: &mut Router<'_>, points: &[PathPoint]) -> Vec<PathPoint> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut out = vec![points[0]];
    let mut i = 0;
    while i < points.len() - 1 {
        let mut furthest = i + 1;
        for k in i + 2..points.len() {
            if !router.line_of_sight(points[i], points[k]) {
                break;
            }
            furthest = k;
        }
        out.push(points[furthest]);
        i = furthest;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::model::CostGrid;
    use std::sync::Arc;

    /// A wall of solid tiles across the way.
    struct Wall;

    impl CostGrid for Wall {
        fn occupancy(&self, x: i32, y: i32, _plane: u8) -> Occupancy {
            if x == 10 && (-8..=8).contains(&y) {
                Occupancy::Solid
            } else {
                Occupancy::Clear
            }
        }
    }

    fn environment() -> PathEnvironment {
        PathEnvironment {
            bounds: TileBounds::new(-30, -30, 30, 30),
            cost_grid: Arc::new(Wall),
            existing_paints: Vec::new(),
        }
    }

    #[test]
    fn a_clear_line_is_left_alone() {
        let env = PathEnvironment::empty(TileBounds::new(-30, -30, 30, 30));
        let line = [PathPoint::new(0.5, 0.5), PathPoint::new(5.5, 0.5)];
        let result = route(&line, 0, &env, &PathSettings::default(), 0.0, f64::MAX);
        assert!(result.succeeded());
        assert!(!result.rerouted);
    }

    #[test]
    fn a_line_through_a_wall_goes_round_it() {
        // The planner hands the router a resampled line, so a blocked tile always holds a sample.
        let line = resample(&[PathPoint::new(2.5, 0.5), PathPoint::new(18.5, 0.5)], 0.25);
        let env = environment();
        let result = route(&line, 0, &env, &PathSettings::default(), 0.0, f64::MAX);
        assert!(result.succeeded() && result.rerouted, "{}", result.failure);
        assert_eq!(result.points[0], line[0]);
        assert!(result
            .points
            .iter()
            .all(|p| !blocked_at(p.x.floor() as i32, p.y.floor() as i32, 0, &env, ObstaclePolicy::AvoidSolids, 0.0, f64::MAX)));
    }

    #[test]
    fn a_blocked_end_fails_with_a_reason() {
        let line = resample(&[PathPoint::new(2.5, 0.5), PathPoint::new(10.5, 0.5)], 0.25);
        let result = route(&line, 0, &environment(), &PathSettings::default(), 0.0, f64::MAX);
        assert!(!result.succeeded());
        assert!(!result.failure.is_empty());
    }
}
