//! Roof volumes: which "under a roof" tiles belong together, so one building's roof can be hidden
//! without touching its neighbours.
//!
//! The pinned client marks every ground tile that has a roof above it with the `UNDER_ROOF` tile
//! setting (`4`). RuneLite's roof removal groups those tiles into connected components per plane
//! (`Scene.buildRoofs`, eight-neighbour flood fill) and buckets the geometry above each component
//! under the component's id, so hiding a roof is a range skip. This module is that grouping, with
//! two differences for an editor:
//!
//! * ids are derived from world coordinates (the component's south-west-most tile), so every scene
//!   window that sees the same building names it the same, and the editor can ask for the id of the
//!   building under a tile without holding a window;
//! * a tile without the flag that touches a flagged tile joins that component, because roofs
//!   overhang the walls by one tile and the overhang must go with the roof.

/// Tile setting bit: this ground tile has a roof above it.
pub const UNDER_ROOF: u8 = 4;

/// The roof id for "no roof" geometry.
pub const NO_ROOF: u32 = 0;

/// The id of the component whose south-west-most flagged tile is `(x, y)` on `plane`.
///
/// Always non-zero. World coordinates are at most 14 bits (`0..16384`).
pub const fn roof_id(plane: u8, x: i32, y: i32) -> u32 {
    0x8000_0000 | ((plane as u32 & 3) << 28) | ((x as u32 & 0x3fff) << 14) | (y as u32 & 0x3fff)
}

/// Roof ids for a rectangle of tiles, all four planes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoofGrid {
    origin: (i32, i32),
    width: usize,
    height: usize,
    ids: Vec<u32>,
}

impl RoofGrid {
    /// Label the rectangle whose south-west tile is `origin` (world tiles). `flags(plane, x, y)`
    /// returns the tile setting byte at rectangle-local `(x, y)`.
    pub fn build(
        origin: (i32, i32),
        width: usize,
        height: usize,
        flags: impl Fn(usize, usize, usize) -> u8,
    ) -> Self {
        let mut ids = vec![NO_ROOF; 4 * width * height];
        let index = |plane: usize, x: usize, y: usize| (plane * width + x) * height + y;
        for plane in 0..4_usize {
            let flagged = |x: usize, y: usize| flags(plane, x, y) & UNDER_ROOF != 0;
            let mut seen = vec![false; width * height];
            // Scanning x-major then y puts the component's south-west-most tile first, which is
            // what names it.
            for x in 0..width {
                for y in 0..height {
                    if seen[x * height + y] || !flagged(x, y) {
                        continue;
                    }
                    let id = roof_id(plane as u8, origin.0 + x as i32, origin.1 + y as i32);
                    let mut stack = vec![(x, y)];
                    seen[x * height + y] = true;
                    while let Some((cx, cy)) = stack.pop() {
                        ids[index(plane, cx, cy)] = id;
                        for (nx, ny) in neighbours(cx, cy, width, height) {
                            if flagged(nx, ny) && !seen[nx * height + ny] {
                                seen[nx * height + ny] = true;
                                stack.push((nx, ny));
                            }
                        }
                    }
                }
            }
            // The overhang: an unflagged tile next to a roof takes the roof's id (the smallest
            // when it touches several, so the answer does not depend on scan order).
            let core: Vec<u32> =
                ids[index(plane, 0, 0)..index(plane, 0, 0) + width * height].to_vec();
            for x in 0..width {
                for y in 0..height {
                    if core[x * height + y] != NO_ROOF {
                        continue;
                    }
                    let joined = neighbours(x, y, width, height)
                        .map(|(nx, ny)| core[nx * height + ny])
                        .filter(|id| *id != NO_ROOF)
                        .min();
                    if let Some(id) = joined {
                        ids[index(plane, x, y)] = id;
                    }
                }
            }
        }
        Self {
            origin,
            width,
            height,
            ids,
        }
    }

    /// The roof id over rectangle-local `(x, y)` on `plane`, [`NO_ROOF`] outside or when none.
    pub fn id(&self, plane: usize, x: usize, y: usize) -> u32 {
        if plane > 3 || x >= self.width || y >= self.height {
            return NO_ROOF;
        }
        self.ids[(plane * self.width + x) * self.height + y]
    }

    /// Whether world tile `(x, y)` lies inside the rectangle, at least `margin` tiles from its
    /// edges (so a roof there is labelled whole).
    pub fn covers_world(&self, x: i32, y: i32, margin: i32) -> bool {
        let (lx, ly) = (x - self.origin.0, y - self.origin.1);
        lx >= margin
            && ly >= margin
            && lx < self.width as i32 - margin
            && ly < self.height as i32 - margin
    }

    /// The roof id over world tile `(x, y)` on `plane`.
    pub fn id_at_world(&self, plane: usize, x: i32, y: i32) -> u32 {
        let (lx, ly) = (x - self.origin.0, y - self.origin.1);
        if lx < 0 || ly < 0 {
            return NO_ROOF;
        }
        self.id(plane, lx as usize, ly as usize)
    }
}

/// The eight tiles around `(x, y)` that exist.
fn neighbours(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> impl Iterator<Item = (usize, usize)> {
    (-1_i32..=1)
        .flat_map(|dx| (-1_i32..=1).map(move |dy| (dx, dy)))
        .filter(|(dx, dy)| (*dx, *dy) != (0, 0))
        .filter_map(move |(dx, dy)| {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            (nx >= 0 && ny >= 0 && (nx as usize) < width && (ny as usize) < height)
                .then_some((nx as usize, ny as usize))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A map from text: `#` is a roof tile, `.` nothing; row 0 of the text is the northernmost.
    fn grid(rows: &[&str]) -> RoofGrid {
        let height = rows.len();
        let width = rows[0].len();
        RoofGrid::build((3200, 3200), width, height, |plane, x, y| {
            if plane != 0 {
                return 0;
            }
            let row = rows[height - 1 - y].as_bytes();
            if row[x] == b'#' { UNDER_ROOF } else { 0 }
        })
    }

    #[test]
    fn separate_buildings_get_separate_ids_and_the_overhang_joins_its_roof() {
        let g = grid(&[
            "........", //
            ".###.##.", //
            ".###.##.", //
            "........",
        ]);
        let left = g.id(0, 2, 1);
        let right = g.id(0, 5, 1);
        assert_ne!(left, NO_ROOF);
        assert_ne!(right, NO_ROOF);
        assert_ne!(left, right);
        // The ring around each roof joins it; the tile between the two touches both and takes
        // the smaller id.
        assert_eq!(g.id(0, 0, 0), left);
        assert_eq!(g.id(0, 4, 1), left.min(right));
        // Two tiles away from any roof there is nothing.
        let far = grid(&["#.....#", ".......", ".......", "......."]);
        assert_eq!(far.id(0, 3, 1), NO_ROOF);
    }

    #[test]
    fn the_id_is_the_south_west_most_tile_in_world_coordinates() {
        let g = grid(&[
            "....", //
            ".##.", //
            ".##.", //
            "....",
        ]);
        assert_eq!(g.id(0, 2, 2), roof_id(0, 3201, 3201));
        assert_eq!(g.id_at_world(0, 3202, 3202), roof_id(0, 3201, 3201));
        assert_eq!(g.id_at_world(0, 3100, 3202), NO_ROOF);
        assert_eq!(g.id(1, 1, 1), NO_ROOF);
    }

    #[test]
    fn diagonal_neighbours_are_one_roof() {
        let g = grid(&[
            "#...", //
            ".#..", //
            "....", //
            "....",
        ]);
        assert_eq!(g.id(0, 0, 3), g.id(0, 1, 2));
    }
}
