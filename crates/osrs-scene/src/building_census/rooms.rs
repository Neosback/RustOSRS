//! Finding rooms and buildings: flood fill across wall edges, then group rooms that touch.

use super::{
    Building, CensusArea, Decoration, Dir, EdgePiece, Edges, Furniture, Ground, Material,
    ObjectTable, Room, WallRecord, tag_room,
};
use crate::object_semantics::Placed;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

/// Smallest and largest enclosed area taken for a room, in tiles.
const MIN_ROOM_TILES: usize = 4;
const MAX_ROOM_TILES: usize = 500;

type Tile = (i32, i32);

/// Label the connected tile sets of one plane. A set that reaches the edge of the area, or is
/// larger than any room, is "outside" (label `None`).
fn components(area: &CensusArea, edges: &Edges) -> HashMap<Tile, Option<usize>> {
    let mut label: HashMap<Tile, Option<usize>> = HashMap::new();
    let mut next = 0;
    let (ox, oy) = area.origin;
    for x in 0..area.width as i32 {
        for y in 0..area.height as i32 {
            let start = (ox + x, oy + y);
            if label.contains_key(&start) {
                continue;
            }
            let mut queue = VecDeque::from([start]);
            let mut members = vec![start];
            let mut touches_edge = false;
            label.insert(start, None);
            while let Some(tile) = queue.pop_front() {
                for dir in Dir::ALL {
                    let neighbour = dir.from(tile);
                    if !area.contains(neighbour) {
                        touches_edge |= edges.between(tile, dir).is_none();
                        continue;
                    }
                    if edges.between(tile, dir).is_some() || label.contains_key(&neighbour) {
                        continue;
                    }
                    label.insert(neighbour, None);
                    members.push(neighbour);
                    queue.push_back(neighbour);
                }
            }
            if !touches_edge && (MIN_ROOM_TILES..=MAX_ROOM_TILES).contains(&members.len()) {
                for tile in &members {
                    label.insert(*tile, Some(next));
                }
                next += 1;
            }
        }
    }
    label
}

fn root(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

/// Every building found on the four planes of `area`.
pub fn analyze(area: &CensusArea, placements: &[Placed], objects: &ObjectTable) -> Vec<Building> {
    let mut buildings = Vec::new();
    for plane in 0..4u8 {
        let edges = Edges::of_plane(placements, plane, objects);
        if edges.pieces().next().is_none() {
            continue;
        }
        let label = components(area, &edges);
        let room_count = label.values().flatten().max().map_or(0, |m| m + 1);
        if room_count == 0 {
            continue;
        }
        let mut tiles_of: Vec<Vec<Tile>> = vec![Vec::new(); room_count];
        for (tile, room) in &label {
            if let Some(room) = room {
                tiles_of[*room].push(*tile);
            }
        }
        for tiles in &mut tiles_of {
            tiles.sort_unstable();
        }
        // Rooms that share a wall or a door belong to one building.
        let mut parent: Vec<usize> = (0..room_count).collect();
        for (tile, room) in &label {
            let Some(room) = room else { continue };
            for dir in Dir::ALL {
                if edges.between(*tile, dir).is_none() {
                    continue;
                }
                if let Some(Some(other)) = label.get(&dir.from(*tile)) {
                    let (a, b) = (root(&mut parent, *room), root(&mut parent, *other));
                    parent[a] = b;
                }
            }
        }
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for room in 0..room_count {
            let group = root(&mut parent, room);
            groups.entry(group).or_default().push(room);
        }
        for rooms in groups.values() {
            if let Some(building) = build(
                area, placements, objects, &edges, &label, &tiles_of, rooms, plane,
            ) {
                buildings.push(building);
            }
        }
    }
    buildings
}

#[allow(clippy::too_many_arguments)]
fn build(
    area: &CensusArea,
    placements: &[Placed],
    objects: &ObjectTable,
    edges: &Edges,
    label: &HashMap<Tile, Option<usize>>,
    tiles_of: &[Vec<Tile>],
    room_ids: &[usize],
    plane: u8,
) -> Option<Building> {
    let in_building: HashSet<usize> = room_ids.iter().copied().collect();
    let owner = |tile: Tile| -> Option<usize> {
        label
            .get(&tile)
            .copied()
            .flatten()
            .filter(|room| in_building.contains(room))
    };
    // Index placements once per tile for speed.
    let mut by_tile: HashMap<Tile, Vec<&Placed>> = HashMap::new();
    for placed in placements
        .iter()
        .filter(|p| p.plane == plane || p.plane == plane + 1)
    {
        by_tile.entry(placed.tile).or_default().push(placed);
    }

    let mut walls = Vec::new();
    let mut door_aprons = Vec::new();
    let mut roof: BTreeMap<_, u32> = BTreeMap::new();
    let mut ring: BTreeMap<Material, u32> = BTreeMap::new();
    let mut ring_tiles: BTreeSet<Tile> = BTreeSet::new();
    let mut rooms = Vec::new();
    for &room_id in room_ids {
        let tiles = &tiles_of[room_id];
        let tile_set: HashSet<Tile> = tiles.iter().copied().collect();
        let mut furniture = Vec::new();
        let mut decorations = Vec::new();
        let mut ground_decor = Vec::new();
        let mut floor: BTreeMap<Material, u32> = BTreeMap::new();
        let mut roofed = 0;
        for &tile in tiles {
            if let Some(ground) = area.ground(plane, tile) {
                *floor.entry(Material::from(ground)).or_default() += 1;
            }
            for dir in Dir::ALL {
                let Some(piece) = edges.between(tile, dir) else {
                    continue;
                };
                let neighbour = dir.from(tile);
                let neighbour_room = owner(neighbour);
                let exterior = neighbour_room.is_none();
                // Interior walls once, from the lower tile; exterior walls from the inside.
                if exterior && piece.feature == super::EdgeFeature::Door {
                    let second = dir.from(neighbour);
                    let material = |t: Tile| area.ground(plane, t).map(Material::from);
                    if let (Some(first), Some(second)) = (material(neighbour), material(second)) {
                        door_aprons.push((first, second));
                    }
                }
                if exterior || neighbour_room.is_some_and(|_| tile < neighbour) {
                    walls.push(WallRecord {
                        piece,
                        tile,
                        dir,
                        exterior,
                    });
                }
                if exterior
                    && ring_tiles.insert(neighbour)
                    && let Some(ground) = area.ground(plane, neighbour)
                {
                    *ring.entry(Material::from(ground)).or_default() += 1;
                }
            }
            let mut has_roof = false;
            for placed in by_tile.get(&tile).into_iter().flatten() {
                if placed.plane == plane + 1 && (12..=21).contains(&placed.kind) {
                    *roof.entry((placed.id, placed.kind)).or_default() += 1;
                    has_roof = true;
                    continue;
                }
                if placed.plane != plane {
                    continue;
                }
                match placed.kind {
                    10 | 11 => furniture.push(furniture_at(placed, &tile_set, edges)),
                    22 => ground_decor.push((placed.id, tile)),
                    4..=8 => decorations.push(decoration_at(placed, edges, true)),
                    _ => {}
                }
            }
            roofed += u32::from(has_roof);
        }
        let tag = tag_room(&furniture, objects);
        rooms.push(Room {
            tiles: tiles.clone(),
            plane,
            tag,
            floor,
            furniture,
            decorations,
            ground_decor,
            roofed: roofed as f32 / tiles.len().max(1) as f32,
        });
    }
    // Decorations on the outside of the walls stand on the ring tiles.
    for &tile in &ring_tiles {
        for placed in by_tile.get(&tile).into_iter().flatten() {
            if placed.plane == plane && (4..=8).contains(&placed.kind) {
                let decoration = decoration_at(placed, edges, false);
                if let Some(room) = rooms.iter_mut().find(|room| {
                    room.tiles
                        .iter()
                        .any(|t| Dir::ALL.iter().any(|d| d.from(*t) == tile))
                }) {
                    room.decorations.push(decoration);
                }
            }
        }
    }
    let heights = |tiles: &mut dyn Iterator<Item = Tile>| -> i32 {
        let values: Vec<i32> = tiles
            .filter_map(|t| {
                area.ground(plane, t)
                    .filter(|g: &Ground| g.height_known)
                    .map(|g| g.height)
            })
            .collect();
        values.iter().max().copied().unwrap_or(0) - values.iter().min().copied().unwrap_or(0)
    };
    let all_tiles: Vec<Tile> = rooms.iter().flat_map(|r| r.tiles.iter().copied()).collect();
    let spread_inside = heights(&mut all_tiles.iter().copied());
    let spread_ring = heights(&mut ring_tiles.iter().copied());
    walls.sort_by_key(|w| (w.tile, w.dir));
    Some(Building {
        plane,
        rooms,
        walls,
        roof,
        ring,
        door_aprons,
        spread_inside,
        spread_ring,
    })
}

/// How far a piece stands from the walls of its room, and against which side.
fn furniture_at(placed: &Placed, room: &HashSet<Tile>, edges: &Edges) -> Furniture {
    let mut nearest: Option<(u8, Dir)> = None;
    for dir in Dir::ALL {
        let mut steps = 0u8;
        let mut at = placed.tile;
        loop {
            if edges.between(at, dir).is_some() || !room.contains(&dir.from(at)) {
                break;
            }
            at = dir.from(at);
            steps += 1;
            if steps > 40 {
                break;
            }
        }
        if nearest.is_none_or(|(best, _)| steps < best) {
            nearest = Some((steps, dir));
        }
    }
    let (wall_distance, side) = nearest.unwrap_or((0, Dir::West));
    Furniture {
        object: placed.id,
        kind: placed.kind,
        tile: placed.tile,
        orientation: placed.orientation,
        wall_distance,
        against: (wall_distance <= 1).then_some(side),
    }
}

fn decoration_at(placed: &Placed, edges: &Edges, inside: bool) -> Decoration {
    let dir = Dir::from_orientation(placed.orientation);
    // Look on the side the orientation names first, then the opposite side.
    let (side, on): (Option<Dir>, Option<EdgePiece>) = match edges.between(placed.tile, dir) {
        Some(piece) => (Some(dir), Some(piece)),
        None => match edges.between(placed.tile, dir.opposite()) {
            Some(piece) => (Some(dir.opposite()), Some(piece)),
            None => (None, None),
        },
    };
    Decoration {
        edge_side: side,
        object: placed.id,
        kind: placed.kind,
        tile: placed.tile,
        orientation: placed.orientation,
        on: on.map(|piece| piece.object),
        inside,
    }
}
