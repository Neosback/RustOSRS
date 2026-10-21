//! Where an upper floor stands on the floor below it.
//!
//! An upper floor is laid out on its own, in a rectangle that lies wholly on the floor below, so
//! every upper tile is supported. Over a rectangular floor it is the same rectangle most of the
//! time, and sometimes steps back from one side by three or four tiles (a roof terrace below);
//! over an L or T it is the largest rectangle that fits in the outline.
use crate::RectSite;
use osrs_core::coords::MapTile;
use std::collections::BTreeSet;

/// Smallest side of an upper floor's interior, in tiles (a room's minimum).
const MIN_SIDE: i32 = 3;
/// Out of 100: how often a rectangular floor has a smaller one above it. A design choice: the
/// corpus has no per-floor outlines, only that most multi-storey buildings read as one block.
const SETBACK_PERCENT: u64 = 25;

fn mix(seed: u64, salt: u64) -> u64 {
    let mut value = (seed ^ salt.rotate_left(23)).wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// An axis-aligned block of tiles: inclusive corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Block {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl Block {
    fn width(self) -> i32 {
        self.x1 - self.x0 + 1
    }

    fn height(self) -> i32 {
        self.y1 - self.y0 + 1
    }

    fn area(self) -> i32 {
        self.width() * self.height()
    }
}

fn bounds(tiles: &BTreeSet<MapTile>) -> Option<Block> {
    let first = tiles.iter().next()?;
    let mut block = Block {
        x0: first.x,
        y0: first.y,
        x1: first.x,
        y1: first.y,
    };
    for tile in tiles {
        block.x0 = block.x0.min(tile.x);
        block.y0 = block.y0.min(tile.y);
        block.x1 = block.x1.max(tile.x);
        block.y1 = block.y1.max(tile.y);
    }
    Some(block)
}

/// The largest rectangle made only of tiles in the set (ties go to the lowest corner).
fn largest_block(tiles: &BTreeSet<MapTile>) -> Option<Block> {
    let outer = bounds(tiles)?;
    let mut best: Option<Block> = None;
    for x0 in outer.x0..=outer.x1 {
        for x1 in x0..=outer.x1 {
            // Runs of rows that are full from x0 to x1.
            let mut start: Option<i32> = None;
            for y in outer.y0..=outer.y1 + 1 {
                let full = y <= outer.y1 && (x0..=x1).all(|x| tiles.contains(&MapTile::new(x, y)));
                match (full, start) {
                    (true, None) => start = Some(y),
                    (false, Some(y0)) => {
                        let block = Block {
                            x0,
                            y0,
                            x1,
                            y1: y - 1,
                        };
                        if best.is_none_or(|b| block.area() > b.area()) {
                            best = Some(block);
                        }
                        start = None;
                    }
                    _ => {}
                }
            }
        }
    }
    best
}

/// The site (with its one-tile outer ring) of the floor above a floor that covers `lower`, or
/// `None` when no rectangle of at least three tiles a side fits. `seed` and `floor` make the
/// choice repeatable.
pub(super) fn upper_site(lower: &BTreeSet<MapTile>, seed: u64, floor: u8) -> Option<RectSite> {
    let outer = bounds(lower)?;
    let is_rectangle = i32::try_from(lower.len()).ok()? == outer.area();
    let mut block = if is_rectangle {
        outer
    } else {
        largest_block(lower)?
    };
    if is_rectangle && mix(seed, u64::from(floor)) % 100 < SETBACK_PERCENT {
        let depth = 3 + i32::from(mix(seed, u64::from(floor) + 100).is_multiple_of(2));
        // Step back from the side the roll picks, if what is left stays a usable room.
        match mix(seed, u64::from(floor) + 200) % 4 {
            0 if block.height() - depth >= 5 => block.y1 -= depth,
            1 if block.height() - depth >= 5 => block.y0 += depth,
            2 if block.width() - depth >= 5 => block.x1 -= depth,
            3 if block.width() - depth >= 5 => block.x0 += depth,
            _ => {}
        }
    }
    if block.width() < MIN_SIDE || block.height() < MIN_SIDE {
        return None;
    }
    RectSite::new(
        MapTile::new(block.x0 - 1, block.y0 - 1),
        u16::try_from(block.width() + 2).ok()?,
        u16::try_from(block.height() + 2).ok()?,
    )
}

/// The tiles a roof over a floor must cover: the floor's own tiles that no floor above stands on,
/// one tile of overhang all round, minus the tiles of the floor above (a roof does not cut into the
/// wall of the storey that rises beside it).
pub(super) fn roof_footprint(
    floor: &BTreeSet<MapTile>,
    above: Option<&BTreeSet<MapTile>>,
) -> Vec<MapTile> {
    let exposed = floor
        .iter()
        .filter(|t| above.is_none_or(|a| !a.contains(t)));
    let mut out = BTreeSet::new();
    for tile in exposed {
        for dx in -1..=1 {
            for dy in -1..=1 {
                out.insert(MapTile::new(tile.x + dx, tile.y + dy));
            }
        }
    }
    if let Some(above) = above {
        out.retain(|t| !above.contains(t));
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> BTreeSet<MapTile> {
        (0..w)
            .flat_map(|dx| (0..h).map(move |dy| MapTile::new(x + dx, y + dy)))
            .collect()
    }

    fn tiles_of(site: RectSite) -> BTreeSet<MapTile> {
        rect(
            site.origin().x + 1,
            site.origin().y + 1,
            i32::from(site.width()) - 2,
            i32::from(site.height()) - 2,
        )
    }

    #[test]
    fn an_upper_floor_always_stands_on_the_floor_below() {
        let mut l_shape = rect(10, 10, 12, 5);
        l_shape.extend(rect(10, 15, 5, 6));
        for lower in [rect(10, 10, 12, 9), rect(4, 4, 5, 5), l_shape] {
            for seed in 0..200 {
                let Some(site) = upper_site(&lower, seed, 1) else {
                    continue;
                };
                assert!(tiles_of(site).is_subset(&lower), "seed {seed}");
            }
        }
    }

    #[test]
    fn a_rectangle_is_sometimes_the_same_and_sometimes_steps_back() {
        let lower = rect(10, 10, 12, 9);
        let mut same = 0;
        let mut smaller = 0;
        for seed in 0..400 {
            let upper = tiles_of(
                upper_site(&lower, seed, 1).unwrap_or_else(|| unreachable!("a room fits")),
            );
            if upper == lower {
                same += 1;
            } else {
                smaller += 1;
                // A setback leaves a strip at least three tiles deep.
                assert!(lower.len() - upper.len() >= 27);
            }
        }
        assert!((60..=140).contains(&smaller), "{smaller} setbacks of 400");
        assert_eq!(same + smaller, 400);
    }

    #[test]
    fn over_an_l_the_upper_floor_is_its_largest_rectangle() {
        let mut l_shape = rect(10, 10, 12, 4);
        l_shape.extend(rect(10, 14, 5, 6));
        let site = upper_site(&l_shape, 3, 1).unwrap_or_else(|| unreachable!("fits"));
        let upper = tiles_of(site);
        // The 12x4 bar is 48 tiles, the 5x10 side is 50: the side wins.
        assert_eq!(upper.len(), 50);
        assert!(upper.is_subset(&l_shape));
    }

    #[test]
    fn a_floor_too_small_for_a_room_has_nothing_above() {
        assert!(upper_site(&rect(10, 10, 2, 5), 0, 1).is_none());
    }

    #[test]
    fn the_same_inputs_choose_the_same_site() {
        let lower = rect(10, 10, 12, 9);
        assert_eq!(upper_site(&lower, 9, 1), upper_site(&lower, 9, 1));
    }

    #[test]
    fn a_roof_over_a_setback_stays_off_the_storey_that_rises_beside_it() {
        let lower = rect(10, 10, 10, 9);
        let upper = rect(10, 10, 10, 5);
        let roof = roof_footprint(&lower, Some(&upper));
        assert!(!roof.is_empty());
        assert!(roof.iter().all(|t| !upper.contains(t)));
        // The exposed strip (y 15..=18) gets an overhang tile beyond it.
        assert!(roof.contains(&MapTile::new(9, 19)) && roof.contains(&MapTile::new(20, 19)));
        // A top floor's roof is the whole floor plus the ring.
        assert_eq!(roof_footprint(&upper, None).len(), 12 * 7);
    }
}
