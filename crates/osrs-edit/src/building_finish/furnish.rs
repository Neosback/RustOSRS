//! Placing furniture in rooms: against walls facing the way real furniture does, or in the
//! middle, never in a doorway, never cutting a room in two.

use super::{KitFinish, KitPlace, Rng, orientation_of, placement, role_name};
use crate::{
    BspBuildingPlan, CardinalDirection, EdgeFeature, GenerationSnapshot, ObjectPlacement, RoomKind,
};
use osrs_core::coords::MapTile;
use std::collections::{HashMap, HashSet};

type Edge = (MapTile, CardinalDirection);

/// Whether the free tiles of a room still form one piece reachable from its doorways.
fn connected(free: &HashSet<MapTile>, seeds: &[MapTile]) -> bool {
    let Some(start) = seeds
        .iter()
        .find(|s| free.contains(s))
        .copied()
        .or_else(|| free.iter().next().copied())
    else {
        return true;
    };
    let mut seen = HashSet::from([start]);
    let mut stack = vec![start];
    while let Some(tile) = stack.pop() {
        for direction in CardinalDirection::ALL {
            if let Some(next) = direction.neighbor(tile)
                && free.contains(&next)
                && seen.insert(next)
            {
                stack.push(next);
            }
        }
    }
    seen.len() == free.len()
}

/// The footprint of a piece anchored at its south-west tile.
fn footprint(anchor: MapTile, size: (i32, i32), orientation: u8) -> Vec<MapTile> {
    let (w, h) = if orientation & 1 == 1 {
        (size.1, size.0)
    } else {
        size
    };
    (0..h)
        .flat_map(|dy| (0..w).map(move |dx| MapTile::new(anchor.x + dx, anchor.y + dy)))
        .collect()
}

pub(super) fn furnish(
    finish: &KitFinish,
    building: &BspBuildingPlan,
    feature: &HashMap<Edge, EdgeFeature>,
    snapshot: &GenerationSnapshot,
    rng: &mut Rng,
) -> Vec<ObjectPlacement> {
    let plan = &building.plan;
    let mut placed = Vec::new();
    for room in &plan.rooms {
        if room.kind == RoomKind::Hallway {
            continue;
        }
        let Some(role) = building.role_for(room.id) else {
            continue;
        };
        let Some(rules) = finish.furniture.get(role_name(role)) else {
            continue;
        };
        let rect = room.rect;
        let (x0, y0) = (rect.origin.x, rect.origin.y);
        let (x1, y1) = (
            x0 + i32::from(rect.width) - 1,
            y0 + i32::from(rect.height) - 1,
        );
        let inside = |t: &MapTile| t.x >= x0 && t.x <= x1 && t.y >= y0 && t.y <= y1;
        let mut free: HashSet<MapTile> = (y0..=y1)
            .flat_map(|y| (x0..=x1).map(move |x| MapTile::new(x, y)))
            .collect();
        // Doorways and the tiles in front of them stay clear.
        let mut clear: HashSet<MapTile> = HashSet::new();
        let doorways = plan
            .connections
            .iter()
            .map(|c| c.doorway)
            .chain(std::iter::once(plan.entrance));
        for door in doorways {
            clear.insert(door);
            clear.extend(
                CardinalDirection::ALL
                    .iter()
                    .filter_map(|d| d.neighbor(door)),
            );
        }
        for (&(tile, direction), &kind) in feature {
            if matches!(kind, EdgeFeature::Door | EdgeFeature::Open) && inside(&tile) {
                clear.insert(tile);
                clear.extend(direction.neighbor(tile));
            }
        }
        let seeds: Vec<MapTile> = clear.iter().filter(|t| inside(t)).copied().collect();
        for rule in rules {
            let wanted = usize::from(rule.min)
                + rng.below(usize::from(rule.max.saturating_sub(rule.min)) + 1);
            let size = (
                i32::from(rule.size[0].max(1)),
                i32::from(rule.size[1].max(1)),
            );
            if rule.place == KitPlace::Row {
                let count = i32::try_from(wanted).unwrap_or(1).max(1);
                'attempts: for _attempt in 0..48 {
                    let side = CardinalDirection::ALL[rng.below(4)];
                    let orientation = (orientation_of(side) + rule.relative) & 3;
                    let (w, h) = if orientation & 1 == 1 {
                        (size.1, size.0)
                    } else {
                        size
                    };
                    // The run goes along the wall: up and down a west or east wall.
                    let (step, extent) = match side {
                        CardinalDirection::West | CardinalDirection::East => ((0, 1), h),
                        CardinalDirection::North | CardinalDirection::South => ((1, 0), w),
                    };
                    let start_x = x0 + rng.below(usize::try_from(x1 - x0 + 1).unwrap_or(1)) as i32;
                    let start_y = y0 + rng.below(usize::try_from(y1 - y0 + 1).unwrap_or(1)) as i32;
                    let first = match side {
                        CardinalDirection::West => MapTile::new(x0, start_y),
                        CardinalDirection::East => MapTile::new(x1 - (w - 1), start_y),
                        CardinalDirection::South => MapTile::new(start_x, y0),
                        CardinalDirection::North => MapTile::new(start_x, y1 - (h - 1)),
                    };
                    let mut after = free.clone();
                    let mut run = Vec::new();
                    for k in 0..count {
                        let anchor = MapTile::new(
                            first.x + step.0 * k * extent,
                            first.y + step.1 * k * extent,
                        );
                        let tiles = footprint(anchor, size, orientation);
                        // Every tile touching the chosen wall must have a real wall there.
                        let mut touching = false;
                        let mut solid = true;
                        for t in &tiles {
                            if side.neighbor(*t).is_some_and(|n| !inside(&n)) {
                                touching = true;
                                solid &= matches!(
                                    feature.get(&(*t, side)),
                                    Some(
                                        EdgeFeature::ExteriorWall
                                            | EdgeFeature::InteriorWall
                                            | EdgeFeature::Window
                                    )
                                );
                            }
                        }
                        if !touching
                            || !solid
                            || tiles
                                .iter()
                                .any(|t| !inside(t) || clear.contains(t) || !after.contains(t))
                        {
                            continue 'attempts;
                        }
                        for t in &tiles {
                            after.remove(t);
                        }
                        run.push(anchor);
                    }
                    if !connected(&after, &seeds) {
                        continue;
                    }
                    for anchor in run {
                        if let Some(piece) =
                            placement(snapshot, rule.id, rule.kind, anchor, orientation)
                        {
                            placed.push(piece);
                        }
                    }
                    free = after;
                    break;
                }
                continue;
            }
            for _ in 0..wanted {
                for _attempt in 0..24 {
                    let (orientation, anchor, side) = match rule.place {
                        KitPlace::AgainstWall => {
                            let side = CardinalDirection::ALL[rng.below(4)];
                            let orientation = (orientation_of(side) + rule.relative) & 3;
                            let (w, h) = if orientation & 1 == 1 {
                                (size.1, size.0)
                            } else {
                                size
                            };
                            // The anchor puts the piece's side against the chosen wall.
                            let along_x =
                                x0 + rng.below(usize::try_from(x1 - x0 + 1).unwrap_or(1)) as i32;
                            let along_y =
                                y0 + rng.below(usize::try_from(y1 - y0 + 1).unwrap_or(1)) as i32;
                            let anchor = match side {
                                CardinalDirection::West => MapTile::new(x0, along_y),
                                CardinalDirection::East => MapTile::new(x1 - (w - 1), along_y),
                                CardinalDirection::South => MapTile::new(along_x, y0),
                                CardinalDirection::North => MapTile::new(along_x, y1 - (h - 1)),
                            };
                            (orientation, anchor, Some(side))
                        }
                        KitPlace::Row => continue,
                        KitPlace::Center => {
                            let x = x0
                                + 2
                                + rng.below(usize::try_from((x1 - x0 - 3).max(1)).unwrap_or(1))
                                    as i32;
                            let y = y0
                                + 2
                                + rng.below(usize::try_from((y1 - y0 - 3).max(1)).unwrap_or(1))
                                    as i32;
                            (rule.relative & 3, MapTile::new(x, y), None)
                        }
                    };
                    let tiles = footprint(anchor, size, orientation);
                    if tiles
                        .iter()
                        .any(|t| !inside(t) || clear.contains(t) || !free.contains(t))
                    {
                        continue;
                    }
                    if let Some(side) = side {
                        // Every tile along the touching side must be a real wall.
                        let solid = tiles
                            .iter()
                            .filter(|t| side.neighbor(**t).is_some_and(|n| !inside(&n)));
                        let mut any = false;
                        let mut ok = true;
                        for t in solid {
                            any = true;
                            ok &= matches!(
                                feature.get(&(*t, side)),
                                Some(
                                    EdgeFeature::ExteriorWall
                                        | EdgeFeature::InteriorWall
                                        | EdgeFeature::Window
                                )
                            );
                        }
                        if !any || !ok {
                            continue;
                        }
                    } else if tiles.iter().any(|t| {
                        CardinalDirection::ALL
                            .iter()
                            .any(|d| d.neighbor(*t).is_some_and(|n| !inside(&n)))
                    }) {
                        // The middle of the room, not against a wall.
                        continue;
                    }
                    let mut after = free.clone();
                    for t in &tiles {
                        after.remove(t);
                    }
                    if !connected(&after, &seeds) {
                        continue;
                    }
                    if let Some(piece) =
                        placement(snapshot, rule.id, rule.kind, anchor, orientation)
                    {
                        free = after;
                        placed.push(piece);
                    }
                    break;
                }
            }
        }
    }
    placed
}
