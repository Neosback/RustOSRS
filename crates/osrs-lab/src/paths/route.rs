//! Drawing a path between places: least-cost routes over the tiles the network likes, joined
//! into a network, and turned into the waypoints the path tool takes.

use super::grid::neighbours8;
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, VecDeque};

pub type Tile = (usize, usize);

/// What a step costs: the tile's own price times the length of the step. `price` is per tile;
/// `None` means the tile cannot be used.
pub fn route(
    width: usize,
    height: usize,
    price: &[Option<f32>],
    from: &[Tile],
    to: &BTreeSet<Tile>,
) -> Option<Vec<Tile>> {
    let mut best = vec![f32::INFINITY; width * height];
    let mut back = vec![usize::MAX; width * height];
    let mut heap = BinaryHeap::new();
    for &(x, y) in from {
        let at = y * width + x;
        if price[at].is_some() || to.contains(&(x, y)) {
            best[at] = 0.0;
            heap.push((Reverse(0_u32), at));
        }
    }
    while let Some((Reverse(cost), at)) = heap.pop() {
        let (x, y) = (at % width, at / width);
        if cost as f32 / 64.0 > best[at] + 1e-3 {
            continue;
        }
        if to.contains(&(x, y)) {
            let mut tiles = vec![(x, y)];
            let mut here = at;
            while back[here] != usize::MAX {
                here = back[here];
                tiles.push((here % width, here / width));
            }
            tiles.reverse();
            return Some(tiles);
        }
        for (nx, ny) in neighbours8(x, y, width, height) {
            let next = ny * width + nx;
            let goal = to.contains(&(nx, ny));
            let Some(p) = price[next].or(goal.then_some(1.0)) else {
                continue;
            };
            let diagonal = nx != x && ny != y;
            // Cutting a corner past a blocked tile is not allowed.
            if diagonal && (price[y * width + nx].is_none() || price[ny * width + x].is_none()) {
                continue;
            }
            let step = if diagonal {
                std::f32::consts::SQRT_2
            } else {
                1.0
            };
            let total = best[at] + p * step;
            if total < best[next] {
                best[next] = total;
                back[next] = at;
                heap.push((Reverse((total * 64.0) as u32), next));
            }
        }
    }
    None
}

/// Join `points` into one connected network of routes: start from the first, and each time
/// route the nearest unreached point to the network so far (so branches join, not cross).
pub fn network(
    width: usize,
    height: usize,
    price: &[Option<f32>],
    points: &[Tile],
) -> Vec<Vec<Tile>> {
    let Some(&first) = points.first() else {
        return Vec::new();
    };
    let mut tree: BTreeSet<Tile> = BTreeSet::from([first]);
    let mut left: Vec<Tile> = points[1..].to_vec();
    let mut routes = Vec::new();
    while !left.is_empty() {
        // The point closest to the network (straight-line), then the cheapest way to it.
        let (index, _) = left
            .iter()
            .enumerate()
            .map(|(i, p)| (i, tree.iter().map(|t| dist2(*p, *t)).min().unwrap_or(0)))
            .min_by_key(|(_, d)| *d)
            .unwrap_or((0, 0));
        let point = left.swap_remove(index);
        match route(width, height, price, &[point], &tree) {
            Some(tiles) => {
                tree.extend(tiles.iter().copied());
                routes.push(tiles);
            }
            None => {
                // Cannot be reached: leave it out and carry on.
            }
        }
    }
    routes
}

fn dist2(a: Tile, b: Tile) -> usize {
    let (dx, dy) = (a.0.abs_diff(b.0), a.1.abs_diff(b.1));
    dx * dx + dy * dy
}

/// 8-connected pieces of `mask` with at least `min` tiles.
pub fn pieces(mask: &[bool], width: usize, height: usize, min: usize) -> Vec<Vec<Tile>> {
    let mut seen = vec![false; width * height];
    let mut out = Vec::new();
    for start in 0..width * height {
        if !mask[start] || seen[start] {
            continue;
        }
        let mut piece = Vec::new();
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        while let Some(at) = queue.pop_front() {
            let (x, y) = (at % width, at / width);
            piece.push((x, y));
            for (nx, ny) in neighbours8(x, y, width, height) {
                let next = ny * width + nx;
                if mask[next] && !seen[next] {
                    seen[next] = true;
                    queue.push_back(next);
                }
            }
        }
        if piece.len() >= min {
            out.push(piece);
        }
    }
    out
}

/// Thin a piece of mask to a one-tile-wide line (Zhang-Suen).
pub fn skeleton(piece: &[Tile], width: usize, height: usize) -> BTreeSet<Tile> {
    let mut on: BTreeSet<Tile> = piece.iter().copied().collect();
    let get = |on: &BTreeSet<Tile>, x: i32, y: i32| {
        x >= 0 && y >= 0 && on.contains(&(x as usize, y as usize))
    };
    let _ = (width, height);
    loop {
        let mut changed = false;
        for pass in 0..2 {
            let mut remove = Vec::new();
            for &(ux, uy) in &on {
                let (x, y) = (ux as i32, uy as i32);
                // Neighbours clockwise from north.
                let n = [
                    get(&on, x, y + 1),
                    get(&on, x + 1, y + 1),
                    get(&on, x + 1, y),
                    get(&on, x + 1, y - 1),
                    get(&on, x, y - 1),
                    get(&on, x - 1, y - 1),
                    get(&on, x - 1, y),
                    get(&on, x - 1, y + 1),
                ];
                let count = n.iter().filter(|b| **b).count();
                if !(2..=6).contains(&count) {
                    continue;
                }
                let transitions = (0..8).filter(|i| !n[*i] && n[(*i + 1) % 8]).count();
                if transitions != 1 {
                    continue;
                }
                let (a, b) = if pass == 0 {
                    (n[0] && n[2] && n[4], n[2] && n[4] && n[6])
                } else {
                    (n[0] && n[2] && n[6], n[0] && n[4] && n[6])
                };
                if !a && !b {
                    remove.push((ux, uy));
                }
            }
            if !remove.is_empty() {
                changed = true;
                for t in remove {
                    on.remove(&t);
                }
            }
        }
        if !changed {
            break;
        }
    }
    on
}

/// The ends of a thin line (tiles with one neighbour); for a closed loop, two far-apart tiles.
/// At most `limit` points, chosen to be far from each other.
pub fn ends(line: &BTreeSet<Tile>, limit: usize) -> Vec<Tile> {
    let count = |t: Tile| {
        line.iter()
            .filter(|o| **o != t && o.0.abs_diff(t.0) <= 1 && o.1.abs_diff(t.1) <= 1)
            .count()
    };
    let mut ends: Vec<Tile> = line.iter().copied().filter(|t| count(*t) <= 1).collect();
    if ends.is_empty() {
        ends = line.iter().copied().take(1).collect();
        if let Some(&far) = line.iter().max_by_key(|t| dist2(**t, ends[0])) {
            ends.push(far);
        }
    }
    // Farthest-point selection down to `limit`.
    let mut chosen: Vec<Tile> = Vec::new();
    while chosen.len() < limit.min(ends.len()) {
        let next = ends
            .iter()
            .copied()
            .filter(|t| !chosen.contains(t))
            .max_by_key(|t| {
                chosen
                    .iter()
                    .map(|c| dist2(*t, *c))
                    .min()
                    .unwrap_or(usize::MAX)
            });
        match next {
            Some(t) => chosen.push(t),
            None => break,
        }
    }
    chosen
}

/// Waypoints along a route, simplified so only real turns remain (Douglas-Peucker), as tile
/// centres.
pub fn waypoints(tiles: &[Tile], tolerance: f64) -> Vec<[f64; 2]> {
    let points: Vec<[f64; 2]> = tiles
        .iter()
        .map(|(x, y)| [*x as f64 + 0.5, *y as f64 + 0.5])
        .collect();
    if points.len() <= 2 {
        return points;
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0, points.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (pa, pb) = (points[a], points[b]);
        let length = ((pb[0] - pa[0]).powi(2) + (pb[1] - pa[1]).powi(2))
            .sqrt()
            .max(1e-9);
        let far = (a + 1..b)
            .map(|i| {
                let d = ((pb[1] - pa[1]) * points[i][0] - (pb[0] - pa[0]) * points[i][1]
                    + pb[0] * pa[1]
                    - pb[1] * pa[0])
                    .abs()
                    / length;
                (i, d)
            })
            .max_by(|x, y| x.1.total_cmp(&y.1));
        if let Some((i, d)) = far
            && d > tolerance
        {
            keep[i] = true;
            stack.push((a, i));
            stack.push((i, b));
        }
    }
    points
        .into_iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(p, _)| p)
        .collect()
}

/// Tiles within `radius` (chessboard) of any tile in `routes`: the band a path of that width paints.
pub fn band(routes: &[Vec<Tile>], radius: usize, width: usize, height: usize) -> Vec<bool> {
    let mut out = vec![false; width * height];
    for tile in routes.iter().flatten() {
        for y in tile.1.saturating_sub(radius)..(tile.1 + radius + 1).min(height) {
            for x in tile.0.saturating_sub(radius)..(tile.0 + radius + 1).min(width) {
                out[y * width + x] = true;
            }
        }
    }
    out
}

/// A straight line of tiles from `a` to `b` (the simplest possible "path").
pub fn straight(a: Tile, b: Tile) -> Vec<Tile> {
    let (mut x, mut y) = (a.0 as i32, a.1 as i32);
    let (dx, dy) = ((b.0 as i32 - x).abs(), -(b.1 as i32 - y).abs());
    let (sx, sy) = (
        if x < b.0 as i32 { 1 } else { -1 },
        if y < b.1 as i32 { 1 } else { -1 },
    );
    let mut err = dx + dy;
    let mut out = Vec::new();
    loop {
        out.push((x as usize, y as usize));
        if (x, y) == (b.0 as i32, b.1 as i32) {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_follow_cheap_tiles_and_skeletons_have_two_ends() {
        let (w, h) = (12, 8);
        // A cheap road along y = 6, expensive grass elsewhere, a wall at x = 6 except y = 6.
        let mut price: Vec<Option<f32>> = (0..w * h)
            .map(|a| Some(if a / w == 6 { 0.3 } else { 5.0 }))
            .collect();
        for y in 0..h {
            if y != 6 {
                price[y * w + 6] = None;
            }
        }
        let tiles = route(w, h, &price, &[(0, 1)], &BTreeSet::from([(11, 1)])).unwrap_or_default();
        assert!(tiles.contains(&(6, 6)), "goes through the gap");
        assert!(
            tiles.len() > 11,
            "and so is longer than straight: {}",
            tiles.len()
        );
        assert_eq!(
            straight((0, 0), (3, 3)),
            vec![(0, 0), (1, 1), (2, 2), (3, 3)]
        );
        let mut mask = vec![false; w * h];
        for x in 1..11 {
            for y in 2..5 {
                mask[y * w + x] = true;
            }
        }
        let piece = &pieces(&mask, w, h, 4)[0];
        let line = skeleton(piece, w, h);
        assert!(line.len() < piece.len());
        let e = ends(&line, 6);
        assert!(
            e.len() >= 2 && e.iter().any(|t| t.0 <= 3) && e.iter().any(|t| t.0 >= 8),
            "{e:?}"
        );
        let simple = waypoints(&[(0, 0), (1, 0), (2, 0), (3, 0)], 0.5);
        assert_eq!(simple.len(), 2);
        assert_eq!(
            band(&[vec![(2, 2)]], 1, w, h)
                .iter()
                .filter(|b| **b)
                .count(),
            9
        );
    }
}
