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
use std::collections::HashMap;
use std::sync::OnceLock;

const GRID: usize = 16;
const WORDS: usize = GRID * GRID / 64;
/// Annealing: sweeps over the tiles, and the temperature range (in cost units).
const ANNEAL_SWEEPS: usize = 160;
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

/// Weight of the shape mismatch (a whole tile's worth of area) against the census cost.
/// Weight of the shape mismatch (a whole tile's worth of area) against the census cost. The fit leads:
/// the shapes can follow a slanted edge closely, and a census-first choice turned slopes into stairs.
const GEOMETRY_WEIGHT: f32 = 30.0;
/// Weight of a fully mismatched, partly covered tile edge (a step in the outline).
const PORTAL_WEIGHT: f32 = 3.0;
/// Weight of the census cost: it breaks ties towards what people actually place.
const PRIOR_WEIGHT: f32 = 0.25;
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

    // Step 2: a piece for every tile the band touches (a small corner shape can sit on a tile the
    // band only clips). Whole tiles that are already there stay.
    let list = candidates_for(params.style);
    let prior = prior();
    let work: Vec<(i32, i32)> = {
        let mut tiles: Vec<(i32, i32)> = targets
            .iter()
            .filter(|(_, mask)| popcount(mask) >= TOUCHED)
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
        let shape = GEOMETRY_WEIGHT * cost(&target, &c.mask) as f32 / 256.0;
        let census = if c.slot == KEEP_SLOT {
            KEEP_COST
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
        shape + PRIOR_WEIGHT * census + PORTAL_WEIGHT * portals
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
    let sweeps = (ANNEAL_SWEEPS * 200 / work.len().max(200)).max(40);
    for sweep in 0..sweeps {
        let temperature =
            ANNEAL_START * (ANNEAL_END / ANNEAL_START).powf(sweep as f32 / sweeps as f32);
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
}

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
