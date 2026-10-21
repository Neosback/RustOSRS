//! Corridor-free macro layout: rooms share walls directly, the way most real buildings do.
//!
//! The corridor layout in the parent module reserves a one-tile band between every pair of
//! partitions, which makes a hallway network out of what real maps build as one to three rooms
//! side by side. Here a rectangle is cut straight into rooms with no band between them, and a
//! footprint can be an L or a T made of two rectangular wings. Doors between the rooms are the
//! topology layer's job (`building_topology::compact`).

use super::{BspLayoutError, BspRoomPlacement, BuildingProgram, SplitMix64, rect_area};
use crate::{RectSite, RoomId, RoomRect};
use osrs_core::coords::MapTile;

/// The outline of the rooms together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootprintShape {
    Rect,
    /// A rectangle with one corner block missing.
    L,
    /// A bar with a stem on one side, leaving a gap beside the stem at both ends.
    T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactLayout {
    pub site: RectSite,
    pub shape: FootprintShape,
    rooms: Vec<BspRoomPlacement>,
}

impl CompactLayout {
    pub fn rooms(&self) -> &[BspRoomPlacement] {
        &self.rooms
    }

    pub fn room(&self, id: RoomId) -> Option<&BspRoomPlacement> {
        self.rooms.iter().find(|placement| placement.room == id)
    }
}

/// Pack `program` into `site` with no corridors, using the first shape that works: an L or T
/// when the seed asks for one and the rooms and site allow it, else a rectangle.
pub fn pack_compact_program(
    program: &BuildingProgram,
    site: RectSite,
    seed: u64,
) -> Result<CompactLayout, BspLayoutError> {
    if program.rooms.is_empty() {
        return Err(BspLayoutError::NoProgramRooms);
    }
    if site.width() < 4 || site.height() < 4 {
        return Err(BspLayoutError::SiteTooSmall {
            width: site.width(),
            height: site.height(),
        });
    }
    let min_span = program
        .rooms
        .iter()
        .map(|room| room.min_width.max(room.min_height))
        .max()
        .unwrap_or(2)
        .max(2);
    let interior = RoomRect {
        origin: MapTile::new(site.origin().x + 1, site.origin().y + 1),
        width: site.width() - 2,
        height: site.height() - 2,
    };
    if interior.width < min_span || interior.height < min_span {
        return Err(BspLayoutError::SiteTooSmall {
            width: site.width(),
            height: site.height(),
        });
    }

    let mut rng = SplitMix64::new(seed ^ 0xc0de_5eed_0000_0001);
    let wanted = choose_shape(&mut rng, program.rooms.len());
    for shape in [wanted, FootprintShape::Rect] {
        let mut attempt = rng;
        let Some(regions) = regions(interior, shape, min_span, &mut attempt) else {
            continue;
        };
        if let Some(leaves) = split_regions(&regions, program.rooms.len(), min_span, &mut attempt) {
            return Ok(assign(program, site, shape, leaves));
        }
        if shape == FootprintShape::Rect {
            break;
        }
    }
    Err(BspLayoutError::CannotSplitForRoomCount {
        room_count: program.rooms.len(),
    })
}

/// The program and layout for a building of `archetype` on `site`: the rooms a real building of
/// that size has (see [`sampled_room_count`]), at most `max_rooms` of them, the least important
/// dropped until the rest fit.
pub fn plan_compact_layout(
    archetype: crate::BuildingArchetype,
    site: RectSite,
    seed: u64,
    max_rooms: Option<usize>,
) -> Result<(BuildingProgram, CompactLayout), BspLayoutError> {
    plan_compact_layout_for(crate::building_program(archetype), site, seed, max_rooms)
}

/// [`plan_compact_layout`] for a given program, such as an upper floor's.
pub fn plan_compact_layout_for(
    program: BuildingProgram,
    site: RectSite,
    seed: u64,
    max_rooms: Option<usize>,
) -> Result<(BuildingProgram, CompactLayout), BspLayoutError> {
    let interior =
        u32::from(site.width().saturating_sub(2)) * u32::from(site.height().saturating_sub(2));
    let wanted = super::sampled_room_count(interior, program.archetype, seed);
    let mut program = program.limited_to(wanted);
    if let Some(count) = max_rooms {
        program = program.limited_to(count);
    }
    pack_fitted_compact_program(&program, site, seed)
}

/// [`pack_compact_program`], dropping the least important rooms until the rest fit the site.
pub fn pack_fitted_compact_program(
    program: &BuildingProgram,
    site: RectSite,
    seed: u64,
) -> Result<(BuildingProgram, CompactLayout), BspLayoutError> {
    let mut fitted = program.clone();
    loop {
        match pack_compact_program(&fitted, site, seed) {
            Ok(layout) => return Ok((fitted, layout)),
            Err(BspLayoutError::CannotSplitForRoomCount { .. }) if fitted.rooms().len() > 1 => {
                fitted = fitted.limited_to(fitted.rooms().len() - 1);
            }
            Err(error) => return Err(error),
        }
    }
}

/// Odds of an outline by room count, from the roofed buildings of the shipped corpus: the share
/// whose floor fills under 90% of its bounding box is 15% for one room (not offered here: a single
/// room is one rectangle), 27% for two, and 59% for three or more. Rolls are out of 100; an L is
/// the commoner outline, a T the rarer.
fn choose_shape(rng: &mut SplitMix64, rooms: usize) -> FootprintShape {
    let roll = rng.range_inclusive(0, 99);
    match rooms {
        0..=1 => FootprintShape::Rect,
        2 if roll < 27 => FootprintShape::L,
        2 => FootprintShape::Rect,
        3 if roll < 42 => FootprintShape::L,
        3 if roll < 59 => FootprintShape::T,
        _ if roll < 44 => FootprintShape::L,
        _ if roll < 59 => FootprintShape::T,
        _ => FootprintShape::Rect,
    }
}

/// How far a notch or stem reaches along a side of `length`: at least two fifths of it, so the
/// outline reads as an L or T and not as a rectangle with a nick out of it, and at least
/// `min_span` short of the far end so the bar keeps a room's depth.
fn notch(length: u16, min_span: u16, rng: &mut SplitMix64) -> u16 {
    let hi = length - min_span;
    let lo = (length * 2 / 5).max(min_span).min(hi);
    rng.range_inclusive(lo, hi)
}

fn offset(origin: MapTile, dx: u16, dy: u16) -> MapTile {
    MapTile::new(origin.x + i32::from(dx), origin.y + i32::from(dy))
}

/// The rectangles an outline is made of: one for a rectangle, a main bar and a wing otherwise.
/// `None` when the interior is too small to show the shape with every part at least `min_span`.
fn regions(
    interior: RoomRect,
    shape: FootprintShape,
    min_span: u16,
    rng: &mut SplitMix64,
) -> Option<Vec<RoomRect>> {
    let (w, h, o) = (interior.width, interior.height, interior.origin);
    match shape {
        FootprintShape::Rect => Some(vec![interior]),
        FootprintShape::L => {
            if w < min_span * 2 || h < min_span * 2 {
                return None;
            }
            let notch_w = notch(w, min_span, rng);
            let notch_h = notch(h, min_span, rng);
            let (wing_w, bar_h) = (w - notch_w, h - notch_h);
            let corner = rng.range_inclusive(0, 3);
            // Bars at the bottom keep the wing above them; bars at the top put it below.
            let bar_at_bottom = corner < 2;
            let wing_at_left = corner.is_multiple_of(2);
            let bar = RoomRect {
                origin: if bar_at_bottom {
                    o
                } else {
                    offset(o, 0, notch_h)
                },
                width: w,
                height: bar_h,
            };
            let wing = RoomRect {
                origin: offset(
                    o,
                    if wing_at_left { 0 } else { notch_w },
                    if bar_at_bottom { bar_h } else { 0 },
                ),
                width: wing_w,
                height: notch_h,
            };
            Some(vec![bar, wing])
        }
        FootprintShape::T => {
            if w < min_span + 4 || h < min_span * 2 {
                return None;
            }
            let stem_w = (w / 3).max(min_span).min(w - 4);
            let stem_w = rng.range_inclusive(stem_w, w - 4);
            let left = rng.range_inclusive(2, w - stem_w - 2);
            let stem_h = notch(h, min_span, rng);
            let bar_h = h - stem_h;
            let stem_on_top = rng.range_inclusive(0, 1) == 0;
            let bar = RoomRect {
                origin: if stem_on_top { o } else { offset(o, 0, stem_h) },
                width: w,
                height: bar_h,
            };
            let stem = RoomRect {
                origin: offset(o, left, if stem_on_top { bar_h } else { 0 }),
                width: stem_w,
                height: stem_h,
            };
            Some(vec![bar, stem])
        }
    }
}

/// Cut the regions into `count` rooms in all: each region gets at least one, the rest go where
/// the area is. Tries the nearest feasible split of the count between two regions.
fn split_regions(
    regions: &[RoomRect],
    count: usize,
    min_span: u16,
    rng: &mut SplitMix64,
) -> Option<Vec<RoomRect>> {
    if regions.len() == 1 {
        return split_region(regions[0], count, min_span, rng);
    }
    if count < regions.len() {
        return None;
    }
    let total: u32 = regions.iter().map(|r| rect_area(*r)).sum();
    let ideal = (u64::from(rect_area(regions[0])) * count as u64 + u64::from(total) / 2)
        / u64::from(total.max(1));
    let ideal = (ideal as usize).clamp(1, count - 1);
    let mut order: Vec<usize> = (1..count).collect();
    order.sort_by_key(|k| (k.abs_diff(ideal), *k));
    for first in order {
        let mut attempt = *rng;
        if let (Some(mut a), Some(b)) = (
            split_region(regions[0], first, min_span, &mut attempt),
            split_region(regions[1], count - first, min_span, &mut attempt),
        ) {
            a.extend(b);
            *rng = attempt;
            return Some(a);
        }
    }
    None
}

/// Straight cuts only: the largest leaf that can still be cut is halved along its longer side
/// (or its other side when only that one has room), at a random line leaving `min_span` each side.
fn split_region(
    region: RoomRect,
    count: usize,
    min_span: u16,
    rng: &mut SplitMix64,
) -> Option<Vec<RoomRect>> {
    let can_cut = |r: &RoomRect| r.width >= min_span * 2 || r.height >= min_span * 2;
    let mut leaves = vec![region];
    while leaves.len() < count {
        let index = leaves
            .iter()
            .enumerate()
            .filter(|(_, leaf)| can_cut(leaf))
            .max_by(|(_, a), (_, b)| {
                rect_area(**a)
                    .cmp(&rect_area(**b))
                    .then_with(|| b.origin.cmp(&a.origin))
            })
            .map(|(index, _)| index)?;
        let leaf = leaves.remove(index);
        let across =
            leaf.width >= leaf.height && leaf.width >= min_span * 2 || leaf.height < min_span * 2;
        if across {
            let (lo, hi) = cut_band(leaf.width, min_span);
            let first = rng.range_inclusive(lo, hi);
            leaves.push(RoomRect {
                width: first,
                ..leaf
            });
            leaves.push(RoomRect {
                origin: offset(leaf.origin, first, 0),
                width: leaf.width - first,
                ..leaf
            });
        } else {
            let (lo, hi) = cut_band(leaf.height, min_span);
            let first = rng.range_inclusive(lo, hi);
            leaves.push(RoomRect {
                height: first,
                ..leaf
            });
            leaves.push(RoomRect {
                origin: offset(leaf.origin, 0, first),
                height: leaf.height - first,
                ..leaf
            });
        }
    }
    Some(leaves)
}

/// Where a cut may fall along a side of `length`: at least `min_span` from each end, and not in
/// the outer quarter either, so a cut never makes a sliver beside a hall.
fn cut_band(length: u16, min_span: u16) -> (u16, u16) {
    let quarter = length / 4;
    let lo = min_span.max(quarter);
    let hi = (length - min_span).min(length - quarter);
    (lo.min(hi), hi)
}

/// The biggest rooms of the program get the biggest leaves, as in the corridor layout.
fn assign(
    program: &BuildingProgram,
    site: RectSite,
    shape: FootprintShape,
    mut leaves: Vec<RoomRect>,
) -> CompactLayout {
    leaves.sort_by(|a, b| {
        rect_area(*b)
            .cmp(&rect_area(*a))
            .then_with(|| a.origin.cmp(&b.origin))
    });
    let mut rooms = program.rooms.clone();
    rooms.sort_by(|a, b| b.area_weight.cmp(&a.area_weight).then(a.id.cmp(&b.id)));
    let mut placements: Vec<BspRoomPlacement> = rooms
        .into_iter()
        .zip(leaves)
        .map(|(room, rect)| BspRoomPlacement {
            room: room.id,
            rect,
        })
        .collect();
    placements.sort_by_key(|placement| placement.room);
    CompactLayout {
        site,
        shape,
        rooms: placements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildingArchetype, building_program};
    use std::collections::BTreeSet;

    fn site(width: u16, height: u16) -> RectSite {
        RectSite::new(MapTile::new(10, 10), width, height)
            .unwrap_or_else(|| unreachable!("test site is valid"))
    }

    fn tiles(rect: RoomRect) -> BTreeSet<MapTile> {
        (0..rect.height)
            .flat_map(|dy| (0..rect.width).map(move |dx| offset(rect.origin, dx, dy)))
            .collect()
    }

    fn packed(rooms: usize, w: u16, h: u16, seed: u64) -> CompactLayout {
        let program = building_program(BuildingArchetype::Tavern).limited_to(rooms);
        pack_compact_program(&program, site(w, h), seed)
            .unwrap_or_else(|e| panic!("{rooms} rooms in {w}x{h} #{seed}: {e:?}"))
    }

    #[test]
    fn rooms_tile_the_footprint_without_gaps_overlaps_or_corridors() {
        for rooms in 1..=5 {
            for seed in 0..60 {
                let layout = packed(rooms, 16, 14, seed);
                assert_eq!(layout.rooms().len(), rooms);
                let mut seen = BTreeSet::new();
                for placement in layout.rooms() {
                    assert!(placement.rect.width >= 3 && placement.rect.height >= 3);
                    for tile in tiles(placement.rect) {
                        assert!(seen.insert(tile), "overlap at {tile:?} (seed {seed})");
                    }
                }
                // Inside the interior (the site minus its outer ring).
                let interior = site(16, 14);
                assert!(seen.iter().all(|t| {
                    t.x > interior.origin().x
                        && t.x < interior.origin().x + 15
                        && t.y > interior.origin().y
                        && t.y < interior.origin().y + 13
                }));
                // A rectangle is filled; an L or T is the interior minus whole blocks.
                if layout.shape == FootprintShape::Rect {
                    assert_eq!(seen.len(), 14 * 12);
                } else {
                    assert!(seen.len() < 14 * 12);
                }
            }
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_layout() {
        assert_eq!(packed(4, 16, 14, 9), packed(4, 16, 14, 9));
        assert_ne!(packed(4, 16, 14, 9), packed(4, 16, 14, 10));
    }

    #[test]
    fn an_l_and_a_t_are_laid_out_for_several_rooms() {
        let shapes: BTreeSet<_> = (0..120)
            .map(|seed| format!("{:?}", packed(4, 18, 14, seed).shape))
            .collect();
        assert!(shapes.contains("L") && shapes.contains("T") && shapes.contains("Rect"));
    }

    #[test]
    fn a_single_room_is_always_a_rectangle() {
        assert!((0..60).all(|seed| packed(1, 12, 10, seed).shape == FootprintShape::Rect));
    }

    #[test]
    fn too_many_rooms_for_the_site_is_an_error_and_fitting_drops_the_extras() {
        let program = building_program(BuildingArchetype::Tavern);
        assert!(matches!(
            pack_compact_program(&program, site(8, 8), 0),
            Err(BspLayoutError::CannotSplitForRoomCount { .. })
        ));
        let (fitted, layout) = pack_fitted_compact_program(&program, site(8, 8), 0)
            .unwrap_or_else(|e| panic!("fits fewer rooms: {e:?}"));
        assert!(fitted.rooms().len() < program.rooms().len());
        assert_eq!(layout.rooms().len(), fitted.rooms().len());
    }

    #[test]
    fn the_planned_layout_keeps_the_rooms_the_real_distribution_asks_for() {
        let area_of = |w: u16, h: u16| u32::from(w - 2) * u32::from(h - 2);
        for seed in 0..40 {
            let (program, layout) =
                plan_compact_layout(BuildingArchetype::House, site(10, 8), seed, None)
                    .unwrap_or_else(|e| panic!("{e:?}"));
            let wanted = crate::sampled_room_count(area_of(10, 8), BuildingArchetype::House, seed);
            assert!(program.rooms().len() <= wanted);
            assert_eq!(layout.rooms().len(), program.rooms().len());
        }
        let (capped, _) = plan_compact_layout(BuildingArchetype::Castle, site(30, 24), 3, Some(2))
            .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(capped.rooms().len(), 2);
    }
}
