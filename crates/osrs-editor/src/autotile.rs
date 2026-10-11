//! The auto-tiler: turn a smooth path into tiles that look hand-made.
//!
//! An OSRS tile holds one overlay in one of 12 shapes and 4 rotations (the real tables in
//! `osrs-scene::terrain`). Painting a path means choosing, for every tile the path touches, the
//! (shape, rotation) whose overlay best matches the part of the tile the path covers.
//!
//! Every tile is rasterised to a 16 x 16 sub-tile mask (256 bits, bit `y * 16 + x`, `y` up). All
//! candidate shapes are rasterised from the real face tables once; the path band is rasterised per
//! tile; the best candidate is the one with the smallest Hamming distance. Slivers (a few percent of a
//! tile) are dropped and almost-full tiles become full, so the edge does not fray. Further passes
//! compare each tile's edge with its neighbours' ("portals") so pieces connect instead of leaving
//! notches, which is what makes the result read as intentional rather than as independent guesses.
//!
//! The path itself is a centripetal Catmull-Rom spline (no loops or overshoot when points are unevenly
//! spaced), or straight lines, optionally closed.

use osrs_core::coords::SceneTile;
use osrs_scene::terrain::{
    ShapedTerrainInput, ShapedTerrainSurface, TerrainColorSource, TerrainCorners,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

const GRID: usize = 16;
const WORDS: usize = GRID * GRID / 64;
/// Fewer covered sub-cells than this (of 256) and a tile is left alone.
const SLIVER: u32 = 14;
/// More covered sub-cells than this and a tile is filled completely.
const NEARLY_FULL: u32 = 242;

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
}

impl EdgeStyle {
    pub const ALL: [Self; 3] = [Self::Smooth, Self::Chiselled, Self::Whole];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Smooth => "Smooth",
            Self::Chiselled => "Chiselled",
            Self::Whole => "Whole tiles",
        }
    }

    pub const fn hint(self) -> &'static str {
        match self {
            Self::Smooth => "Rounded edge pieces: organic paths and rivers",
            Self::Chiselled => "Straight and diagonal cuts only: roads and courtyards",
            Self::Whole => "Full tiles, no shaped edges",
        }
    }

    /// Whether a shape (file value `1..=11`) may be used.
    fn allows(self, shape: u8) -> bool {
        match self {
            Self::Smooth => true,
            Self::Chiselled => !matches!(shape, 9..=11),
            Self::Whole => false,
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
}

/// Every candidate, simplest first: Keep, Full, then shapes by file value and rotation.
fn all_candidates() -> &'static [Candidate] {
    static CANDIDATES: OnceLock<Vec<Candidate>> = OnceLock::new();
    CANDIDATES.get_or_init(|| {
        let mut list = vec![
            Candidate {
                choice: TileChoice::Keep,
                mask: EMPTY,
            },
            Candidate {
                choice: TileChoice::Full,
                mask: FULL,
            },
        ];
        for shape in 1..=11_u8 {
            for rotation in 0..4_u8 {
                let mask = shape_mask(shape, rotation);
                // Skip empty shapes and masks an earlier (simpler) shape already has.
                if popcount(&mask) == 0 || list.iter().any(|candidate| candidate.mask == mask) {
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
}

impl Default for PathParams {
    fn default() -> Self {
        Self {
            width: 3.0,
            style: EdgeStyle::Smooth,
            curve: CurveKind::Curve,
            closed: false,
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

/// The path as a dense polyline (points about `step` tiles apart) in world tile coordinates.
pub fn path_polyline(points: &[(f32, f32)], params: PathParams, step: f32) -> Vec<(f32, f32)> {
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

/// The tiles a path of `params` laid along `points` (world tile coordinates; a control point on tile
/// `(x, y)` is at `(x + 0.5, y + 0.5)` or anywhere inside it) turns into. Tiles that stay as they are
/// are not listed.
pub fn generate(points: &[(f32, f32)], params: PathParams) -> Vec<((i32, i32), TileChoice)> {
    if points.is_empty() {
        return Vec::new();
    }
    let half = (params.width / 2.0).max(0.5);
    let line = path_polyline(points, params, 0.25);
    let index = SegmentIndex::new(&line, half + 0.1);

    // The band mask of every tile it touches.
    let mut targets: HashMap<(i32, i32), Mask> = HashMap::new();
    let tiles: Vec<(i32, i32)> = index.cells.keys().copied().collect();
    for tile in tiles {
        let mask = band_mask(&index, half, tile);
        if popcount(&mask) > 0 {
            targets.insert(tile, mask);
        }
    }

    let list = candidates_for(params.style);
    if params.style == EdgeStyle::Whole {
        let mut out: Vec<_> = targets
            .iter()
            .filter(|(_, mask)| popcount(mask) >= 128)
            .map(|(tile, _)| (*tile, TileChoice::Full))
            .collect();
        out.sort_by_key(|(tile, _)| *tile);
        return out;
    }

    // First choice: the smallest mismatch. Slivers and nearly full tiles snap to the simple answer
    // (the list puts Keep and Full first, so ties go there too).
    let mut chosen: HashMap<(i32, i32), usize> = HashMap::new();
    for (tile, target) in &targets {
        let covered = popcount(target);
        let best = if covered < SLIVER {
            0
        } else if covered > NEARLY_FULL {
            1
        } else {
            (0..list.len())
                .min_by_key(|i| cost(target, &list[*i].mask))
                .unwrap_or(0)
        };
        chosen.insert(*tile, best);
    }

    // Portal passes: among near-best candidates, prefer the one whose edges agree with the
    // neighbours' current choice.
    let neighbour = |chosen: &HashMap<(i32, i32), usize>, tile: (i32, i32)| -> Mask {
        chosen.get(&tile).map_or(EMPTY, |i| list[*i].mask)
    };
    let mut order: Vec<(i32, i32)> = targets.keys().copied().collect();
    order.sort();
    for pass in 0..4 {
        if pass % 2 == 1 {
            order.reverse();
        }
        for tile in &order {
            let target = &targets[tile];
            let covered = popcount(target);
            if !(SLIVER..=NEARLY_FULL).contains(&covered) {
                continue;
            }
            let west = edge(&neighbour(&chosen, (tile.0 - 1, tile.1)), Side::East);
            let east = edge(&neighbour(&chosen, (tile.0 + 1, tile.1)), Side::West);
            let south = edge(&neighbour(&chosen, (tile.0, tile.1 - 1)), Side::North);
            let north = edge(&neighbour(&chosen, (tile.0, tile.1 + 1)), Side::South);
            let best_cost = list
                .iter()
                .map(|candidate| cost(target, &candidate.mask))
                .min()
                .unwrap_or(0);
            let score = |mask: &Mask| {
                let portals = (edge(mask, Side::West) ^ west).count_ones()
                    + (edge(mask, Side::East) ^ east).count_ones()
                    + (edge(mask, Side::South) ^ south).count_ones()
                    + (edge(mask, Side::North) ^ north).count_ones();
                // The shape error counts once, a mismatched edge cell twice: a slightly worse shape
                // that joins its neighbours beats a better one that leaves a notch.
                cost(target, mask) + 2 * portals
            };
            let pick = (0..list.len())
                .filter(|i| cost(target, &list[*i].mask) <= best_cost + 24)
                .min_by_key(|i| score(&list[*i].mask));
            if let Some(pick) = pick {
                chosen.insert(*tile, pick);
            }
        }
    }

    let mut out: Vec<_> = chosen
        .into_iter()
        .map(|(tile, i)| (tile, list[i].choice))
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
    fn candidates_are_distinct_and_start_with_keep_and_full() {
        let list = all_candidates();
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
        let result = generate(&points, params(3.0));
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
        let result = generate(&points, params(3.0));
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
        let smooth_error = total_error(&points, smooth, &generate(&points, smooth));
        let whole_error = total_error(&points, smooth, &generate(&points, whole));
        assert!(
            smooth_error * 2 < whole_error,
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
        );
        assert!(whole.iter().all(|(_, c)| *c == TileChoice::Full));
        let chiselled = generate(
            &points,
            PathParams {
                style: EdgeStyle::Chiselled,
                ..params(4.0)
            },
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
        let result = generate(&points, params(3.04));
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
            generate(&points, params(3.0)),
            generate(&points, params(3.0))
        );
    }

    #[test]
    fn a_single_point_makes_a_small_blob_and_no_points_make_nothing() {
        assert!(generate(&[], params(3.0)).is_empty());
        assert!(!generate(&[(10.5, 10.5)], params(3.0)).is_empty());
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
}
