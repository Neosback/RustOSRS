//! The auto-tiler: turn a smooth path into tiles that look hand-made.
//!
//! An OSRS tile holds one overlay in one of 12 shapes and 4 rotations (the real tables in
//! `osrs-scene::terrain`). A census of the real map (`osrs-world`'s `shape_census` example, stored in
//! `data/shape-census.txt`) shows how people use them: 99.9% of tiles with eight same-overlay
//! neighbours are whole, a straight edge is a whole tile 87% of the time, and shapes appear mainly at
//! corners. Hand-made paths are staircases of whole tiles with the odd cut corner, not a thin
//! diagonal sliver on every edge tile.
//!
//! So the tiler works in two steps:
//!
//! 1. **Occupancy**: a tile belongs to the path when the band covers at least half of it (tiles that
//!    already carry the path's overlay count too, so a new path flows into an old one).
//! 2. **Shapes**: every tile the band touches picks whole, nothing, or one of the shapes, scored by how
//!    far its sub-tile mask (16 x 16) is from the band's (dominant), a penalty for partly covered edges
//!    that do not meet the neighbours' ("portals"), and a small census term (rotation-symmetric,
//!    smoothed) that breaks ties towards what people place. Six relaxation passes let neighbours settle.
//!
//! The path itself is a centripetal Catmull-Rom spline (no loops or overshoot when points are unevenly
//! spaced), or straight lines, optionally closed.

use osrs_core::coords::SceneTile;
use osrs_scene::terrain::{
    ShapedTerrainInput, ShapedTerrainSurface, TerrainColorSource, TerrainCorners,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const GRID: usize = 16;
const WORDS: usize = GRID * GRID / 64;
/// Annealing: sweeps over the tiles, and the temperature range (in cost units).
const ANNEAL_SWEEPS: usize = 300;
const ANNEAL_START: f32 = 3.0;
const ANNEAL_END: f32 = 0.05;
/// A tile the band covers at least this many cells of may carry a small piece of the path.
const TOUCHED: u32 = 10;
/// What leaving a tile alone costs, against the census cost of a piece.
const KEEP_COST: f32 = 0.8;
/// A tile joins the path when the band covers at least this many of its 256 sub-cells.
const OCCUPIED: u32 = 128;
/// The census: `pattern shape rotation count` per line.
const CENSUS: &str = include_str!("../data/shape-census.txt");

/// A tile's 16 x 16 mask.
type Mask = [u64; WORDS];
const EMPTY: Mask = [0; WORDS];
const FULL: Mask = [u64::MAX; WORDS];

fn set(mask: &mut Mask, x: usize, y: usize) {
    let bit = y * GRID + x;
    mask[bit / 64] |= 1 << (bit % 64);
}

fn get(mask: &Mask, x: usize, y: usize) -> bool {
    let bit = y * GRID + x;
    mask[bit / 64] >> (bit % 64) & 1 == 1
}

fn popcount(mask: &Mask) -> u32 {
    mask.iter().map(|word| word.count_ones()).sum()
}

fn cost(target: &Mask, candidate: &Mask) -> u32 {
    target
        .iter()
        .zip(candidate)
        .map(|(a, b)| (a ^ b).count_ones())
        .sum()
}

/// What to do with one tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileChoice {
    /// Leave the tile alone.
    #[allow(
        dead_code,
        reason = "part of the choice vocabulary; the tiler lists only changed tiles"
    )]
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

/// How many of the 256 sub-cells the overlay part of a shape covers.
#[cfg(test)]
fn shape_cells(shape: u8, rotation: u8) -> u32 {
    popcount(&shape_mask(shape, rotation))
}

fn shape_mask(shape: u8, rotation: u8) -> Mask {
    if shape == 0 {
        return FULL;
    }
    let triangles: Vec<_> = shape_triangles(shape, rotation)
        .into_iter()
        .filter(|(_, overlay)| *overlay)
        .map(|(triangle, _)| triangle)
        .collect();
    let mut mask = EMPTY;
    for y in 0..GRID {
        for x in 0..GRID {
            let centre = cell_centre(x, y);
            if triangles
                .iter()
                .any(|triangle| in_triangle(centre, triangle))
            {
                set(&mut mask, x, y);
            }
        }
    }
    mask
}

/// How the path's edge tiles are cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EdgeStyle {
    /// Every shape, including the rounded pieces: organic paths and rivers.
    #[default]
    Smooth,
    /// Straight and diagonal cuts only (no rounded pieces): roads, walls, courtyards.
    Chiselled,
    /// Whole tiles only.
    Whole,
    /// Castle walls along the path: every band tile is laid whole and wall objects are spawned
    /// (the old WALL brush style).
    Walls,
    /// The outline is fitted first with the fewest straight runs the shapes can draw: no zigzag,
    /// at the price of straying a little further (up to about 0.45 tiles) from the drawn curve.
    Runs,
}

impl EdgeStyle {
    pub const ALL: [Self; 5] = [
        Self::Smooth,
        Self::Runs,
        Self::Chiselled,
        Self::Whole,
        Self::Walls,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Smooth => "Smooth",
            Self::Chiselled => "Chiselled",
            Self::Whole => "Whole tiles",
            Self::Walls => "Walls",
            Self::Runs => "Straight runs",
        }
    }

    pub const fn hint(self) -> &'static str {
        match self {
            Self::Smooth => "Rounded edge pieces: organic paths and rivers",
            Self::Chiselled => "Straight and diagonal cuts only: roads and courtyards",
            Self::Whole => "Full tiles, no shaped edges",
            Self::Walls => "Castle wall along the path: whole tiles plus spawned wall objects",
            Self::Runs => "Long straight edges with few turns: no zigzag, strays a little more",
        }
    }

    /// Whether a shape (file value `1..=11`) may be used.
    fn allows(self, shape: u8) -> bool {
        match self {
            Self::Smooth | Self::Runs => true,
            Self::Chiselled => !matches!(shape, 9..=11),
            // Walls carry the visuals: every tile is laid whole, like `Whole`.
            Self::Whole | Self::Walls => false,
        }
    }
}

/// Whether the path bends smoothly through its points or runs straight between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CurveKind {
    #[default]
    Curve,
    Lines,
}

/// One thing a tile can become, with its mask.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    choice: TileChoice,
    mask: Mask,
    /// `shape * 4 + rotation` in the census (0 for a whole tile).
    slot: usize,
}

/// Every candidate: the whole tile, then each shape (file value `1..=11`) in each rotation. Shapes
/// with the same mask stay separate: the census says which spelling people use.
fn all_candidates() -> &'static [Candidate] {
    static CANDIDATES: OnceLock<Vec<Candidate>> = OnceLock::new();
    CANDIDATES.get_or_init(|| {
        let mut list = vec![
            Candidate {
                choice: TileChoice::Keep,
                mask: EMPTY,
                slot: KEEP_SLOT,
            },
            Candidate {
                choice: TileChoice::Full,
                mask: FULL,
                slot: 0,
            },
        ];
        for shape in 1..=11_u8 {
            for rotation in 0..4_u8 {
                let mask = shape_mask(shape, rotation);
                if popcount(&mask) == 0 {
                    continue;
                }
                list.push(Candidate {
                    choice: TileChoice::Shape { shape, rotation },
                    mask,
                    slot: usize::from(shape) * 4 + usize::from(rotation),
                });
            }
        }
        list
    })
}

/// The candidates a style may use.
fn candidates_for(style: EdgeStyle) -> Vec<&'static Candidate> {
    all_candidates()
        .iter()
        .filter(|candidate| match candidate.choice {
            TileChoice::Keep | TileChoice::Full => true,
            TileChoice::Shape { shape, .. } => style.allows(shape),
        })
        .collect()
}

const SLOTS: usize = 12 * 4;
/// The census has nothing to say about leaving a tile alone.
const KEEP_SLOT: usize = usize::MAX;

/// How likely each (shape, rotation) is for each pattern of same-overlay neighbours (bit 0 = north,
/// then clockwise: NE, E, SE, S, SW, W, NW), learned from the real map.
struct Prior {
    /// `-ln p` per pattern and slot.
    cost: Vec<[f32; SLOTS]>,
}

fn prior() -> &'static Prior {
    static PRIOR: OnceLock<Prior> = OnceLock::new();
    PRIOR.get_or_init(|| {
        let mut counts = vec![[0.0_f32; SLOTS]; 256];
        for line in CENSUS.lines() {
            let mut fields = line
                .split_whitespace()
                .map(|f| f.parse::<u32>().unwrap_or(0));
            let (Some(pattern), Some(shape), Some(rotation), Some(count)) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            if pattern > 255 || shape > 11 {
                continue;
            }
            // Turning the neighbourhood a quarter turn clockwise (bits move two places) turns the
            // tile's rotation by one, so every observation counts in all four orientations.
            for turn in 0..4_u32 {
                let rotated = ((pattern << (2 * turn)) | (pattern >> (8 - 2 * turn))) & 255;
                let slot = if shape == 0 {
                    0
                } else {
                    (shape * 4 + (rotation + turn) % 4) as usize
                };
                counts[rotated as usize][slot] += count as f32;
            }
        }
        // Background: how shapes are spread over every pattern with a boundary in it.
        let mut background = [0.0_f32; SLOTS];
        for row in &counts[..255] {
            for (slot, count) in row.iter().enumerate() {
                background[slot] += count;
            }
        }
        let background_total: f32 = background.iter().sum::<f32>().max(1.0);
        const WEIGHT: f32 = 12.0;
        let cost = counts
            .iter()
            .map(|row| {
                let total: f32 = row.iter().sum();
                let mut out = [0.0_f32; SLOTS];
                for slot in 0..SLOTS {
                    let p = (row[slot] + WEIGHT * background[slot] / background_total)
                        / (total + WEIGHT);
                    out[slot] = -p.max(1e-4).ln();
                }
                out
            })
            .collect();
        Prior { cost }
    })
}

#[derive(Clone, Copy)]
enum Side {
    West,
    East,
    South,
    North,
}

/// The cells along one side of a mask: bit `i` is cell `i` along the edge.
fn edge(mask: &Mask, side: Side) -> u16 {
    let mut bits = 0_u16;
    for i in 0..GRID {
        let (x, y) = match side {
            Side::West => (0, i),
            Side::East => (GRID - 1, i),
            Side::South => (i, 0),
            Side::North => (i, GRID - 1),
        };
        if get(mask, x, y) {
            bits |= 1 << i;
        }
    }
    bits
}

/// How a path is laid down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathParams {
    /// Width of the path in tiles.
    pub width: f32,
    pub style: EdgeStyle,
    pub curve: CurveKind,
    /// Join the last point back to the first.
    pub closed: bool,
    /// How far (tiles) a control point may be nudged so the runs between points take one of the
    /// slopes the shapes draw exactly straight (`0`: leave the points where they are).
    pub snap: f32,
    /// Straight runs style: how far (tiles) the outline may stray from the drawn curve to keep a
    /// run straight.
    pub straighten: f32,
}

impl Default for PathParams {
    fn default() -> Self {
        Self {
            width: 3.0,
            style: EdgeStyle::Smooth,
            curve: CurveKind::Curve,
            closed: false,
            snap: 0.0,
            straighten: 0.45,
        }
    }
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Barry-Goldman evaluation of a centripetal Catmull-Rom segment `p1 -> p2` at `t` in `0..=1`.
fn centripetal(
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    p3: (f32, f32),
    t: f32,
) -> (f32, f32) {
    let knot = |a: (f32, f32), b: (f32, f32)| dist(a, b).max(1e-3).sqrt();
    let t0 = 0.0;
    let t1 = t0 + knot(p0, p1);
    let t2 = t1 + knot(p1, p2);
    let t3 = t2 + knot(p2, p3);
    let t = t1 + (t2 - t1) * t;
    let lerp = |a: (f32, f32), b: (f32, f32), ta: f32, tb: f32| {
        let w = if tb - ta > 1e-6 {
            (t - ta) / (tb - ta)
        } else {
            0.0
        };
        (a.0 + (b.0 - a.0) * w, a.1 + (b.1 - a.1) * w)
    };
    let a1 = lerp(p0, p1, t0, t1);
    let a2 = lerp(p1, p2, t1, t2);
    let a3 = lerp(p2, p3, t2, t3);
    let b1 = lerp(a1, a2, t0, t2);
    let b2 = lerp(a2, a3, t1, t3);
    lerp(b1, b2, t1, t2)
}

/// Directions (radians) along which the shapes draw an edge exactly straight: the axes, the
/// diagonals and the 2:1 slopes (corner to the middle of the far side), in every quadrant.
fn clean_angles() -> &'static [f32] {
    static ANGLES: OnceLock<Vec<f32>> = OnceLock::new();
    ANGLES.get_or_init(|| {
        let mut list = Vec::new();
        for quarter in 0..8 {
            let base = quarter as f32 * std::f32::consts::FRAC_PI_2;
            for a in [
                0.0,
                0.5_f32.atan(),
                std::f32::consts::FRAC_PI_4,
                2.0_f32.atan(),
            ] {
                list.push(base + a);
                list.push(base - a);
            }
        }
        list
    })
}

/// The nearest clean direction (radians) to a run heading `angle`.
fn nearest_clean(angle: f32) -> f32 {
    clean_angles()
        .iter()
        .copied()
        .min_by(|a, b| {
            let diff = |x: f32| (x - angle).sin().abs();
            diff(*a).total_cmp(&diff(*b))
        })
        .unwrap_or(angle)
}

/// The control points moved as little as possible (least squares) so that every run between
/// them, two tiles or longer, takes a clean direction exactly. If that would move a point further
/// than `reach` tiles, the run that asks for the most is left as it was and the rest is solved
/// again.
#[allow(
    clippy::needless_range_loop,
    reason = "small dense elimination reads clearer indexed"
)]
pub fn snap_points(points: &[(f32, f32)], reach: f32, curve: CurveKind) -> Vec<(f32, f32)> {
    let count = points.len();
    if count < 2 || reach <= 0.0 {
        return points.to_vec();
    }
    // The wanted direction of each run (a unit normal), `None` for short runs.
    // Along a curve a run is only straightened where the curve is nearly straight there (a turn of
    // more than 25 degrees at either end of the run is a bend, and moving its points reshapes the
    // bend, which is worse than the stair it would save); polylines are straight everywhere.
    let heading = |i: usize| (points[i + 1].1 - points[i].1).atan2(points[i + 1].0 - points[i].0);
    let turn = |i: usize| {
        let d = heading(i) - heading(i - 1);
        d.sin().atan2(d.cos()).abs()
    };
    let gentle = |i: usize| {
        curve == CurveKind::Lines
            || ((i == 0 || turn(i) <= 0.44) && (i + 2 >= count || turn(i + 1) <= 0.44))
    };
    let mut normals: Vec<Option<(f32, f32)>> = (0..count - 1)
        .map(|i| {
            let (dx, dy) = (points[i + 1].0 - points[i].0, points[i + 1].1 - points[i].1);
            if (dx * dx + dy * dy).sqrt() < 2.0 || !gentle(i) {
                return None;
            }
            let angle = nearest_clean(dy.atan2(dx));
            Some((-angle.sin(), angle.cos()))
        })
        .collect();
    loop {
        let rows: Vec<usize> = (0..count - 1).filter(|i| normals[*i].is_some()).collect();
        if rows.is_empty() {
            return points.to_vec();
        }
        let m = rows.len();
        // A A^T (m x m, dense: runs are few) and the residuals r = A P.
        let mut matrix = vec![vec![0.0_f32; m + 1]; m];
        for (a, &i) in rows.iter().enumerate() {
            let ni = normals[i].unwrap_or((0.0, 0.0));
            matrix[a][m] =
                (points[i + 1].0 - points[i].0) * ni.0 + (points[i + 1].1 - points[i].1) * ni.1;
            for (b, &j) in rows.iter().enumerate() {
                let nj = normals[j].unwrap_or((0.0, 0.0));
                let dot = ni.0 * nj.0 + ni.1 * nj.1;
                // Rows touch the same point: i + 1 == j gives -dot, j + 1 == i gives -dot.
                matrix[a][b] = if i == j {
                    2.0
                } else if i + 1 == j || j + 1 == i {
                    -dot
                } else {
                    0.0
                };
            }
        }
        // Gaussian elimination with partial pivoting.
        let mut solved = true;
        for col in 0..m {
            let pivot = (col..m)
                .max_by(|a, b| matrix[*a][col].abs().total_cmp(&matrix[*b][col].abs()))
                .unwrap_or(col);
            if matrix[pivot][col].abs() < 1e-6 {
                solved = false;
                break;
            }
            matrix.swap(col, pivot);
            for row in col + 1..m {
                let factor = matrix[row][col] / matrix[col][col];
                for k in col..=m {
                    matrix[row][k] -= factor * matrix[col][k];
                }
            }
        }
        if !solved {
            // Parallel runs make the system singular: drop the shortest and try again.
            if let Some(&i) = rows.first() {
                normals[i] = None;
                continue;
            }
            return points.to_vec();
        }
        let mut lambda = vec![0.0_f32; m];
        for row in (0..m).rev() {
            let mut value = matrix[row][m];
            for k in row + 1..m {
                value -= matrix[row][k] * lambda[k];
            }
            lambda[row] = value / matrix[row][row];
        }
        // Q = P - A^T lambda.
        let mut moved = points.to_vec();
        for (a, &i) in rows.iter().enumerate() {
            let ni = normals[i].unwrap_or((0.0, 0.0));
            moved[i + 1].0 -= lambda[a] * ni.0;
            moved[i + 1].1 -= lambda[a] * ni.1;
            moved[i].0 += lambda[a] * ni.0;
            moved[i].1 += lambda[a] * ni.1;
        }
        // Too far? Release the run whose points moved most.
        let worst = rows
            .iter()
            .map(|&i| {
                (
                    i,
                    dist(moved[i], points[i]).max(dist(moved[i + 1], points[i + 1])),
                )
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        match worst {
            Some((i, far)) if far > reach => normals[i] = None,
            _ => return moved,
        }
    }
}

/// The path as a dense polyline (points about `step` tiles apart) in world tile coordinates.
pub fn path_polyline(points: &[(f32, f32)], params: PathParams, step: f32) -> Vec<(f32, f32)> {
    let snapped;
    let points = if params.snap > 0.0 && !params.closed && points.len() >= 2 {
        snapped = snap_points(points, params.snap, params.curve);
        &snapped[..]
    } else {
        points
    };
    match points.len() {
        0 => return Vec::new(),
        1 => return vec![points[0]],
        _ => {}
    }
    let closed = params.closed && points.len() >= 3;
    let count = points.len();
    let at = |i: isize| -> (f32, f32) {
        if closed {
            points[i.rem_euclid(count as isize) as usize]
        } else if i < 0 {
            // Beyond the ends, mirror the neighbouring point so the curve leaves straight.
            let (p, q) = (points[0], points[1]);
            (2.0 * p.0 - q.0, 2.0 * p.1 - q.1)
        } else if i >= count as isize {
            let (p, q) = (points[count - 1], points[count - 2]);
            (2.0 * p.0 - q.0, 2.0 * p.1 - q.1)
        } else {
            points[i as usize]
        }
    };
    let segments = if closed { count } else { count - 1 };
    let mut out = vec![points[0]];
    for segment in 0..segments as isize {
        let (p0, p1, p2, p3) = (
            at(segment - 1),
            at(segment),
            at(segment + 1),
            at(segment + 2),
        );
        let length = dist(p1, p2);
        let samples = ((length / step).ceil() as usize).max(1);
        for k in 1..=samples {
            let t = k as f32 / samples as f32;
            out.push(match params.curve {
                CurveKind::Curve => centripetal(p0, p1, p2, p3, t),
                CurveKind::Lines => (p1.0 + (p2.0 - p1.0) * t, p1.1 + (p2.1 - p1.1) * t),
            });
        }
    }
    out
}

/// Where along the path (`0..=1` by length) the point nearest to `point` lies.
pub fn ramp_parameter(line: &[(f32, f32)], point: (f32, f32)) -> f32 {
    if line.len() < 2 {
        return 0.0;
    }
    let mut total = 0.0;
    let mut best = (f32::MAX, 0.0);
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length = (dx * dx + dy * dy).sqrt();
        let t = if length > 0.0 {
            (((point.0 - a.0) * dx + (point.1 - a.1) * dy) / (length * length)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let closest = (a.0 + dx * t, a.1 + dy * t);
        let d = dist(point, closest);
        if d < best.0 {
            best = (d, total + length * t);
        }
        total += length;
    }
    if total > 0.0 { best.1 / total } else { 0.0 }
}

/// Polyline spacing for the wall band rasterizer (matches the preview line's density).
const BAND_STEP: f32 = 0.25;

/// A rasterized castle-wall band: the tiles plus the averaged unit tangent per tile (wall
/// direction info for diagonal detection). Mirrors the old `rasterizeBandWithTangents` band walk.
pub struct BandTiles {
    pub tiles: HashSet<(i32, i32)>,
    pub tangents: HashMap<(i32, i32), (f32, f32)>,
}

/// Stamp the tiles a dense polyline covers `thickness` tiles wide, walking each sample's
/// perpendicular, and average the unit tangent per tile. Tiles a straight run crosses are stamped
/// symmetrically; even thicknesses shift the far edge in by one so the count stays exact.
pub fn rasterize_band(line: &[(f32, f32)], thickness: f32) -> BandTiles {
    let mut tiles = HashSet::new();
    let mut tangents: HashMap<(i32, i32), (f32, f32)> = HashMap::new();
    if line.len() < 2 {
        return BandTiles { tiles, tangents };
    }
    let count = thickness.max(1.0).round() as i32;
    let half = count / 2;
    let even = count % 2 == 0;
    for (index, &(x, y)) in line.iter().enumerate() {
        // Central difference along the dense polyline; forward/backward at the ends.
        let (tx, ty) = if index == 0 {
            (line[1].0 - x, line[1].1 - y)
        } else if index + 1 == line.len() {
            (x - line[index - 1].0, y - line[index - 1].1)
        } else {
            (
                line[index + 1].0 - line[index - 1].0,
                line[index + 1].1 - line[index - 1].1,
            )
        };
        let (mut dx, mut dy) = (tx, ty);
        let mut length = (tx * tx + ty * ty).sqrt();
        if length < 1e-4 {
            dx = 1.0;
            dy = 0.0;
            length = 1.0;
        }
        let (nx, ny) = (-dy / length, dx / length);
        let (ux, uy) = (dx / length, dy / length);
        // Tiles use floor coordinates throughout the editor (a point at 0.5 sits in tile 0).
        for k in -half..=(half - if even { 1 } else { 0 }) {
            let tile = (
                (x + nx * k as f32).floor() as i32,
                (y + ny * k as f32).floor() as i32,
            );
            tiles.insert(tile);
            tangents
                .entry(tile)
                .and_modify(|tangent| {
                    tangent.0 = (tangent.0 + ux) * 0.5;
                    tangent.1 = (tangent.1 + uy) * 0.5;
                })
                .or_insert((ux, uy));
        }
    }
    BandTiles { tiles, tangents }
}

/// A WALL commit: flat overlay tiles (band plus control-point tower squares) with per-tile
/// tangents for wall emission. No shape search: walls carry the visuals, as in the old WALL mode.
pub struct WallPlan {
    pub tiles: Vec<((i32, i32), TileChoice)>,
    pub tangents: HashMap<(i32, i32), (f32, f32)>,
}

/// Lay a castle-wall band `thickness` tiles wide with `tower_size` squares around each control
/// point (size 1 disables towers, as in the old tool). Every covered tile is laid whole.
pub fn generate_walls(
    points: &[(f32, f32)],
    params: PathParams,
    thickness: f32,
    tower_size: f32,
) -> WallPlan {
    let mut plan = WallPlan {
        tiles: Vec::new(),
        tangents: HashMap::new(),
    };
    if points.len() < 2 {
        return plan;
    }
    let line = path_polyline(points, params, BAND_STEP);
    if line.len() < 2 {
        return plan;
    }
    let band = rasterize_band(&line, thickness);
    let mut tiles = band.tiles;
    let size = tower_size.max(1.0).round() as i32;
    if size > 1 {
        let half = size / 2;
        let far = size - 1 - half;
        for &(x, y) in points {
            let (cx, cy) = (x.floor() as i32, y.floor() as i32);
            for dx in -half..=far {
                for dy in -half..=far {
                    tiles.insert((cx + dx, cy + dy));
                }
            }
        }
    }
    let mut tiles: Vec<_> = tiles
        .into_iter()
        .map(|tile| (tile, TileChoice::Full))
        .collect();
    // Deterministic order (the old tool iterated a hash set).
    tiles.sort_by_key(|(tile, _)| *tile);
    plan.tiles = tiles;
    plan.tangents = band.tangents;
    plan
}

/// The polyline's segments bucketed by the tiles they can influence, so a tile only measures the few
/// segments near it.
struct SegmentIndex {
    segments: Vec<((f32, f32), (f32, f32))>,
    cells: HashMap<(i32, i32), Vec<u32>>,
}

impl SegmentIndex {
    fn new(line: &[(f32, f32)], reach: f32) -> Self {
        let segments: Vec<_> = if line.len() == 1 {
            vec![(line[0], line[0])]
        } else {
            line.windows(2).map(|pair| (pair[0], pair[1])).collect()
        };
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (index, (a, b)) in segments.iter().enumerate() {
            let (x0, x1) = (a.0.min(b.0) - reach, a.0.max(b.0) + reach);
            let (y0, y1) = (a.1.min(b.1) - reach, a.1.max(b.1) + reach);
            for x in x0.floor() as i32..=x1.floor() as i32 {
                for y in y0.floor() as i32..=y1.floor() as i32 {
                    cells.entry((x, y)).or_default().push(index as u32);
                }
            }
        }
        Self { segments, cells }
    }

    /// Distance from `point` to the nearest segment among those bucketed with `tile`.
    fn distance(&self, tile: (i32, i32), point: (f32, f32)) -> f32 {
        let Some(list) = self.cells.get(&tile) else {
            return f32::MAX;
        };
        let mut best = f32::MAX;
        for index in list {
            let (a, b) = self.segments[*index as usize];
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
}

/// The part of a tile the band covers, as a mask.
fn band_mask(index: &SegmentIndex, half_width: f32, tile: (i32, i32)) -> Mask {
    let mut mask = EMPTY;
    for y in 0..GRID {
        for x in 0..GRID {
            let c = cell_centre(x, y);
            let point = (tile.0 as f32 + c[0], tile.1 as f32 + c[1]);
            if index.distance(tile, point) <= half_width {
                set(&mut mask, x, y);
            }
        }
    }
    mask
}

/// The cells that must be covered (nearer than `half - tolerance` to the centre line) and the
/// cells that may be (nearer than `half + tolerance`).
fn tolerance_masks(
    index: &SegmentIndex,
    half_width: f32,
    tolerance: f32,
    tile: (i32, i32),
) -> (Mask, Mask) {
    let (mut inner, mut outer) = (EMPTY, EMPTY);
    for y in 0..GRID {
        for x in 0..GRID {
            let c = cell_centre(x, y);
            let d = index.distance(tile, (tile.0 as f32 + c[0], tile.1 as f32 + c[1]));
            if d <= half_width - tolerance {
                set(&mut inner, x, y);
            }
            if d <= half_width + tolerance {
                set(&mut outer, x, y);
            }
        }
    }
    (inner, outer)
}

/// Mismatch of a candidate against the tolerant target (in cells): a cell that must be covered and
/// is not, a cell outside the allowed zone that is covered, and a small pull towards the ideal.
fn tolerant_cost(ideal: &Mask, inner: &Mask, outer: &Mask, candidate: &Mask, pull: f32) -> f32 {
    let mut missing = 0;
    let mut excess = 0;
    for w in 0..WORDS {
        missing += (inner[w] & !candidate[w]).count_ones();
        excess += (candidate[w] & !outer[w]).count_ones();
    }
    (missing + excess) as f32 + pull * cost(ideal, candidate) as f32
}

/// The knobs of the shape search (the playground sweeps them).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuning {
    /// Weight of the shape mismatch (a whole tile's worth of area). The fit leads: the shapes can
    /// follow a slanted edge closely, and a census-first choice turned slopes into stairs.
    pub geometry: f32,
    /// Weight of a fully mismatched, partly covered tile edge (a step in the outline).
    pub portal: f32,
    /// Weight of the census cost: it breaks ties towards what people actually place.
    pub prior: f32,
    /// What leaving a tile alone costs, against the census cost of a piece.
    pub keep: f32,
    /// Annealing sweeps for a small path (long paths get fewer) and the temperature range.
    pub sweeps: usize,
    pub start: f32,
    pub end: f32,
    /// A tile the band covers at least this many cells (of 256) of may carry a small piece.
    pub touched: u32,
    /// Weight of differences between neighbouring tiles' area errors: an outline whose error
    /// drifts smoothly looks straight; one whose error flips sign tile to tile looks jagged.
    pub smooth: f32,
    /// How far (tiles) the made outline may stray from the ideal one where that lets it run
    /// straighter: cells nearer than this to the ideal outline are "don't care" (apart from a
    /// small pull towards the ideal), so the search can pick one consistent stair instead of
    /// chasing every wiggle.
    pub tolerance: f32,
    /// Inside the free zone, how strongly the fit still prefers the ideal outline.
    pub pull: f32,
    /// Fit the outline first (long straight runs) instead of choosing every tile's piece alone.
    pub outline: Option<outline::OutlineTuning>,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            geometry: 120.0,
            portal: 3.0,
            prior: 0.25,
            keep: KEEP_COST,
            sweeps: ANNEAL_SWEEPS,
            start: ANNEAL_START,
            end: ANNEAL_END,
            touched: TOUCHED,
            smooth: 0.0,
            tolerance: 0.2,
            pull: 0.04,
            outline: None,
        }
    }
}

/// The mask a tile that already carries the overlay shows (`None`: it does not).
fn existing_mask(
    existing: &dyn Fn((i32, i32)) -> Option<(u8, u8)>,
    tile: (i32, i32),
) -> Option<Mask> {
    let (shape, rotation) = existing(tile)?;
    Some(if shape == 0 {
        FULL
    } else {
        shape_mask(shape, rotation)
    })
}

/// The tiles a path of `params` laid along `points` (world tile coordinates; a control point on tile
/// `(x, y)` is at `(x + 0.5, y + 0.5)` or anywhere inside it) turns into. Tiles that stay as they are
/// are not listed.
///
/// `existing(tile)` says whether a tile already carries the overlay being laid, and in which
/// `(shape, rotation)` (shape 0 = whole): those tiles count as part of the path, so a new path joins
/// an old one instead of leaving a seam.
pub fn generate(
    points: &[(f32, f32)],
    params: PathParams,
    existing: &dyn Fn((i32, i32)) -> Option<(u8, u8)>,
) -> Vec<((i32, i32), TileChoice)> {
    generate_with(points, params, existing, &Tuning::default())
}

/// [`generate`] with explicit search weights.
pub fn generate_with(
    points: &[(f32, f32)],
    params: PathParams,
    existing: &dyn Fn((i32, i32)) -> Option<(u8, u8)>,
    tuning: &Tuning,
) -> Vec<((i32, i32), TileChoice)> {
    if points.is_empty() {
        return Vec::new();
    }
    let half = (params.width / 2.0).max(0.5);
    let line = path_polyline(points, params, 0.25);
    let index = SegmentIndex::new(&line, half + 0.1);

    // The band mask of every tile it touches.
    let mut targets: HashMap<(i32, i32), Mask> = HashMap::new();
    let tiles: Vec<(i32, i32)> = index.cells.keys().copied().collect();
    let mut free: HashMap<(i32, i32), (Mask, Mask)> = HashMap::new();
    for tile in tiles {
        let mask = band_mask(&index, half, tile);
        if popcount(&mask) > 0 {
            targets.insert(tile, mask);
            if tuning.tolerance > 0.0 {
                free.insert(tile, tolerance_masks(&index, half, tuning.tolerance, tile));
            }
        }
    }

    // Step 1: which tiles belong to the path.
    let mut occupied: std::collections::BTreeSet<(i32, i32)> = targets
        .iter()
        .filter(|(_, mask)| popcount(mask) >= OCCUPIED)
        .map(|(tile, _)| *tile)
        .collect();
    if params.width < 2.0 {
        // A thin path must stay joined: every tile its centre line crosses belongs to it.
        for point in &line {
            occupied.insert((point.0.floor() as i32, point.1.floor() as i32));
        }
    }
    if params.style == EdgeStyle::Whole {
        return occupied
            .iter()
            .filter(|tile| existing(**tile).is_none_or(|(shape, _)| shape != 0))
            .map(|tile| (*tile, TileChoice::Full))
            .collect();
    }

    // Outline first, when the path is wide enough for its two sides to be fitted apart and there is
    // nothing already laid to join.
    let outline_tuning =
        tuning.outline.or(
            (params.style == EdgeStyle::Runs).then(|| outline::OutlineTuning {
                max_miss: params.straighten.clamp(0.25, 0.8),
                ..outline::OutlineTuning::default()
            }),
        );
    if let Some(outline_tuning) = &outline_tuning
        && params.width >= 1.9
        && !targets.keys().any(|tile| existing(*tile).is_some())
        && let Some(result) = outline::generate_outline(&line, half, params.style, outline_tuning)
    {
        return result;
    }

    // Step 2: a piece for every tile the band touches (a small corner shape can sit on a tile the
    // band only clips). Whole tiles that are already there stay.
    let list = candidates_for(params.style);
    let prior = prior();
    let work: Vec<(i32, i32)> = {
        let mut tiles: Vec<(i32, i32)> = targets
            .iter()
            .filter(|(_, mask)| popcount(mask) >= tuning.touched)
            .map(|(tile, _)| *tile)
            .chain(occupied.iter().copied())
            .filter(|tile| existing(*tile).is_none_or(|(shape, _)| shape != 0))
            .collect();
        tiles.sort();
        tiles.dedup();
        tiles
    };
    let target_of = |tile: (i32, i32)| targets.get(&tile).copied().unwrap_or(EMPTY);
    // Index into `list`; tiles not in the map are left alone.
    let mut chosen: HashMap<(i32, i32), usize> = HashMap::new();
    for tile in &occupied {
        if work.binary_search(tile).is_ok() {
            chosen.insert(*tile, 1);
        }
    }
    // The piece a neighbour shows right now: its pick, what is already there, or nothing.
    let neighbour_mask = |chosen: &HashMap<(i32, i32), usize>, tile: (i32, i32)| -> Mask {
        if let Some(i) = chosen.get(&tile) {
            return list[*i].mask;
        }
        existing_mask(existing, tile).unwrap_or(EMPTY)
    };
    let occupied_now = |chosen: &HashMap<(i32, i32), usize>, tile: (i32, i32)| -> bool {
        chosen.get(&tile).is_some_and(|i| *i != 0) || existing(tile).is_some()
    };
    let pattern_now = |chosen: &HashMap<(i32, i32), usize>, tile: (i32, i32)| -> usize {
        const AROUND: [(i32, i32); 8] = [
            (0, 1),
            (1, 1),
            (1, 0),
            (1, -1),
            (0, -1),
            (-1, -1),
            (-1, 0),
            (-1, 1),
        ];
        let mut pattern = 0_usize;
        for (bit, (dx, dy)) in AROUND.iter().enumerate() {
            if occupied_now(chosen, (tile.0 + dx, tile.1 + dy)) {
                pattern |= 1 << bit;
            }
        }
        pattern
    };
    // How badly two touching edges disagree: only a partly covered edge makes a step in the outline
    // (whole against empty is just the outline running along the tile border).
    let portal = |a: u16, b: u16| -> f32 {
        let partial = |bits: u16| bits != 0 && bits != u16::MAX;
        if partial(a) || partial(b) {
            (a ^ b).count_ones() as f32 / GRID as f32
        } else {
            0.0
        }
    };
    // What a tile costs if it shows candidate `i`, given what its neighbours show now.
    let energy = |chosen: &HashMap<(i32, i32), usize>, tile: (i32, i32), i: usize| -> f32 {
        let c = list[i];
        let target = target_of(tile);
        let shape = tuning.geometry
            * match free.get(&tile) {
                Some((inner, outer)) => tolerant_cost(&target, inner, outer, &c.mask, tuning.pull),
                None => cost(&target, &c.mask) as f32,
            }
            / 256.0;
        let census = if c.slot == KEEP_SLOT {
            tuning.keep
        } else {
            let pattern = pattern_now(chosen, tile);
            if pattern == 255 {
                // Surrounded by the path: whole, whatever else is asked.
                if c.slot == 0 { 0.0 } else { 12.0 }
            } else {
                prior.cost[pattern][c.slot]
            }
        };
        let side = |dx: i32, dy: i32, mine: Side, theirs: Side| {
            portal(
                edge(&c.mask, mine),
                edge(&neighbour_mask(chosen, (tile.0 + dx, tile.1 + dy)), theirs),
            )
        };
        let portals = side(-1, 0, Side::West, Side::East)
            + side(1, 0, Side::East, Side::West)
            + side(0, -1, Side::South, Side::North)
            + side(0, 1, Side::North, Side::South);
        // How far this tile's area is off, against its neighbours' (in tiles of area).
        let smooth = if tuning.smooth > 0.0 {
            let off = |mask: &Mask, tile: (i32, i32)| {
                (popcount(mask) as f32 - popcount(&target_of(tile)) as f32) / 256.0
            };
            let mine = off(&c.mask, tile);
            let mut sum = 0.0_f32;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let next = (tile.0 + dx, tile.1 + dy);
                if popcount(&target_of(next)) == 0 {
                    continue;
                }
                let theirs = off(&neighbour_mask(chosen, next), next);
                sum += (mine - theirs) * (mine - theirs);
            }
            sum
        } else {
            0.0
        };
        shape + tuning.prior * census + tuning.portal * portals + tuning.smooth * smooth
    };

    // Annealing first, so tiles can leave a locally good but jagged arrangement (a notch only
    // disappears when two neighbours change together), then greedy passes to settle.
    let mut order = work.clone();
    let mut rng =
        0x9E37_79B9_7F4A_7C15_u64 ^ (order.len() as u64).wrapping_mul(0xD1B5_4A32_D192_ED03);
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    // Long paths get fewer sweeps so a live preview stays responsive.
    let sweeps = (tuning.sweeps * 200 / work.len().max(200)).max(tuning.sweeps.min(40));
    for sweep in 0..sweeps {
        let temperature =
            tuning.start * (tuning.end / tuning.start).powf(sweep as f32 / sweeps as f32);
        for tile in &order {
            let current = chosen.get(tile).copied().unwrap_or(0);
            let before = energy(&chosen, *tile, current);
            let proposal = (next() % list.len() as u64) as usize;
            if proposal == current {
                continue;
            }
            let delta = energy(&chosen, *tile, proposal) - before;
            let accept = delta <= 0.0 || {
                let roll = (next() >> 11) as f32 / (1u64 << 53) as f32;
                roll < (-delta / temperature).exp()
            };
            if accept {
                chosen.insert(*tile, proposal);
            }
        }
    }
    for pass in 0..4 {
        if pass % 2 == 1 {
            order.reverse();
        }
        for tile in &order {
            let pick = (0..list.len())
                .min_by(|a, b| energy(&chosen, *tile, *a).total_cmp(&energy(&chosen, *tile, *b)));
            if let Some(pick) = pick {
                chosen.insert(*tile, pick);
            }
        }
    }

    let mut out: Vec<_> = chosen
        .iter()
        .filter(|(_, i)| **i != 0)
        .map(|(tile, i)| (*tile, list[*i].choice))
        .collect();
    // Old partial pieces now surrounded by the path become whole.
    let mut seen: std::collections::BTreeSet<(i32, i32)> = std::collections::BTreeSet::new();
    for tile in chosen.keys() {
        for dx in -1..=1 {
            for dy in -1..=1 {
                let near = (tile.0 + dx, tile.1 + dy);
                if chosen.contains_key(&near) || !seen.insert(near) {
                    continue;
                }
                if existing(near).is_some_and(|(shape, _)| shape != 0)
                    && pattern_now(&chosen, near) == 255
                {
                    out.push((near, TileChoice::Full));
                }
            }
        }
    }
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
    let line = path_polyline(points, params, 0.25);
    let index = SegmentIndex::new(&line, half + 0.1);
    let chosen: HashMap<(i32, i32), TileChoice> = result.iter().copied().collect();
    let mut tiles: std::collections::HashSet<(i32, i32)> = chosen.keys().copied().collect();
    tiles.extend(index.cells.keys().copied());
    tiles
        .into_iter()
        .map(|tile| {
            let mask = match chosen.get(&tile) {
                None | Some(TileChoice::Keep) => EMPTY,
                Some(TileChoice::Full) => FULL,
                Some(TileChoice::Shape { shape, rotation }) => shape_mask(*shape, *rotation),
            };
            cost(&band_mask(&index, half, tile), &mask)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share(shape: u8, rotation: u8) -> f32 {
        shape_cells(shape, rotation) as f32 / 256.0
    }

    fn none(_: (i32, i32)) -> Option<(u8, u8)> {
        None
    }

    fn params(width: f32) -> PathParams {
        PathParams {
            width,
            ..PathParams::default()
        }
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
        for (shape, want) in expected {
            let got = share(shape, 0);
            assert!(
                (got - want).abs() < 0.08,
                "shape {shape}: expected about {want}, rasterised {got}"
            );
        }
        assert_eq!(shape_cells(0, 0), 256);
    }

    #[test]
    fn rotations_of_a_corner_piece_cover_different_corners() {
        let masks: Vec<Mask> = (0..4).map(|r| shape_mask(2, r)).collect();
        for i in 0..4 {
            for j in i + 1..4 {
                assert_ne!(
                    masks[i], masks[j],
                    "rotations {i} and {j} of shape 2 are equal"
                );
            }
        }
        assert!(masks.iter().all(|m| popcount(m) == popcount(&masks[0])));
    }

    #[test]
    fn candidates_start_with_the_whole_tile_and_cover_every_shape() {
        let list = all_candidates();
        assert_eq!(list[0].choice, TileChoice::Keep);
        assert_eq!(list[1].choice, TileChoice::Full);
        assert!(list.len() > 30, "{} candidates", list.len());
    }

    #[test]
    fn the_census_knows_how_corners_and_edges_are_made() {
        let prior = prior();
        let best = |pattern: usize| {
            (0..SLOTS)
                .min_by(|a, b| prior.cost[pattern][*a].total_cmp(&prior.cost[pattern][*b]))
                .unwrap_or(0)
        };
        // Neighbours on every side: whole. A straight edge (west side open): whole is the usual.
        assert_eq!(best(255), 0);
        assert_eq!(best(0b0001_1111), 0);
        // A convex corner (only N, NE, E present, so the south-west is open) is cut with shape 1
        // or left whole, never anything exotic; rotating the neighbourhood rotates the piece.
        let corner = |turn: usize| {
            let pattern = 0b0000_0111_usize;
            ((pattern << (2 * turn)) | (pattern >> (8 - 2 * turn))) & 255
        };
        let probable = |pattern: usize| {
            let mut slots: Vec<usize> = (1..SLOTS).collect();
            slots.sort_by(|a, b| prior.cost[pattern][*a].total_cmp(&prior.cost[pattern][*b]));
            slots[0]
        };
        assert_eq!(probable(corner(0)), 4 + 2, "shape 1 rotation 2");
        assert_eq!(probable(corner(1)), 4 + 3, "shape 1 rotation 3");
    }

    #[test]
    fn a_long_path_is_laid_quickly_enough_for_a_live_preview() {
        let points = [(2.5, 2.5), (40.5, 20.5), (80.5, 5.5), (120.5, 30.5)];
        let started = std::time::Instant::now();
        let result = generate(&points, params(4.0), &none);
        let took = started.elapsed();
        assert!(!result.is_empty());
        assert!(
            took.as_millis()
                < if cfg!(debug_assertions) {
                    20_000
                } else {
                    1_500
                },
            "{took:?} for {} tiles",
            result.len()
        );
        eprintln!("{} tiles in {took:?}", result.len());
    }

    #[test]
    fn straight_runs_do_not_zigzag_where_the_close_fit_does() {
        // A shallow slope (about 15 degrees) is where a tile-by-tile fit alternates flat and 2:1.
        let points = [(3.5, 3.5), (33.5, 11.5)];
        let zigzags = |style: EdgeStyle| {
            let result = generate(
                &points,
                PathParams {
                    style,
                    ..params(3.0)
                },
                &none,
            );
            outline::outline_stats(&refine::result_masks(&result)).1
        };
        let (smooth, runs) = (zigzags(EdgeStyle::Smooth), zigzags(EdgeStyle::Runs));
        assert!(
            runs < smooth,
            "runs {runs} vs smooth {smooth} zigzags per ten tiles"
        );
        assert!(runs <= 1.0, "runs {runs} zigzags per ten tiles");
    }

    #[test]
    fn a_new_path_flows_into_an_old_one() {
        // An existing whole-tile road runs along y = 10.5; a path starting on it and leaving to
        // the north must not cut its own tiles or leave a seam.
        let road = |tile: (i32, i32)| {
            ((5..=25).contains(&tile.0) && (9..=11).contains(&tile.1)).then_some((0, 0))
        };
        let points = [(15.5, 10.5), (15.5, 25.5)];
        let result = generate(&points, params(3.0), &road);
        assert!(
            result
                .iter()
                .all(|((x, y), _)| !(14..=16).contains(x) || *y >= 12 || *y <= 8),
            "tiles of the road were rewritten: {:?}",
            result
                .iter()
                .filter(|((x, y), _)| (14..=16).contains(x) && (9..=11).contains(y))
                .collect::<Vec<_>>()
        );
        // Beside the junction the road's edge and the new path's edge are whole (no notch).
        let junction = |x: i32, y: i32| result.iter().find(|(t, _)| *t == (x, y)).map(|(_, c)| *c);
        for x in 14..=16 {
            assert!(
                matches!(junction(x, 12), Some(TileChoice::Full)),
                "tile {x}, 12: {:?}",
                junction(x, 12)
            );
        }
    }

    #[test]
    fn a_tile_aligned_straight_band_is_all_full_tiles() {
        // Centre line y = 10.5 (the middle of tile row 10), width 3: rows 9, 10 and 11 exactly.
        let points = [(0.5, 10.5), (20.5, 10.5)];
        let result = generate(&points, params(3.0), &none);
        assert!(result.iter().any(|(_, c)| *c == TileChoice::Full));
        for ((x, y), choice) in &result {
            if (9..=11).contains(y) && (3..=17).contains(x) {
                assert_eq!(*choice, TileChoice::Full, "tile {x}, {y}");
            }
        }
        // Nothing outside the band is touched except the rounded ends.
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
        let result = generate(&points, params(3.0), &none);
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
        let smooth = params(4.0);
        let whole = PathParams {
            style: EdgeStyle::Whole,
            ..smooth
        };
        let smooth_error = total_error(&points, smooth, &generate(&points, smooth, &none));
        let whole_error = total_error(&points, smooth, &generate(&points, whole, &none));
        assert!(
            smooth_error < whole_error,
            "smooth {smooth_error} vs whole {whole_error}"
        );
    }

    #[test]
    fn whole_tiles_never_use_shapes_and_chiselled_never_uses_rounded_ones() {
        let points = [(2.5, 2.5), (18.5, 11.5), (24.5, 3.5)];
        let whole = generate(
            &points,
            PathParams {
                style: EdgeStyle::Whole,
                ..params(4.0)
            },
            &none,
        );
        assert!(whole.iter().all(|(_, c)| *c == TileChoice::Full));
        let chiselled = generate(
            &points,
            PathParams {
                style: EdgeStyle::Chiselled,
                ..params(4.0)
            },
            &none,
        );
        assert!(
            chiselled
                .iter()
                .all(|(_, c)| !matches!(c, TileChoice::Shape { shape: 9..=11, .. }))
        );
        assert!(
            chiselled
                .iter()
                .any(|(_, c)| matches!(c, TileChoice::Shape { .. }))
        );
    }

    #[test]
    fn slivers_do_not_become_shaped_tiles() {
        // The band's edges sit 0.02 tiles into rows 8 and 12: far too little to shape a tile.
        let points = [(0.5, 10.5), (20.5, 10.5)];
        let result = generate(&points, params(3.04), &none);
        for ((x, y), choice) in &result {
            if (4..=16).contains(x) && (*y == 8 || *y == 12) {
                assert_eq!(
                    *choice,
                    TileChoice::Keep,
                    "tile {x}, {y} should not be listed"
                );
            }
        }
    }

    #[test]
    fn the_result_is_deterministic() {
        let points = [(2.5, 2.5), (18.5, 11.5), (24.5, 3.5)];
        assert_eq!(
            generate(&points, params(3.0), &none),
            generate(&points, params(3.0), &none)
        );
    }

    #[test]
    fn a_single_point_makes_a_small_blob_and_no_points_make_nothing() {
        assert!(generate(&[], params(3.0), &none).is_empty());
        assert!(!generate(&[(10.5, 10.5)], params(3.0), &none).is_empty());
    }

    #[test]
    fn the_curve_passes_through_its_points_and_lines_are_straight() {
        let points = [(0.0, 0.0), (5.0, 3.0), (9.0, 0.0)];
        for curve in [CurveKind::Curve, CurveKind::Lines] {
            let line = path_polyline(
                &points,
                PathParams {
                    curve,
                    ..params(3.0)
                },
                0.1,
            );
            for p in points {
                assert!(
                    line.iter()
                        .any(|q| (q.0 - p.0).abs() < 0.05 && (q.1 - p.1).abs() < 0.05),
                    "{p:?} not on the {curve:?}"
                );
            }
        }
        let lines = path_polyline(
            &points[..2],
            PathParams {
                curve: CurveKind::Lines,
                ..params(3.0)
            },
            0.5,
        );
        for q in &lines {
            assert!(
                (q.1 - q.0 * 0.6).abs() < 1e-3,
                "{q:?} is off the straight line"
            );
        }
    }

    #[test]
    fn a_centripetal_curve_does_not_overshoot_uneven_points() {
        // Two close points then a far one: uniform Catmull-Rom loops here, centripetal does not.
        let points = [(0.0, 0.0), (1.0, 0.0), (1.2, 1.0), (30.0, 1.0)];
        let line = path_polyline(&points, params(3.0), 0.1);
        let lowest = line.iter().map(|p| p.1).fold(f32::MAX, f32::min);
        let highest = line.iter().map(|p| p.1).fold(f32::MIN, f32::max);
        // A uniform Catmull-Rom swings several tiles here; the centripetal one stays within a tile or so.
        assert!(
            lowest > -0.6 && highest < 1.9,
            "y range {lowest}..{highest}"
        );
    }

    #[test]
    fn a_closed_path_returns_to_its_start() {
        let points = [(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)];
        let line = path_polyline(
            &points,
            PathParams {
                closed: true,
                ..params(3.0)
            },
            0.25,
        );
        let (first, last) = (line[0], line[line.len() - 1]);
        assert!(dist(first, last) < 0.05, "{first:?} .. {last:?}");
        let open = path_polyline(&points, params(3.0), 0.25);
        assert!(dist(open[0], open[open.len() - 1]) > 5.0);
    }

    #[test]
    fn the_ramp_parameter_grows_along_the_path() {
        let line = path_polyline(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], params(3.0), 0.25);
        let (a, b, c) = (
            ramp_parameter(&line, (1.0, 0.5)),
            ramp_parameter(&line, (10.0, 5.0)),
            ramp_parameter(&line, (10.0, 9.5)),
        );
        assert!(a < b && b < c && a >= 0.0 && c <= 1.0, "{a} {b} {c}");
    }

    #[test]
    fn wall_band_of_thickness_1_follows_a_straight_line() {
        let line = path_polyline(&[(0.5, 0.5), (5.5, 0.5)], params(3.0), 0.25);
        let band = rasterize_band(&line, 1.0);
        for x in 0..=5 {
            assert!(band.tiles.contains(&(x, 0)), "missing ({x}, 0)");
        }
        assert_eq!(band.tiles.len(), 6);
        // Tangents point east everywhere.
        for tile in &band.tiles {
            let (tx, ty) = band.tangents[tile];
            assert!(tx > 0.99 && ty.abs() < 0.01, "{tile:?}: ({tx:?}, {ty:?})");
        }
    }

    #[test]
    fn wall_band_thickness_counts_tiles_exactly() {
        let line = path_polyline(&[(0.5, 2.5), (6.5, 2.5)], params(3.0), 0.25);
        // Odd thickness 3 stamps y in 1..=3; even 4 stamps 0..=3 (floor coordinates).
        let odd = rasterize_band(&line, 3.0);
        assert!(odd.tiles.contains(&(3, 1)) && odd.tiles.contains(&(3, 3)));
        assert!(!odd.tiles.contains(&(3, 0)) && !odd.tiles.contains(&(3, 4)));
        let even = rasterize_band(&line, 4.0);
        assert!(even.tiles.contains(&(3, 0)) && even.tiles.contains(&(3, 3)));
        assert!(!even.tiles.contains(&(3, -1)) && !even.tiles.contains(&(3, 4)));
    }

    #[test]
    fn wall_band_diagonal_runs_diagonally_with_diagonal_tangents() {
        let line = path_polyline(&[(0.5, 0.5), (4.5, 4.5)], params(3.0), 0.25);
        let band = rasterize_band(&line, 1.0);
        for d in 0..=4 {
            assert!(band.tiles.contains(&(d, d)), "missing ({d}, {d})");
        }
        for tile in &band.tiles {
            let (tx, ty) = band.tangents[tile];
            assert!(
                (tx - ty).abs() < 0.05 && tx > 0.5,
                "{tile:?}: ({tx:?}, {ty:?})"
            );
        }
    }

    #[test]
    fn wall_band_needs_two_points() {
        let empty = rasterize_band(&[], 3.0);
        assert!(empty.tiles.is_empty() && empty.tangents.is_empty());
        let single = rasterize_band(&[(1.0, 1.0)], 3.0);
        assert!(single.tiles.is_empty() && single.tangents.is_empty());
    }

    #[test]
    fn generate_walls_lays_band_and_towers_whole() {
        let points = [(0.5, 0.5), (6.5, 0.5)];
        let plan = generate_walls(&points, params(3.0), 1.0, 3.0);
        assert!(!plan.tiles.is_empty());
        assert!(
            plan.tiles
                .iter()
                .all(|(_, choice)| *choice == TileChoice::Full)
        );
        // Towers: 3x3 squares around both control points (rounded to tiles 0 and 6).
        for (x, y) in [(0, 0), (6, 0), (-1, -1), (7, 1), (0, 1), (6, -1)] {
            assert!(
                plan.tiles.iter().any(|(t, _)| *t == (x, y)),
                "missing tower tile ({x}, {y})"
            );
        }
        // Band row present between the towers.
        assert!(plan.tiles.iter().any(|(t, _)| *t == (3, 0)));
        // Tangents cover the band.
        assert!(plan.tangents.contains_key(&(3, 0)));
        // Fewer than two points: nothing.
        let none = generate_walls(&[(1.0, 1.0)], params(3.0), 1.0, 3.0);
        assert!(none.tiles.is_empty() && none.tangents.is_empty());
    }
}

mod outline;
mod refine;
#[cfg(test)]
use refine::measure;
pub use refine::{Refined, generate_refined};
pub mod walls;

#[cfg(test)]
mod playground;

/// Draws what the tiler does to a few paths into `/tmp/autotile-*.png` (run with
/// `cargo test -p osrs-editor draw_paths -- --ignored`): tiles filled as the shapes say, the ideal
/// band outlined.
#[cfg(test)]
mod draw {
    use super::*;

    const PX: i32 = 28;

    fn render(name: &str, points: &[(f32, f32)], params: PathParams) {
        let none = |_: (i32, i32)| None;
        let result = generate(points, params, &none);
        let chosen: HashMap<(i32, i32), TileChoice> = result.iter().copied().collect();
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for p in points {
            x0 = x0.min(p.0 as i32 - 4);
            y0 = y0.min(p.1 as i32 - 4);
            x1 = x1.max(p.0 as i32 + 5);
            y1 = y1.max(p.1 as i32 + 5);
        }
        let (w, h) = (((x1 - x0) * PX) as u32, ((y1 - y0) * PX) as u32);
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([70, 110, 50, 255]));
        let half = (params.width / 2.0).max(0.5);
        let line = path_polyline(points, params, 0.25);
        let index = SegmentIndex::new(&line, half + 0.1);
        for py in 0..h as i32 {
            for px in 0..w as i32 {
                let wx = x0 as f32 + (px as f32 + 0.5) / PX as f32;
                let wy = y1 as f32 - (py as f32 + 0.5) / PX as f32;
                let tile = (wx.floor() as i32, wy.floor() as i32);
                let (fx, fy) = (wx - tile.0 as f32, wy - tile.1 as f32);
                let inside = match chosen.get(&tile) {
                    Some(TileChoice::Full) => true,
                    Some(TileChoice::Shape { shape, rotation }) => {
                        shape_triangles(*shape, *rotation)
                            .iter()
                            .any(|(t, overlay)| *overlay && in_triangle([fx, fy], t))
                    }
                    _ => false,
                };
                if inside {
                    img.put_pixel(px as u32, py as u32, image::Rgba([190, 160, 90, 255]));
                }
                // The ideal band's outline.
                let d = index.distance(tile, (wx, wy));
                if (d - half).abs() < 0.5 / PX as f32 * 1.2 {
                    img.put_pixel(px as u32, py as u32, image::Rgba([220, 50, 50, 255]));
                }
                if fx < 0.02 || fy < 0.02 {
                    let p = img.get_pixel_mut(px as u32, py as u32);
                    p.0 = [p.0[0] / 2 + 20, p.0[1] / 2 + 20, p.0[2] / 2 + 20, 255];
                }
            }
        }
        let _ = img.save(format!("/tmp/autotile-{name}.png"));
    }

    #[test]
    #[ignore = "writes images for looking at"]
    fn draw_paths() {
        let p = |width: f32| PathParams {
            width,
            ..PathParams::default()
        };
        render("diag3", &[(2.5, 2.5), (22.5, 12.5)], p(3.0));
        render("steep4", &[(2.5, 2.5), (8.5, 24.5)], p(4.0));
        render(
            "s-curve",
            &[(2.5, 2.5), (12.5, 8.5), (22.5, 4.5), (30.5, 14.5)],
            p(4.0),
        );
        render("thin", &[(2.5, 2.5), (14.5, 9.5), (24.5, 8.5)], p(2.0));
        render("wide7", &[(3.5, 3.5), (14.2, 12.7), (28.5, 9.5)], p(7.0));
        render("field", &[(3.5, 3.5), (8.5, 13.5), (4.5, 25.5)], p(3.0));
        render("w1", &[(2.5, 2.5), (20.5, 11.5)], p(1.0));
        render(
            "loop",
            &[(10.5, 6.5), (22.5, 6.5), (22.5, 18.5), (10.5, 18.5)],
            PathParams {
                closed: true,
                ..p(3.0)
            },
        );
        render(
            "lines5",
            &[(2.5, 2.5), (12.5, 14.5), (22.5, 4.5), (30.5, 12.5)],
            PathParams {
                curve: CurveKind::Lines,
                ..p(5.0)
            },
        );
        render(
            "tight",
            &[
                (3.2, 3.7),
                (8.1, 9.3),
                (12.9, 3.1),
                (17.4, 9.6),
                (22.2, 3.4),
            ],
            p(3.0),
        );
    }
}

#[cfg(test)]
mod dump {
    use super::*;

    #[test]
    #[ignore = "prints the shape alphabet"]
    fn dump_shapes() {
        for shape in 1..=11_u8 {
            println!("shape {shape} rotation 0 (y up), tris:");
            for (t, overlay) in shape_triangles(shape, 0) {
                println!("   overlay={overlay} {:?}", t);
            }
            let m = shape_mask(shape, 0);
            for y in (0..16).rev().step_by(2) {
                let row: String = (0..16)
                    .step_by(1)
                    .map(|x| if get(&m, x, y) { '#' } else { '.' })
                    .collect();
                println!("   {row}");
            }
        }
    }
}
