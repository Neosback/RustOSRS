//! Checking a drawing against the real paths, and the simple drawings to beat.

use super::grid::distance_to;
use super::model::Score;
use super::route::{self, Tile};
use super::sense::Scene;

/// A drawing: the routes, and the tiles a path of the model's width painted along them.
pub struct Draft {
    pub routes: Vec<Vec<Tile>>,
    pub painted: Vec<bool>,
}

impl Draft {
    pub fn new(scene: &Scene, routes: Vec<Vec<Tile>>, radius: usize) -> Self {
        let painted = route::band(&routes, radius, scene.width, scene.height);
        Self { routes, painted }
    }
}

/// How many tiles either side of the centre a path of `width` covers.
pub fn painted_radius(width: f32) -> usize {
    (((width - 1.0) / 2.0).round().max(0.0)) as usize
}

/// How close two drawings are, tolerating `tolerance` tiles of slack.
pub fn score(real: &[bool], made: &[bool], width: usize, height: usize, tolerance: u8) -> Score {
    let near_real = distance_to(real, width, height, 250);
    let near_made = distance_to(made, width, height, 250);
    let count = |marks: &[bool]| marks.iter().filter(|m| **m).count();
    let (real_n, made_n) = (count(real), count(made));
    let hit_made = (0..real.len())
        .filter(|a| made[*a] && near_real[*a] <= tolerance)
        .count();
    let hit_real = (0..real.len())
        .filter(|a| real[*a] && near_made[*a] <= tolerance)
        .count();
    let precision = if made_n == 0 {
        0.0
    } else {
        hit_made as f32 / made_n as f32
    };
    let recall = if real_n == 0 {
        0.0
    } else {
        hit_real as f32 / real_n as f32
    };
    let f1 = if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    };
    Score {
        precision,
        recall,
        f1,
    }
}

/// The ends of each piece of path in `mask` (at least `min` tiles): the "from here to there"
/// that a person would give. One group per piece.
pub fn endpoints_of(mask: &[bool], width: usize, height: usize, min: usize) -> Vec<Vec<Tile>> {
    route::pieces(mask, width, height, min)
        .iter()
        .map(|piece| route::ends(&route::skeleton(piece, width, height), 6))
        .filter(|ends| ends.len() >= 2)
        .collect()
}

/// Straight lines joining each group's points one after the other.
pub fn straight_lines(groups: &[Vec<Tile>]) -> Vec<Vec<Tile>> {
    groups
        .iter()
        .flat_map(|g| {
            g.windows(2)
                .map(|w| route::straight(w[0], w[1]))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The shortest way over open ground, ignoring everything the model knows.
pub fn shortest_walks(scene: &Scene, groups: &[Vec<Tile>]) -> Vec<Vec<Tile>> {
    let prices: Vec<Option<f32>> = (0..scene.tiles())
        .map(|a| {
            if scene.inside[a] {
                None
            } else {
                Some(if scene.walkable[a] { 1.0 } else { 40.0 })
            }
        })
        .collect();
    groups
        .iter()
        .flat_map(|g| route::network(scene.width, scene.height, &prices, g))
        .collect()
}

/// The centre lines of the real paths: each piece of at least `min` tiles thinned to one tile.
pub fn centre_lines(mask: &[bool], width: usize, height: usize, min: usize) -> Vec<bool> {
    let mut out = vec![false; width * height];
    for piece in route::pieces(mask, width, height, min) {
        for (x, y) in route::skeleton(&piece, width, height) {
            out[y * width + x] = true;
        }
    }
    out
}

/// How well drawn lines match the real paths: of the tiles drawn, the share on (or next to) a
/// real path; of the real centre lines, the share a drawn line comes within 2 tiles of.
pub fn line_score(
    real: &[bool],
    centres: &[bool],
    drawn: &[bool],
    width: usize,
    height: usize,
) -> Score {
    let near_real = distance_to(real, width, height, 250);
    let near_drawn = distance_to(drawn, width, height, 250);
    let drawn_n = drawn.iter().filter(|m| **m).count();
    let centre_n = centres.iter().filter(|m| **m).count();
    let on = (0..real.len())
        .filter(|a| drawn[*a] && near_real[*a] <= 1)
        .count();
    let covered = (0..real.len())
        .filter(|a| centres[*a] && near_drawn[*a] <= 2)
        .count();
    let precision = if drawn_n == 0 {
        0.0
    } else {
        on as f32 / drawn_n as f32
    };
    let recall = if centre_n == 0 {
        0.0
    } else {
        covered as f32 / centre_n as f32
    };
    let f1 = if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    };
    Score {
        precision,
        recall,
        f1,
    }
}

/// The tiles of a set of routes.
pub fn route_tiles(routes: &[Vec<Tile>], width: usize, height: usize) -> Vec<bool> {
    route::band(routes, 0, width, height)
}
