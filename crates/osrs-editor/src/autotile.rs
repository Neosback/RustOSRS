//! The auto-tiler: turn a smooth path into tiles that look hand-made.
//!
//! An OSRS tile holds one overlay in one of 12 shapes and 4 rotations (the real tables in
//! `osrs-scene::terrain`). Painting a path means choosing, for every tile the path touches, the
//! (shape, rotation) whose overlay best matches the part of the tile the path covers.
//!
//! Every tile is rasterised to an 8x8 sub-tile mask (64 bits, bit `y * 8 + x`, `y` up). All
//! candidate shapes are rasterised from the real face tables once; the path band is rasterised per
//! tile; the best candidate is the one with the smallest Hamming distance. A second pass compares each
//! tile's edge with its neighbours' ("portals") so pieces connect instead of leaving notches, which is
//! what makes the result read as intentional rather than as 64 independent guesses.

use osrs_core::coords::SceneTile;
use osrs_scene::terrain::{
    ShapedTerrainInput, ShapedTerrainSurface, TerrainColorSource, TerrainCorners,
};
use std::collections::HashMap;
use std::sync::OnceLock;

const GRID: usize = 8;
const ALL_BITS: u64 = u64::MAX;

/// What to do with one tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileChoice {
    /// Leave the tile alone.
    Keep,
    /// Whole tile is overlay (shape 0).
    Full,
    Shape {
        shape: u8,
        rotation: u8,
    },
}

impl TileChoice {
    /// `(shape, rotation)` of the overlay, `None` for `Keep`.
    pub fn shape_rotation(self) -> Option<(u8, u8)> {
        match self {
            Self::Keep => None,
            Self::Full => Some((0, 0)),
            Self::Shape { shape, rotation } => Some((shape, rotation)),
        }
    }
}

/// Triangles of one overlay shape on a unit square (`y` up) and whether each is overlay
/// (otherwise underlay). Built from the real terrain tables so thumbnails and the tiler cannot drift.
pub fn shape_triangles(shape: u8, rotation: u8) -> Vec<([[f32; 2]; 3], bool)> {
    let flat = TerrainCorners::new(0, 0, 0, 0);
    // The builder indexes its tables with the file shape plus one (rows 0 and 1 are "all underlay" and
    // "all overlay").
    let Ok(surface) = ShapedTerrainSurface::build(ShapedTerrainInput {
        shape: shape + 1,
        rotation: rotation & 3,
        texture_id: None,
        tile: SceneTile::new(0, 0),
        heights: flat,
        underlay_colors: flat,
        overlay_colors: flat,
        underlay_rgb: 0,
        overlay_rgb: 0,
    }) else {
        return Vec::new();
    };
    let point = |index: usize| {
        let position = surface.vertices[index].position;
        [
            position.x.units() as f32 / 128.0,
            position.z.units() as f32 / 128.0,
        ]
    };
    surface
        .faces
        .iter()
        .map(|face| {
            (
                [
                    point(face.indices[0]),
                    point(face.indices[1]),
                    point(face.indices[2]),
                ],
                face.color_source == TerrainColorSource::Overlay,
            )
        })
        .collect()
}

fn in_triangle(p: [f32; 2], t: &[[f32; 2]; 3]) -> bool {
    let sign = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
        (a[0] - c[0]) * (b[1] - c[1]) - (b[0] - c[0]) * (a[1] - c[1])
    };
    let d1 = sign(p, t[0], t[1]);
    let d2 = sign(p, t[1], t[2]);
    let d3 = sign(p, t[2], t[0]);
    let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(negative && positive)
}

/// The sub-cell centre in tile coordinates.
fn cell_centre(x: usize, y: usize) -> [f32; 2] {
    [
        (x as f32 + 0.5) / GRID as f32,
        (y as f32 + 0.5) / GRID as f32,
    ]
}

/// The 8x8 mask of the overlay part of a shape.
pub fn shape_mask(shape: u8, rotation: u8) -> u64 {
    if shape == 0 {
        return ALL_BITS;
    }
    let triangles: Vec<_> = shape_triangles(shape, rotation)
        .into_iter()
        .filter(|(_, overlay)| *overlay)
        .map(|(triangle, _)| triangle)
        .collect();
    let mut mask = 0_u64;
    for y in 0..GRID {
        for x in 0..GRID {
            let centre = cell_centre(x, y);
            if triangles
                .iter()
                .any(|triangle| in_triangle(centre, triangle))
            {
                mask |= 1 << (y * GRID + x);
            }
        }
    }
    mask
}

/// One thing a tile can become, with its mask.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    choice: TileChoice,
    mask: u64,
}

fn candidates() -> &'static [Candidate] {
    static CANDIDATES: OnceLock<Vec<Candidate>> = OnceLock::new();
    CANDIDATES.get_or_init(|| {
        let mut list = vec![
            Candidate {
                choice: TileChoice::Keep,
                mask: 0,
            },
            Candidate {
                choice: TileChoice::Full,
                mask: ALL_BITS,
            },
        ];
        for shape in 1..=11_u8 {
            for rotation in 0..4_u8 {
                let mask = shape_mask(shape, rotation);
                // Skip empty shapes and masks an earlier (simpler) shape already has.
                if mask == 0 || list.iter().any(|candidate| candidate.mask == mask) {
                    continue;
                }
                list.push(Candidate {
                    choice: TileChoice::Shape { shape, rotation },
                    mask,
                });
            }
        }
        list
    })
}

/// The column / row of a mask along one side: bit `i` is cell `i` along the edge.
fn edge(mask: u64, side: Side) -> u8 {
    let mut bits = 0_u8;
    for i in 0..GRID {
        let (x, y) = match side {
            Side::West => (0, i),
            Side::East => (GRID - 1, i),
            Side::South => (i, 0),
            Side::North => (i, GRID - 1),
        };
        if mask >> (y * GRID + x) & 1 == 1 {
            bits |= 1 << i;
        }
    }
    bits
}

#[derive(Clone, Copy)]
enum Side {
    West,
    East,
    South,
    North,
}

/// How a path is laid down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathParams {
    /// Width of the path in tiles.
    pub width: f32,
    /// Whole tiles only (no shaped edges).
    pub hard_edges: bool,
}

impl Default for PathParams {
    fn default() -> Self {
        Self {
            width: 3.0,
            hard_edges: false,
        }
    }
}

/// Points of a Catmull-Rom spline through `points` (world tile coordinates), about `step` apart.
pub fn spline_polyline(points: &[(f32, f32)], step: f32) -> Vec<(f32, f32)> {
    match points.len() {
        0 => return Vec::new(),
        1 => return vec![points[0]],
        _ => {}
    }
    let at = |i: isize| points[i.clamp(0, points.len() as isize - 1) as usize];
    let mut out = vec![points[0]];
    for segment in 0..points.len() as isize - 1 {
        let (p0, p1, p2, p3) = (
            at(segment - 1),
            at(segment),
            at(segment + 1),
            at(segment + 2),
        );
        let length = ((p2.0 - p1.0).powi(2) + (p2.1 - p1.1).powi(2)).sqrt();
        let samples = ((length / step).ceil() as usize).max(1);
        for k in 1..=samples {
            let t = k as f32 / samples as f32;
            let (t2, t3) = (t * t, t * t * t);
            let blend = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * ((2.0 * b)
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push((blend(p0.0, p1.0, p2.0, p3.0), blend(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    out
}

fn distance_to_polyline(point: (f32, f32), line: &[(f32, f32)]) -> f32 {
    if line.len() == 1 {
        return ((point.0 - line[0].0).powi(2) + (point.1 - line[0].1).powi(2)).sqrt();
    }
    let mut best = f32::MAX;
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length_squared = dx * dx + dy * dy;
        let t = if length_squared > 0.0 {
            (((point.0 - a.0) * dx + (point.1 - a.1) * dy) / length_squared).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (cx, cy) = (a.0 + dx * t, a.1 + dy * t);
        best = best.min((point.0 - cx).powi(2) + (point.1 - cy).powi(2));
    }
    best.sqrt()
}

/// The part of a tile the band covers, as a mask. `tile` is the tile's south-west corner in world
/// tile coordinates; a control point at tile `(x, y)` sits at `(x + 0.5, y + 0.5)`.
pub fn band_mask(line: &[(f32, f32)], half_width: f32, tile: (i32, i32)) -> u64 {
    let mut mask = 0_u64;
    for y in 0..GRID {
        for x in 0..GRID {
            let c = cell_centre(x, y);
            let point = (tile.0 as f32 + c[0], tile.1 as f32 + c[1]);
            if distance_to_polyline(point, line) <= half_width {
                mask |= 1 << (y * GRID + x);
            }
        }
    }
    mask
}

fn cost(target: u64, candidate: u64) -> u32 {
    (target ^ candidate).count_ones()
}

/// The tiles a path of `params` laid along `points` (world tile coordinates of the control points)
/// turns into. Tiles that stay as they are are not listed.
pub fn generate(points: &[(f32, f32)], params: PathParams) -> Vec<((i32, i32), TileChoice)> {
    if points.is_empty() {
        return Vec::new();
    }
    let half = (params.width / 2.0).max(0.5);
    let line = spline_polyline(points, 0.5);
    let (mut min, mut max) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for p in &line {
        min = (min.0.min(p.0), min.1.min(p.1));
        max = (max.0.max(p.0), max.1.max(p.1));
    }
    let (x0, y0) = (
        (min.0 - half - 1.0).floor() as i32,
        (min.1 - half - 1.0).floor() as i32,
    );
    let (x1, y1) = (
        (max.0 + half + 1.0).ceil() as i32,
        (max.1 + half + 1.0).ceil() as i32,
    );

    // The band mask of every tile it touches.
    let mut targets: HashMap<(i32, i32), u64> = HashMap::new();
    for x in x0..=x1 {
        for y in y0..=y1 {
            let centre = (x as f32 + 0.5, y as f32 + 0.5);
            // Cheap rejection: farther than half a tile diagonal beyond the band.
            if distance_to_polyline(centre, &line) > half + 0.75 {
                continue;
            }
            let mask = band_mask(&line, half, (x, y));
            if mask != 0 {
                targets.insert((x, y), mask);
            }
        }
    }

    if params.hard_edges {
        let mut out: Vec<_> = targets
            .iter()
            .filter(|(_, mask)| mask.count_ones() >= 32)
            .map(|(tile, _)| (*tile, TileChoice::Full))
            .collect();
        out.sort_by_key(|(tile, _)| *tile);
        return out;
    }

    let list = candidates();
    // First choice: the smallest mismatch (the candidate list puts Keep and Full first, so ties go to
    // the simple answer).
    let mut chosen: HashMap<(i32, i32), usize> = HashMap::new();
    for (tile, target) in &targets {
        let best = (0..list.len())
            .min_by_key(|index| cost(*target, list[*index].mask))
            .unwrap_or(0);
        chosen.insert(*tile, best);
    }

    // Portal passes: among near-best candidates, prefer the one whose edges agree with the
    // neighbours' current choice.
    let neighbour_mask = |chosen: &HashMap<(i32, i32), usize>, tile: (i32, i32)| -> u64 {
        chosen.get(&tile).map_or(0, |index| list[*index].mask)
    };
    let mut order: Vec<(i32, i32)> = targets.keys().copied().collect();
    order.sort();
    for pass in 0..3 {
        if pass % 2 == 1 {
            order.reverse();
        }
        for tile in &order {
            let target = targets[tile];
            let west = edge(neighbour_mask(&chosen, (tile.0 - 1, tile.1)), Side::East);
            let east = edge(neighbour_mask(&chosen, (tile.0 + 1, tile.1)), Side::West);
            let south = edge(neighbour_mask(&chosen, (tile.0, tile.1 - 1)), Side::North);
            let north = edge(neighbour_mask(&chosen, (tile.0, tile.1 + 1)), Side::South);
            let best_cost = (0..list.len())
                .map(|index| cost(target, list[index].mask))
                .min()
                .unwrap_or(0);
            let score = |index: usize| {
                let mask = list[index].mask;
                let portals = (edge(mask, Side::West) ^ west).count_ones()
                    + (edge(mask, Side::East) ^ east).count_ones()
                    + (edge(mask, Side::South) ^ south).count_ones()
                    + (edge(mask, Side::North) ^ north).count_ones();
                // The shape error counts once, a mismatched edge cell twice: a slightly worse shape
                // that joins its neighbours beats a better one that leaves a notch.
                cost(target, mask) + 2 * portals
            };
            let pick = (0..list.len())
                .filter(|index| cost(target, list[*index].mask) <= best_cost + 6)
                .min_by_key(|index| score(*index));
            if let Some(pick) = pick {
                chosen.insert(*tile, pick);
            }
        }
    }

    let mut out: Vec<_> = chosen
        .into_iter()
        .map(|(tile, index)| (tile, list[index].choice))
        .filter(|(_, choice)| *choice != TileChoice::Keep)
        .collect();
    out.sort_by_key(|(tile, _)| *tile);
    out
}

/// Total mismatch of a result against the ideal band: every tile the band touches counts, whether or
/// not the result changed it (for tests and diagnostics).
#[cfg(test)]
fn total_error(
    points: &[(f32, f32)],
    params: PathParams,
    result: &[((i32, i32), TileChoice)],
) -> u32 {
    let half = (params.width / 2.0).max(0.5);
    let line = spline_polyline(points, 0.5);
    let chosen: HashMap<(i32, i32), TileChoice> = result.iter().copied().collect();
    let mut tiles: std::collections::HashSet<(i32, i32)> = chosen.keys().copied().collect();
    let (mut min, mut max) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for p in &line {
        min = (min.0.min(p.0), min.1.min(p.1));
        max = (max.0.max(p.0), max.1.max(p.1));
    }
    for x in (min.0 - half - 1.0).floor() as i32..=(max.0 + half + 1.0).ceil() as i32 {
        for y in (min.1 - half - 1.0).floor() as i32..=(max.1 + half + 1.0).ceil() as i32 {
            tiles.insert((x, y));
        }
    }
    tiles
        .into_iter()
        .map(|tile| {
            let mask = match chosen.get(&tile) {
                None | Some(TileChoice::Keep) => 0,
                Some(TileChoice::Full) => ALL_BITS,
                Some(TileChoice::Shape { shape, rotation }) => shape_mask(*shape, *rotation),
            };
            cost(band_mask(&line, half, tile), mask)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fraction(mask: u64) -> f32 {
        mask.count_ones() as f32 / 64.0
    }

    #[test]
    fn shape_masks_cover_the_share_the_tables_describe() {
        // From `docs/plans/AUTO-TILER.md`: overlay share per file shape.
        let expected = [
            (1, 0.5),
            (2, 0.25),
            (3, 0.25),
            (4, 0.75),
            (5, 0.75),
            (6, 0.5),
            (7, 0.125),
            (8, 0.875),
        ];
        for (shape, share) in expected {
            let got = fraction(shape_mask(shape, 0));
            assert!(
                (got - share).abs() < 0.1,
                "shape {shape}: expected about {share}, rasterised {got}"
            );
        }
        assert_eq!(shape_mask(0, 0), ALL_BITS);
    }

    #[test]
    fn rotations_of_a_corner_piece_cover_different_corners() {
        let masks: Vec<u64> = (0..4).map(|r| shape_mask(2, r)).collect();
        for i in 0..4 {
            for j in i + 1..4 {
                assert_ne!(
                    masks[i], masks[j],
                    "rotations {i} and {j} of shape 2 are equal"
                );
            }
        }
        // Each rotation covers the same area.
        assert!(
            masks
                .iter()
                .all(|m| m.count_ones() == masks[0].count_ones())
        );
    }

    #[test]
    fn candidates_are_distinct_and_start_with_keep_and_full() {
        let list = candidates();
        assert_eq!(list[0].choice, TileChoice::Keep);
        assert_eq!(list[1].choice, TileChoice::Full);
        for i in 0..list.len() {
            for j in i + 1..list.len() {
                assert_ne!(list[i].mask, list[j].mask);
            }
        }
        assert!(list.len() > 20, "{} candidates", list.len());
    }

    #[test]
    fn a_tile_aligned_straight_band_is_all_full_tiles() {
        // Centre line y = 10.5 (the middle of tile row 10), width 3: rows 9, 10 and 11 exactly.
        let points = [(0.5, 10.5), (20.5, 10.5)];
        let result = generate(
            &points,
            PathParams {
                width: 3.0,
                hard_edges: false,
            },
        );
        let full: Vec<_> = result
            .iter()
            .filter(|(_, c)| *c == TileChoice::Full)
            .collect();
        assert!(!full.is_empty());
        for ((x, y), choice) in &result {
            // Away from the rounded ends.
            if (9..=11).contains(y) && (3..=17).contains(x) {
                assert_eq!(*choice, TileChoice::Full, "tile {x}, {y}");
            }
        }
        // No tile outside the band is touched except the rounded ends.
        assert!(
            result
                .iter()
                .filter(|((x, _), _)| (2..=18).contains(x))
                .all(|((_, y), _)| (9..=11).contains(y))
        );
    }

    #[test]
    fn a_diagonal_band_uses_shaped_edge_tiles() {
        let points = [(2.5, 2.5), (22.5, 22.5)];
        let result = generate(
            &points,
            PathParams {
                width: 3.0,
                hard_edges: false,
            },
        );
        let shaped = result
            .iter()
            .filter(|(_, c)| matches!(c, TileChoice::Shape { .. }))
            .count();
        let full = result
            .iter()
            .filter(|(_, c)| *c == TileChoice::Full)
            .count();
        assert!(shaped > 10 && full > 5, "shaped {shaped}, full {full}");
    }

    #[test]
    fn shaped_edges_follow_the_ideal_band_better_than_whole_tiles() {
        let points = [(2.5, 2.5), (18.5, 11.5), (30.5, 14.5)];
        let smooth = PathParams {
            width: 4.0,
            hard_edges: false,
        };
        let hard = PathParams {
            width: 4.0,
            hard_edges: true,
        };
        let smooth_error = total_error(&points, smooth, &generate(&points, smooth));
        // Whole tiles error against the same ideal band.
        let hard_error = total_error(&points, smooth, &generate(&points, hard));
        assert!(
            smooth_error * 2 < hard_error,
            "smooth {smooth_error} vs hard {hard_error}"
        );
    }

    #[test]
    fn hard_edges_never_use_shapes() {
        let points = [(2.5, 2.5), (18.5, 11.5)];
        let result = generate(
            &points,
            PathParams {
                width: 4.0,
                hard_edges: true,
            },
        );
        assert!(result.iter().all(|(_, c)| *c == TileChoice::Full));
    }

    #[test]
    fn the_result_is_deterministic() {
        let points = [(2.5, 2.5), (18.5, 11.5), (24.5, 3.5)];
        let params = PathParams::default();
        assert_eq!(generate(&points, params), generate(&points, params));
    }

    #[test]
    fn a_single_point_makes_a_small_blob_and_no_points_make_nothing() {
        assert!(generate(&[], PathParams::default()).is_empty());
        let blob = generate(
            &[(10.5, 10.5)],
            PathParams {
                width: 3.0,
                hard_edges: false,
            },
        );
        assert!(!blob.is_empty());
    }

    #[test]
    fn spline_passes_through_its_control_points() {
        let points = [(0.0, 0.0), (5.0, 3.0), (9.0, 0.0)];
        let line = spline_polyline(&points, 0.25);
        for p in points {
            assert!(
                line.iter()
                    .any(|q| (q.0 - p.0).abs() < 1e-3 && (q.1 - p.1).abs() < 1e-3),
                "{p:?} not on the spline"
            );
        }
    }
}
