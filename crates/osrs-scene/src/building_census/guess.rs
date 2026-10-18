//! Guessing what an unnamed object is from how it is used in a building: wall pieces make up
//! the outside of a building, a piece that stands alone in a run of wall is a way through it,
//! roof pieces lie on the plane above, furniture stands in rooms.
//!
//! A guess is only ever a suggestion for a person to accept: it says what it saw so they can
//! judge it, and it never changes a name by itself.

use super::{Building, Dir, EdgeFeature, OPENING_OBJECT, ObjectTable};
use osrs_core::ids::ObjectId;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guess {
    pub object: ObjectId,
    /// A short name to offer ("wall", "door or window").
    pub label: String,
    /// The role to offer (`wall`, `door`, `roof`... see `osrs_core::object_names::ROLES`).
    pub role: String,
    /// How sure, 0 to 100.
    pub confidence: u8,
    /// What was seen, in a sentence a person can check.
    pub why: String,
    /// How many times the object appears in the buildings looked at.
    pub count: u32,
}

/// Whether the table gives the object a name worth keeping.
fn is_named(objects: &ObjectTable, id: ObjectId) -> bool {
    objects.get(&id).is_some_and(|info| {
        let name = info.name.trim();
        (!name.is_empty() && !name.eq_ignore_ascii_case("null"))
            || (!info.symbol.is_empty() && !osrs_core::object_names::is_opaque(&info.symbol))
    })
}

#[derive(Default)]
struct Evidence {
    exterior_wall: u32,
    interior_wall: u32,
    in_gap: u32,
    windowed: u32,
    roof: u32,
    furniture: u32,
    against: u32,
    hung: u32,
}

/// Guesses for the unnamed objects in `buildings`, most used first.
pub fn guess_objects(buildings: &[Building], objects: &ObjectTable) -> Vec<Guess> {
    let mut seen: BTreeMap<ObjectId, Evidence> = BTreeMap::new();
    for building in buildings {
        // The piece standing on each wall edge, to see which pieces stand alone in a run.
        let mut edge: BTreeMap<((i32, i32), Dir), ObjectId> = BTreeMap::new();
        for wall in &building.walls {
            if wall.piece.object != OPENING_OBJECT {
                edge.insert((wall.tile, wall.dir), wall.piece.object);
            }
        }
        for wall in &building.walls {
            let id = wall.piece.object;
            if id == OPENING_OBJECT {
                continue;
            }
            let entry = seen.entry(id).or_default();
            match (wall.piece.feature, wall.exterior) {
                (EdgeFeature::Wall, true) => entry.exterior_wall += 1,
                (EdgeFeature::Wall, false) => entry.interior_wall += 1,
                _ => {}
            }
            // Along the wall: the tiles either side of this one on the same side.
            let along = match wall.dir {
                Dir::West | Dir::East => [(0, 1), (0, -1)],
                Dir::North | Dir::South => [(1, 0), (-1, 0)],
            };
            let around: Vec<ObjectId> = along
                .iter()
                .filter_map(|(dx, dy)| {
                    edge.get(&(((wall.tile.0 + dx), (wall.tile.1 + dy)), wall.dir))
                })
                .copied()
                .collect();
            if around.len() == 2 && around[0] != id && around[1] != id {
                entry.in_gap += 1;
            }
        }
        for ((object, kind), count) in &building.roof {
            let _ = kind;
            seen.entry(*object).or_default().roof += count;
        }
        for room in &building.rooms {
            for piece in &room.furniture {
                let entry = seen.entry(piece.object).or_default();
                entry.furniture += 1;
                if piece.wall_distance == 0 {
                    entry.against += 1;
                }
            }
            for decoration in &room.decorations {
                let entry = seen.entry(decoration.object).or_default();
                if decoration.on.is_some() {
                    entry.windowed += 1;
                } else if decoration.inside {
                    entry.hung += 1;
                }
            }
        }
    }
    let mut guesses: Vec<Guess> = seen
        .into_iter()
        .filter(|(id, _)| !is_named(objects, *id))
        .filter_map(|(object, e)| guess_one(object, &e))
        .collect();
    guesses.sort_by(|a, b| b.count.cmp(&a.count).then(a.object.cmp(&b.object)));
    guesses
}

fn guess_one(object: ObjectId, e: &Evidence) -> Option<Guess> {
    let made = |label: &str, role: &str, confidence: u8, count: u32, why: String| {
        Some(Guess {
            object,
            label: label.to_owned(),
            role: role.to_owned(),
            confidence,
            why,
            count,
        })
    };
    if e.roof > 0 {
        return made(
            "roof",
            "roof",
            95,
            e.roof,
            format!(
                "It lies on the plane above the building, like a roof ({} pieces).",
                e.roof
            ),
        );
    }
    if e.in_gap > 0 && e.in_gap >= e.exterior_wall + e.interior_wall {
        return made(
            "door or window",
            "door",
            60,
            e.in_gap,
            format!(
                "It stands alone in a run of wall of another kind ({} times), the way a door or a \
                 window does.",
                e.in_gap
            ),
        );
    }
    if e.exterior_wall + e.interior_wall > 0 {
        let outside = e.exterior_wall >= e.interior_wall;
        return made(
            "wall",
            "wall",
            if outside { 85 } else { 70 },
            e.exterior_wall + e.interior_wall,
            format!(
                "It makes up the {} of buildings ({} pieces).",
                if outside {
                    "outside walls"
                } else {
                    "walls between rooms"
                },
                e.exterior_wall + e.interior_wall
            ),
        );
    }
    if e.windowed > 0 {
        return made(
            "window decoration",
            "window",
            70,
            e.windowed,
            format!(
                "It hangs on the wall piece of a window ({} times).",
                e.windowed
            ),
        );
    }
    if e.furniture > 0 {
        let wall = if e.against * 2 >= e.furniture {
            "mostly against a wall"
        } else {
            "away from the walls"
        };
        return made(
            "furniture",
            "other",
            40,
            e.furniture,
            format!("It stands inside rooms ({} times), {wall}.", e.furniture),
        );
    }
    if e.hung > 0 {
        return made(
            "wall decoration",
            "painting",
            50,
            e.hung,
            format!("It hangs on the inside of walls ({} times).", e.hung),
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::{CensusArea, Ground, ObjectInfo, analyze};
    use super::*;
    use crate::object_semantics::Placed;

    const WALL: u32 = 10;
    const GATE: u32 = 11;
    const TABLE: u32 = 21;
    const ROOF: u32 = 30;

    fn put(id: u32, kind: u8, orientation: u8, tile: (i32, i32), plane: u8) -> Placed {
        Placed {
            id: ObjectId::new(id),
            kind,
            orientation,
            tile,
            plane,
        }
    }

    /// A 6x3 room: walls all round, an unnamed piece in the south wall, a table, a roof above.
    fn buildings() -> Vec<Building> {
        let mut area = CensusArea::new((100, 100), 20, 16);
        for x in 100..120 {
            for y in 100..116 {
                area.set(
                    0,
                    (x, y),
                    Ground {
                        underlay: 5,
                        height_known: true,
                        ..Ground::default()
                    },
                );
            }
        }
        let mut placements = Vec::new();
        for x in 103..=108 {
            placements.push(put(WALL, 0, 3, (x, 103), 0));
            placements.push(put(WALL, 0, 1, (x, 105), 0));
        }
        for y in 103..=105 {
            placements.push(put(WALL, 0, 0, (103, y), 0));
            placements.push(put(WALL, 0, 2, (108, y), 0));
        }
        placements.retain(|p| !(p.tile == (105, 103) && p.orientation == 3));
        placements.push(put(GATE, 0, 3, (105, 103), 0));
        placements.push(put(TABLE, 10, 0, (106, 104), 0));
        for x in 103..=108 {
            for y in 103..=105 {
                placements.push(put(ROOF, 12, 1, (x, y), 1));
            }
        }
        analyze(&area, &placements, &ObjectTable::new())
    }

    #[test]
    fn unnamed_pieces_get_a_guess_that_says_what_was_seen() {
        let found = buildings();
        assert_eq!(found.len(), 1);
        let guesses = guess_objects(&found, &ObjectTable::new());
        let by = |id: u32| guesses.iter().find(|g| g.object == ObjectId::new(id));
        assert_eq!(by(WALL).map(|g| g.role.as_str()), Some("wall"));
        assert!(by(WALL).is_some_and(|g| g.why.contains("outside walls")));
        assert_eq!(by(GATE).map(|g| g.label.as_str()), Some("door or window"));
        assert_eq!(by(ROOF).map(|g| g.role.as_str()), Some("roof"));
        assert_eq!(by(TABLE).map(|g| g.label.as_str()), Some("furniture"));
        assert!(
            guesses.windows(2).all(|w| w[0].count >= w[1].count),
            "most used first"
        );
    }

    #[test]
    fn objects_that_already_have_names_get_no_guess() {
        let found = buildings();
        let mut named = ObjectTable::new();
        for id in [WALL, GATE, TABLE, ROOF] {
            named.insert(
                ObjectId::new(id),
                ObjectInfo {
                    name: "something".to_owned(),
                    ..ObjectInfo::default()
                },
            );
        }
        assert!(guess_objects(&found, &named).is_empty());
    }
}
