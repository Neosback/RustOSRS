//! The editor's ground as the planner sees it: what stands on each tile, how steep it is and which
//! steps a wall stops, read from the regions the editor has loaded; and the tiles that already
//! carry an overlay.

use super::model::{CostGrid, Occupancy, TileBounds};
use crate::pick::Infos;
use osrs_world::RegionInfo;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const SIZE: usize = 64;

/// One region on one plane, flattened.
struct RegionGrid {
    occupancy: Vec<Occupancy>,
    slope: Vec<f32>,
    /// Sides a wall closes (bit per orientation: 0 west, 1 north, 2 east, 3 south).
    walls: Vec<u8>,
    /// Diagonal walls (bit per orientation).
    diagonals: Vec<u8>,
}

fn index(local_x: usize, local_y: usize) -> usize {
    local_x * SIZE + local_y
}

impl RegionGrid {
    fn build(info: &RegionInfo, plane: u8) -> Self {
        let mut grid = Self {
            occupancy: vec![Occupancy::Clear; SIZE * SIZE],
            slope: vec![0.0; SIZE * SIZE],
            walls: vec![0; SIZE * SIZE],
            diagonals: vec![0; SIZE * SIZE],
        };
        let (base_x, base_y) = (info.region.x * 64, info.region.y * 64);
        for lx in 0..SIZE {
            for ly in 0..SIZE {
                let corner =
                    |dx: usize, dy: usize| info.corner_height(usize::from(plane), lx + dx, ly + dy);
                let heights = [corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1)];
                let (low, high) = heights
                    .iter()
                    .fold((i32::MAX, i32::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
                grid.slope[index(lx, ly)] = (high - low) as f32 / 128.0;
                let tile = (base_x + lx as i32, base_y + ly as i32);
                if info
                    .tile(usize::from(plane), tile)
                    .is_some_and(|t| t.settings & 1 != 0)
                {
                    grid.occupancy[index(lx, ly)] = Occupancy::Solid;
                }
            }
        }
        for loc in &info.locs {
            if loc.plane != plane {
                continue;
            }
            let solid = loc.definition.placement.interact_type != 0;
            let (lx, ly) = (loc.tile.0 - base_x, loc.tile.1 - base_y);
            if !(0..SIZE as i32).contains(&lx) || !(0..SIZE as i32).contains(&ly) {
                continue;
            }
            let at = index(lx as usize, ly as usize);
            let orientation = loc.orientation & 3;
            match loc.loc_type {
                0 if solid => grid.walls[at] |= 1 << orientation,
                2 if solid => grid.walls[at] |= 1 << orientation | 1 << ((orientation + 1) & 3),
                1 | 3 if solid => grid.diagonals[at] |= 1 << orientation,
                // Walls and wall decorations do not occupy the tile.
                0..=8 => {}
                22 => {
                    let tile = if loc.definition.placement.interact_type == 1 {
                        Occupancy::Solid
                    } else {
                        Occupancy::Decoration
                    };
                    mark(&mut grid.occupancy[at], tile);
                }
                _ => {
                    let footprint = loc.plan.rotated_definition_footprint;
                    let mark_as = if solid {
                        Occupancy::Solid
                    } else {
                        Occupancy::Decoration
                    };
                    for dx in 0..i32::from(footprint.width) {
                        for dy in 0..i32::from(footprint.depth) {
                            let (x, y) = (lx + dx, ly + dy);
                            if (0..SIZE as i32).contains(&x) && (0..SIZE as i32).contains(&y) {
                                mark(&mut grid.occupancy[index(x as usize, y as usize)], mark_as);
                            }
                        }
                    }
                }
            }
        }
        grid
    }
}

/// Solid outranks decoration outranks clear.
fn mark(slot: &mut Occupancy, with: Occupancy) {
    let rank = |o: Occupancy| match o {
        Occupancy::Clear => 0,
        Occupancy::Decoration => 1,
        Occupancy::Solid => 2,
    };
    if rank(with) > rank(*slot) {
        *slot = with;
    }
}

/// The ground of the loaded regions.
pub struct SceneGrid {
    infos: Infos,
    cache: Mutex<HashMap<(i32, i32, u8), Arc<RegionGrid>>>,
}

impl SceneGrid {
    pub fn new(infos: Infos) -> Self {
        Self {
            infos,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// The tiles of the loaded regions (a box round them all).
    pub fn bounds(&self) -> TileBounds {
        let mut regions = self.infos.keys();
        let Some(first) = regions.next() else {
            return TileBounds::empty();
        };
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (first.0, first.1, first.0, first.1);
        for (x, y) in regions {
            min_x = min_x.min(*x);
            min_y = min_y.min(*y);
            max_x = max_x.max(*x);
            max_y = max_y.max(*y);
        }
        TileBounds::new(min_x * 64, min_y * 64, (max_x + 1) * 64, (max_y + 1) * 64)
    }

    fn region(&self, x: i32, y: i32, plane: u8) -> Option<(Arc<RegionGrid>, usize, usize)> {
        let key = (x.div_euclid(64), y.div_euclid(64));
        let info = self.infos.get(&key)?;
        let mut cache = self.cache.lock().ok()?;
        let grid = cache
            .entry((key.0, key.1, plane))
            .or_insert_with(|| Arc::new(RegionGrid::build(info, plane)))
            .clone();
        Some((grid, x.rem_euclid(64) as usize, y.rem_euclid(64) as usize))
    }

    fn exists(&self, x: i32, y: i32, plane: u8) -> bool {
        plane < 4
            && self
                .infos
                .contains_key(&(x.div_euclid(64), y.div_euclid(64)))
    }

    fn wall_blocks(&self, x: i32, y: i32, dx: i32, dy: i32, plane: u8) -> bool {
        let Some((grid, lx, ly)) = self.region(x, y, plane) else {
            return false;
        };
        let side = if dx < 0 {
            0
        } else if dy > 0 {
            1
        } else if dx > 0 {
            2
        } else {
            3
        };
        grid.walls[index(lx, ly)] & (1 << side) != 0
    }

    fn diagonal_blocks(&self, x: i32, y: i32, dx: i32, dy: i32, plane: u8) -> bool {
        let Some((grid, lx, ly)) = self.region(x, y, plane) else {
            return false;
        };
        let corner = if dx < 0 {
            if dy > 0 { 0 } else { 3 }
        } else if dy > 0 {
            1
        } else {
            2
        };
        grid.diagonals[index(lx, ly)] & (1 << corner) != 0
    }
}

impl CostGrid for SceneGrid {
    fn occupancy(&self, x: i32, y: i32, plane: u8) -> Occupancy {
        match self.region(x, y, plane) {
            Some((grid, lx, ly)) => grid.occupancy[index(lx, ly)],
            None => Occupancy::Solid,
        }
    }

    fn traversal_cost(&self, x: i32, y: i32, plane: u8) -> f64 {
        if self.exists(x, y, plane) {
            1.0
        } else {
            f64::INFINITY
        }
    }

    fn slope(&self, x: i32, y: i32, plane: u8) -> f64 {
        match self.region(x, y, plane) {
            Some((grid, lx, ly)) => f64::from(grid.slope[index(lx, ly)]),
            None => f64::INFINITY,
        }
    }

    fn edge_blocked(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32, plane: u8) -> bool {
        if !self.exists(from_x, from_y, plane) || !self.exists(to_x, to_y, plane) {
            return true;
        }
        let (dx, dy) = ((to_x - from_x).signum(), (to_y - from_y).signum());
        let (ax, ay) = ((to_x - from_x).abs(), (to_y - from_y).abs());
        if ax > 1 || ay > 1 {
            true
        } else if ax == 1 && ay == 1 {
            self.diagonal_blocks(from_x, from_y, dx, dy, plane)
                || self.diagonal_blocks(to_x, to_y, -dx, -dy, plane)
                || self.edge_blocked(from_x, from_y, from_x + dx, from_y, plane)
                || self.edge_blocked(from_x, from_y, from_x, from_y + dy, plane)
        } else if ax + ay != 1 {
            false
        } else {
            self.wall_blocks(from_x, from_y, dx, dy, plane)
                || self.wall_blocks(to_x, to_y, -dx, -dy, plane)
        }
    }
}
