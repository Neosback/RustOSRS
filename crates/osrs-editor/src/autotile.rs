//! The path tiler: turn control-point paths into overlay tiles, old-tool rules.
//!
//! Control points are tile integers. The curve is a uniform Catmull-Rom spline; samples stamp
//! squares (the path width) that merge into a set, and each tile's 4-bit neighbour mask picks its
//! overlay shape and rotation from fixed per-style tables (SOLID/WEDGE/SMOOTH). WALL mode lays
//! every band tile whole and spawns castle walls instead of shaping edges.

use osrs_core::coords::SceneTile;
use osrs_scene::terrain::{
    ShapedTerrainInput, ShapedTerrainSurface, TerrainColorSource, TerrainCorners,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
/// (otherwise underlay). Built from the real terrain tables so thumbnails and the tiler cannot
/// drift.
pub fn shape_triangles(shape: u8, rotation: u8) -> Vec<([[f32; 2]; 3], bool)> {
    let flat = TerrainCorners::new(0, 0, 0, 0);
    // The builder indexes its tables with the file shape plus one (rows 0 and 1 are "all underlay"
    // and "all overlay").
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

/// How the path's edge tiles are cut (the old brush styles).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EdgeStyle {
    /// Whole tiles, shaped tips only.
    #[default]
    #[serde(alias = "Whole")]
    Solid,
    /// Pointed wedge tips and straight edges.
    #[serde(alias = "Chiselled")]
    Wedge,
    /// Rounded concave corners.
    #[serde(alias = "Runs")]
    Smooth,
    /// Castle walls along the path: whole tiles plus spawned wall objects.
    Walls,
}

impl EdgeStyle {
    pub const ALL: [Self; 4] = [Self::Solid, Self::Wedge, Self::Smooth, Self::Walls];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Solid => "Solid",
            Self::Wedge => "Wedge",
            Self::Smooth => "Smooth",
            Self::Walls => "Walls",
        }
    }

    pub const fn hint(self) -> &'static str {
        match self {
            Self::Solid => "Whole tiles with shaped tips",
            Self::Wedge => "Pointed wedge tips and straight edges",
            Self::Smooth => "Rounded concave corners",
            Self::Walls => "Castle wall along the path: whole tiles plus spawned wall objects",
        }
    }
}

/// `(shape, rotation)` per 4-bit neighbour mask, mirroring the old edge tables.
const SOLID_EDGES: [(u8, u8); 16] = [
    (0, 0),
    (1, 0),
    (1, 0),
    (1, 2), // 0..=3
    (1, 3),
    (0, 0),
    (1, 3),
    (0, 0), // 4..=7
    (1, 2),
    (1, 1),
    (0, 0),
    (0, 0), // 8..=11
    (1, 0),
    (0, 0),
    (0, 0),
    (0, 0), // 12..=15
];
const WEDGE_EDGES: [(u8, u8); 16] = [
    (0, 0),
    (6, 3),
    (6, 2),
    (1, 2), // 0..=3
    (6, 1),
    (0, 0),
    (1, 3),
    (6, 2), // 4..=7
    (6, 0),
    (1, 1),
    (0, 0),
    (6, 0), // 8..=11
    (1, 0),
    (6, 3),
    (6, 1),
    (0, 0), // 12..=15
];
const SMOOTH_EDGES: [(u8, u8); 16] = [
    (0, 0),
    (11, 2),
    (11, 2),
    (11, 0), // 0..=3
    (11, 1),
    (0, 0),
    (11, 1),
    (0, 0), // 4..=7
    (11, 0),
    (11, 3),
    (0, 0),
    (0, 0), // 8..=11
    (11, 2),
    (0, 0),
    (0, 0),
    (0, 0), // 12..=15
];

fn edge_table(style: EdgeStyle) -> [(u8, u8); 16] {
    match style {
        EdgeStyle::Solid | EdgeStyle::Walls => SOLID_EDGES,
        EdgeStyle::Wedge => WEDGE_EDGES,
        EdgeStyle::Smooth => SMOOTH_EDGES,
    }
}

/// 4-bit occupancy of the N/E/S/W neighbours (bit 0 = y+1, bit 1 = x+1, bit 2 = y-1,
/// bit 3 = x-1), mirroring the old `neighbourMask`.
fn neighbour_mask(set: &HashSet<(i32, i32)>, x: i32, y: i32) -> usize {
    let mut mask = 0_usize;
    if set.contains(&(x, y + 1)) {
        mask |= 1;
    }
    if set.contains(&(x + 1, y)) {
        mask |= 2;
    }
    if set.contains(&(x, y - 1)) {
        mask |= 4;
    }
    if set.contains(&(x - 1, y)) {
        mask |= 8;
    }
    mask
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Uniform Catmull-Rom kernel (the old `catmullRom`).
fn catmull_rom(
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    p3: (f32, f32),
    t: f32,
) -> (f32, f32) {
    let t2 = t * t;
    let t3 = t2 * t;
    let axis = |a: f32, b: f32, c: f32, d: f32| {
        0.5 * (2.0 * b
            + (-a + c) * t
            + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
            + (-a + 3.0 * b - 3.0 * c + d) * t3)
    };
    (axis(p0.0, p1.0, p2.0, p3.0), axis(p0.1, p1.1, p2.1, p3.1))
}

/// Uniform Catmull-Rom samples of tile control points, `step` tiles apart (the old `evaluate`):
/// clamped ends, adaptive subdivision (at least 2 per segment), no duplicate joints.
pub fn evaluate(points: &[(i32, i32)], step: f32) -> Vec<(f32, f32)> {
    if points.len() < 2 {
        return Vec::new();
    }
    let at = |index: isize| -> (f32, f32) {
        let clamped = index.clamp(0, points.len() as isize - 1) as usize;
        let (x, y) = points[clamped];
        (x as f32, y as f32)
    };
    let mut out = Vec::new();
    for segment in 0..points.len() - 1 {
        let s = segment as isize;
        let (p0, p1, p2, p3) = (at(s - 1), at(s), at(s + 1), at(s + 2));
        let count = ((dist(p1, p2) / step.max(0.25)).ceil() as usize).max(2);
        for k in 0..=count {
            if segment > 0 && k == 0 {
                continue;
            }
            out.push(catmull_rom(p0, p1, p2, p3, k as f32 / count as f32));
        }
    }
    out
}

/// Square-stamp rasterize (the old `rasterize`): each sample stamps a `width_tiles` square;
/// even widths shift the far edge in by one so the count stays exact.
pub fn rasterize(points: &[(i32, i32)], width_tiles: u32, step: f32) -> HashSet<(i32, i32)> {
    let line = evaluate(points, step);
    let mut set = HashSet::new();
    let width = width_tiles.max(1) as i32;
    let half = width / 2;
    let even = width % 2 == 0;
    for (x, y) in line {
        let (cx, cy) = (x.round() as i32, y.round() as i32);
        for dx in -half..=(half - if even { 1 } else { 0 }) {
            for dy in -half..=(half - if even { 1 } else { 0 }) {
                set.insert((cx + dx, cy + dy));
            }
        }
    }
    set
}

/// Tile the path with the style's edge table (the old non-WALL generate): every stamped tile is
/// painted, interiors whole, tips and corners shaped. Deterministic order.
pub fn generate(
    points: &[(i32, i32)],
    style: EdgeStyle,
    width: f32,
) -> Vec<((i32, i32), TileChoice)> {
    if points.len() < 2 {
        return Vec::new();
    }
    let set = rasterize(points, width.max(1.0).round() as u32, 0.4);
    if style == EdgeStyle::Walls {
        let mut out: Vec<_> = set
            .into_iter()
            .map(|tile| (tile, TileChoice::Full))
            .collect();
        out.sort_by_key(|(tile, _)| *tile);
        return out;
    }
    let table = edge_table(style);
    let mut out: Vec<_> = set
        .iter()
        .map(|&tile| {
            let (shape, rotation) = table[neighbour_mask(&set, tile.0, tile.1)];
            let choice = if shape == 0 {
                TileChoice::Full
            } else {
                TileChoice::Shape { shape, rotation }
            };
            (tile, choice)
        })
        .collect();
    out.sort_by_key(|(tile, _)| *tile);
    out
}

/// Polyline spacing for the wall band rasterizer.
const BAND_STEP: f32 = 0.4;

/// A rasterized castle-wall band: the tiles plus the averaged unit tangent per tile (wall
/// direction info for diagonal detection). Mirrors the old `rasterizeBandWithTangents` band walk.
pub struct BandTiles {
    pub tiles: HashSet<(i32, i32)>,
    pub tangents: HashMap<(i32, i32), (f32, f32)>,
}

/// Stamp the tiles a dense polyline covers `thickness` tiles wide, walking each sample's
/// perpendicular, and average the unit tangent per tile. Tile coordinates use round-half-up like
/// the old tool (control points are tile integers, so samples land on exact rows/columns).
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
        for k in -half..=(half - if even { 1 } else { 0 }) {
            let tile = (
                (x + nx * k as f32).round() as i32,
                (y + ny * k as f32).round() as i32,
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
/// tangents for wall emission.
pub struct WallPlan {
    pub tiles: Vec<((i32, i32), TileChoice)>,
    pub tangents: HashMap<(i32, i32), (f32, f32)>,
}

/// Lay a castle-wall band `thickness` tiles wide with `tower_size` squares around each control
/// point (size 1 disables towers, as in the old tool). Every covered tile is laid whole.
pub fn generate_walls(points: &[(i32, i32)], thickness: f32, tower_size: f32) -> WallPlan {
    let mut plan = WallPlan {
        tiles: Vec::new(),
        tangents: HashMap::new(),
    };
    if points.len() < 2 {
        return plan;
    }
    let line = evaluate(points, BAND_STEP);
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
            for dx in -half..=far {
                for dy in -half..=far {
                    tiles.insert((x + dx, y + dy));
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

pub mod walls;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solid_style_leaves_interiors_whole_and_shapes_tips() {
        // A straight row: end tiles are single-neighbour tips, the middle is interior.
        let tiles = generate(&[(0, 0), (4, 0)], EdgeStyle::Solid, 1.0);
        let at = |x: i32| tiles.iter().find(|(t, _)| *t == (x, 0)).map(|(_, c)| *c);
        assert_eq!(at(2), Some(TileChoice::Full));
        // West tip (mask 2, neighbour east): shape 1 rotation 0. East tip: shape 1 rotation 2.
        assert_eq!(
            at(0),
            Some(TileChoice::Shape {
                shape: 1,
                rotation: 0
            })
        );
        assert_eq!(
            at(4),
            Some(TileChoice::Shape {
                shape: 1,
                rotation: 2
            })
        );
    }

    #[test]
    fn wedge_style_points_tips_and_smooth_rounds_corners() {
        // An L: the corner tile has neighbours east + north (mask 3).
        let l = generate(&[(0, 2), (0, 0), (2, 0)], EdgeStyle::Wedge, 1.0);
        let corner = l.iter().find(|(t, _)| *t == (0, 0)).map(|(_, c)| *c);
        assert_eq!(
            corner,
            Some(TileChoice::Shape {
                shape: 1,
                rotation: 2
            })
        );
        let smooth = generate(&[(0, 2), (0, 0), (2, 0)], EdgeStyle::Smooth, 1.0);
        let corner = smooth.iter().find(|(t, _)| *t == (0, 0)).map(|(_, c)| *c);
        assert_eq!(
            corner,
            Some(TileChoice::Shape {
                shape: 11,
                rotation: 0
            })
        );
        // A lone tip tile gets the wedge point (mask 1: neighbour north).
        let tip = generate(&[(0, 0), (0, 3)], EdgeStyle::Wedge, 1.0);
        assert_eq!(
            tip.iter().find(|(t, _)| *t == (0, 0)).map(|(_, c)| *c),
            Some(TileChoice::Shape {
                shape: 6,
                rotation: 3
            })
        );
    }

    #[test]
    fn wall_band_of_thickness_1_follows_a_straight_line() {
        let line = evaluate(&[(0, 0), (5, 0)], 0.25);
        let band = rasterize_band(&line, 1.0);
        for x in 0..=5 {
            assert!(band.tiles.contains(&(x, 0)), "missing ({x}, 0)");
        }
        assert_eq!(band.tiles.len(), 6);
        for tile in &band.tiles {
            let (tx, ty) = band.tangents[tile];
            assert!(tx > 0.99 && ty.abs() < 0.01, "{tile:?}: ({tx:?}, {ty:?})");
        }
    }

    #[test]
    fn wall_band_thickness_counts_tiles_exactly() {
        let line = evaluate(&[(0, 2), (6, 2)], 0.25);
        // Odd thickness 3 stamps y in 1..=3; even 4 stamps 0..=3.
        let odd = rasterize_band(&line, 3.0);
        assert!(odd.tiles.contains(&(3, 1)) && odd.tiles.contains(&(3, 3)));
        assert!(!odd.tiles.contains(&(3, 0)) && !odd.tiles.contains(&(3, 4)));
        let even = rasterize_band(&line, 4.0);
        assert!(even.tiles.contains(&(3, 0)) && even.tiles.contains(&(3, 3)));
        assert!(!even.tiles.contains(&(3, -1)) && !even.tiles.contains(&(3, 4)));
    }

    #[test]
    fn wall_band_diagonal_runs_diagonally_with_diagonal_tangents() {
        let line = evaluate(&[(0, 0), (4, 4)], 0.25);
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
        let plan = generate_walls(&[(0, 0), (6, 0)], 1.0, 3.0);
        assert!(!plan.tiles.is_empty());
        assert!(
            plan.tiles
                .iter()
                .all(|(_, choice)| *choice == TileChoice::Full)
        );
        // Towers: 3x3 squares around both control points.
        for (x, y) in [(0, 0), (6, 0), (-1, -1), (7, 1), (0, 1), (6, -1)] {
            assert!(
                plan.tiles.iter().any(|(t, _)| *t == (x, y)),
                "missing tower tile ({x}, {y})"
            );
        }
        assert!(plan.tiles.iter().any(|(t, _)| *t == (3, 0)));
        assert!(plan.tangents.contains_key(&(3, 0)));
        let none = generate_walls(&[(1, 1)], 1.0, 3.0);
        assert!(none.tiles.is_empty() && none.tangents.is_empty());
    }

    #[test]
    fn retired_style_names_still_load() -> Result<(), serde_json::Error> {
        assert_eq!(
            serde_json::from_str::<EdgeStyle>("\"Whole\"")?,
            EdgeStyle::Solid
        );
        assert_eq!(
            serde_json::from_str::<EdgeStyle>("\"Chiselled\"")?,
            EdgeStyle::Wedge
        );
        assert_eq!(
            serde_json::from_str::<EdgeStyle>("\"Runs\"")?,
            EdgeStyle::Smooth
        );
        Ok(())
    }
}
