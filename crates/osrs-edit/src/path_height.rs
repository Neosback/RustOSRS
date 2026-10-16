//! "Smooth terrain height" of the path tool: even out the ground under a laid path by blending
//! every height point of the path, and a margin around it, with its neighbours. Ported from the
//! EikenB00m map editor (`SceneGraph.smoothPathHeights`).
//!
//! The planner is pure: it reads heights through a closure and returns the new height of each
//! vertex it changes. The patch builder preserves their single-unit precision, carries
//! the planes above along (they keep their offset from the plane below) and emits one reversible
//! `MapPatch`.
//!
//! Heights follow the map convention: client units, up is negative, and the lowest plane never
//! goes below zero.

use crate::path::PathCommitError;
use crate::{HeightVertexCoord, MapOp, MapPatch, MapQuery};
use osrs_core::coords::{MapTile, SourcePlane};
use std::collections::BTreeMap;

/// Extra tiles around the path's half-width that every vertex of the smoothing reach includes.
const REACH_MARGIN: f64 = 1.5;

/// The tool's terrain settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathHeightSpec {
    /// Width of the path in tiles (as drawn).
    pub width: f64,
    /// How many times every vertex is blended with its neighbours.
    pub passes: u32,
    /// Tiles around the path, beyond its edge, that are blended too, so the ground next to it
    /// slopes gradually instead of ending in a wall.
    pub blend: f64,
}

/// Distance from a point to a polyline (to the point itself for a one-point line).
pub fn distance_to_line(line: &[(f64, f64)], x: f64, y: f64) -> f64 {
    match line {
        [] => f64::INFINITY,
        [only] => (x - only.0).hypot(y - only.1),
        _ => line
            .windows(2)
            .map(|pair| distance_to_segment(pair[0], pair[1], x, y))
            .fold(f64::INFINITY, f64::min),
    }
}

fn distance_to_segment(a: (f64, f64), b: (f64, f64), x: f64, y: f64) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared == 0.0 {
        0.0
    } else {
        (((x - a.0) * dx + (y - a.1) * dy) / length_squared).clamp(0.0, 1.0)
    };
    (x - (a.0 + t * dx)).hypot(y - (a.1 + t * dy))
}

/// The vertices (tile corners) within the smoothing reach of the line, in `(x, y)` order.
pub fn smoothing_vertices(line: &[(f64, f64)], spec: PathHeightSpec) -> Vec<(i32, i32)> {
    if line.is_empty() {
        return Vec::new();
    }
    let reach = spec.width / 2.0 + REACH_MARGIN + spec.blend;
    let (mut min, mut max) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    for p in line {
        min = (min.0.min(p.0), min.1.min(p.1));
        max = (max.0.max(p.0), max.1.max(p.1));
    }
    let (x0, x1) = (
        (min.0 - reach).floor() as i32,
        (max.0 + reach).floor() as i32 + 1,
    );
    let (y0, y1) = (
        (min.1 - reach).floor() as i32,
        (max.1 + reach).floor() as i32 + 1,
    );
    let mut vertices = Vec::new();
    for x in x0..=x1 {
        for y in y0..=y1 {
            if distance_to_line(line, f64::from(x), f64::from(y)) <= reach {
                vertices.push((x, y));
            }
        }
    }
    vertices
}

/// The new height of every vertex that smoothing changes, sorted by vertex. `height` gives a
/// vertex's current height on the plane being edited (`None` where it is not loaded: such
/// vertices are neither changed nor counted as neighbours).
///
/// Each pass sets every vertex in reach to the mean of its own height and the mean of its (up to
/// nine, itself included) neighbourhood, all read from the previous pass, with integer division as
/// the original does. Vertices outside the reach keep their heights and are read as neighbours.
pub fn plan_path_height_smoothing(
    line: &[(f64, f64)],
    spec: PathHeightSpec,
    height: &dyn Fn((i32, i32)) -> Option<i32>,
) -> Vec<((i32, i32), i32)> {
    if spec.passes == 0 {
        return Vec::new();
    }
    let vertices: Vec<(i32, i32)> = smoothing_vertices(line, spec)
        .into_iter()
        .filter(|&v| height(v).is_some())
        .collect();
    if vertices.is_empty() {
        return Vec::new();
    }
    // Heights of the vertices and their neighbours, read once.
    let mut current = BTreeMap::new();
    for &(x, y) in &vertices {
        for dx in -1..=1 {
            for dy in -1..=1 {
                let v = (x + dx, y + dy);
                current.entry(v).or_insert_with(|| height(v));
            }
        }
    }
    let original: BTreeMap<_, _> = vertices
        .iter()
        .filter_map(|v| current.get(v).copied().flatten().map(|h| (*v, h)))
        .collect();
    for _ in 0..spec.passes {
        let previous = current.clone();
        for v in &vertices {
            let Some(Some(own)) = previous.get(v) else {
                continue;
            };
            let (mut total, mut count) = (0_i32, 0_i32);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(Some(h)) = previous.get(&(v.0 + dx, v.1 + dy)) {
                        total += h;
                        count += 1;
                    }
                }
            }
            current.insert(*v, Some((own + total / count) / 2));
        }
    }
    vertices
        .into_iter()
        .filter_map(|v| {
            let new = current.get(&v).copied().flatten()?.min(0);
            (new != *original.get(&v)?).then_some((v, new))
        })
        .collect()
}

/// One reversible patch for smoothed heights on `plane` (as [`plan_path_height_smoothing`]
/// returns them). Heights retain single-unit precision. The planes above move by
/// the same amount, so they keep their offset from the plane below, and never end up higher than
/// the plane under them.
pub fn build_path_height_patch(
    query: &impl MapQuery,
    plane: SourcePlane,
    smoothed: &[((i32, i32), i32)],
) -> Result<MapPatch, PathCommitError> {
    let mut ops = Vec::new();
    for &((x, y), desired) in smoothed {
        let tile = MapTile::new(x, y);
        let missing = || PathCommitError::MissingHeight { tile };
        let vertex = HeightVertexCoord::new(tile, plane);
        let before = query.height_at(vertex).ok_or_else(missing)?;
        let after = desired.min(0);
        let shift = after - before;
        // The source carries upper planes, then enforces ordering on every plane, even below the
        // edited one. Query values stay authoritative and the complete result is one patch.
        let mut below = None;
        for z in 0..4 {
            let source = SourcePlane::new(z).ok_or_else(missing)?;
            let coord = HeightVertexCoord::new(tile, source);
            let Some(current) = query.height_at(coord) else {
                continue;
            };
            let mut moved = if z == plane.index().get() {
                after
            } else if z > plane.index().get() {
                current + shift
            } else {
                current
            };
            if let Some(lower) = below {
                moved = moved.min(lower);
            }
            if moved != current {
                ops.push(MapOp::SetHeight {
                    coord,
                    before: current,
                    after: moved,
                });
            }
            below = Some(moved);
        }
    }
    Ok(MapPatch::new(ops))
}

#[cfg(test)]
mod tests;
