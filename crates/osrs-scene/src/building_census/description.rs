//! Placement-level building evidence. A roof visibility component is never an ownership ID.
use super::{CensusArea, Dir, EdgeFeature, Edges, ObjectTable, analyze};
use crate::object_semantics::Placed;
use osrs_core::coords::MapTile;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberRole {
    Boundary,
    Decoration,
    Furniture,
    GroundDecoration,
    Roof,
    Stair,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    BoundaryIncident,
    SameAnchorBoundary,
    FootprintInside,
    AboveFloor,
    StairVerb,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub placement: Placed,
    pub role: MemberRole,
    pub evidence: Evidence,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKind {
    Continues,
    CornerArm,
    Decorates,
    AgainstWall,
    RoofAbove,
    StairPair,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub from: usize,
    pub to: usize,
    pub kind: ConnectionKind,
    pub evidence: Evidence,
}
/// An edge with canonical anchor (west or north), plus its original placement occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boundary {
    pub member: usize,
    pub tile: MapTile,
    pub side: Dir,
    pub exterior: bool,
    pub feature: EdgeFeature,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Portal {
    pub member: usize,
    pub approaches: [MapTile; 2],
    pub exterior: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingDescription {
    pub floors: BTreeMap<u8, BTreeSet<MapTile>>,
    pub members: Vec<Member>,
    pub boundaries: Vec<Boundary>,
    pub portals: Vec<Portal>,
    pub connections: Vec<Connection>,
    /// These must be resolved by the capture review, never silently treated as confidence.
    pub review: Vec<String>,
}

pub fn canonical_edge(tile: MapTile, dir: Dir) -> (MapTile, Dir) {
    match dir {
        Dir::East => (MapTile::new(tile.x + 1, tile.y), Dir::West),
        Dir::South => (MapTile::new(tile.x, tile.y - 1), Dir::North),
        _ => (tile, dir),
    }
}

fn arms(p: &Placed) -> Vec<(MapTile, Dir)> {
    let side = Dir::from_orientation(p.orientation);
    let tile = MapTile::new(p.tile.0, p.tile.1);
    match p.kind {
        0 => vec![(tile, side)],
        2 => vec![(tile, side), (tile, side.next())],
        _ => vec![],
    }
}
fn incident(floor: &BTreeSet<MapTile>, tile: MapTile, side: Dir) -> bool {
    let other = side.from((tile.x, tile.y));
    floor.contains(&tile) || floor.contains(&MapTile::new(other.0, other.1))
}
fn decoration_on(p: &Placed, floor: &BTreeSet<MapTile>, placements: &[Placed]) -> bool {
    let tile = MapTile::new(p.tile.0, p.tile.1);
    let side = Dir::from_orientation(p.orientation);
    let candidates = [
        canonical_edge(tile, side),
        canonical_edge(tile, side.opposite()),
    ];
    placements.iter().filter(|w| w.plane == p.plane).any(|w| {
        arms(w)
            .iter()
            .any(|&(t, d)| incident(floor, t, d) && candidates.contains(&canonical_edge(t, d)))
    })
}
fn endpoints(tile: MapTile, dir: Dir) -> [MapTile; 2] {
    match dir {
        Dir::West => [tile, MapTile::new(tile.x, tile.y + 1)],
        Dir::North => [
            MapTile::new(tile.x, tile.y + 1),
            MapTile::new(tile.x + 1, tile.y + 1),
        ],
        Dir::East => [
            MapTile::new(tile.x + 1, tile.y),
            MapTile::new(tile.x + 1, tile.y + 1),
        ],
        Dir::South => [tile, MapTile::new(tile.x + 1, tile.y)],
    }
}
fn corner(p: &Placed) -> MapTile {
    let (x, y) = p.tile;
    match p.orientation & 3 {
        0 => MapTile::new(x, y + 1),
        1 => MapTile::new(x + 1, y + 1),
        2 => MapTile::new(x + 1, y),
        _ => MapTile::new(x, y),
    }
}
fn corner_incident(p: &Placed, floor: &BTreeSet<MapTile>, placements: &[Placed]) -> bool {
    matches!(p.kind, 1 | 3)
        && placements.iter().filter(|w| w.plane == p.plane).any(|w| {
            arms(w)
                .iter()
                .any(|&(t, d)| incident(floor, t, d) && endpoints(t, d).contains(&corner(p)))
        })
}
fn inside(p: &Placed, floor: &BTreeSet<MapTile>, table: &ObjectTable) -> bool {
    let (w, h) = if matches!(p.kind, 10 | 11) {
        table.get(&p.id).map_or((1, 1), |d| d.size)
    } else {
        (1, 1)
    };
    let (w, h) = if p.orientation & 1 == 1 {
        (h, w)
    } else {
        (w, h)
    };
    (0..w.max(1)).all(|x| {
        (0..h.max(1)).all(|y| {
            floor.contains(&MapTile::new(
                p.tile.0 + i32::from(x),
                p.tile.1 + i32::from(y),
            ))
        })
    })
}
fn stair(p: &Placed, table: &ObjectTable) -> bool {
    table.get(&p.id).is_some_and(|d| {
        d.actions
            .iter()
            .any(|a| a.starts_with("climb") || a == "ascend" || a == "descend")
    })
}

/// Describe the rooms accessible through structural portals from the seed, with upper floors as
/// reviewable geometric candidates. Collision state does not erase a closed door's portal.
pub fn describe(
    area: &CensusArea,
    placements: &[Placed],
    table: &ObjectTable,
    seed: MapTile,
    plane: u8,
) -> Option<BuildingDescription> {
    let buildings = analyze(area, placements, table);
    describe_with_census(area, placements, table, &buildings, seed, plane)
}

/// Reuse an already computed census when exporting many seeds from the same contextual window.
pub fn describe_with_census(
    area: &CensusArea,
    placements: &[Placed],
    table: &ObjectTable,
    buildings: &[super::Building],
    seed: MapTile,
    plane: u8,
) -> Option<BuildingDescription> {
    let base = buildings
        .iter()
        .find(|b| b.plane == plane && b.tiles().any(|t| t == (seed.x, seed.y)))?;
    let edges = Edges::of_plane(placements, plane, table);
    let rooms = &base.rooms;
    let start = rooms
        .iter()
        .position(|r| r.tiles.contains(&(seed.x, seed.y)))?;
    let mut included = BTreeSet::from([start]);
    loop {
        let mut next = included.clone();
        for &index in &included {
            for &tile in &rooms[index].tiles {
                for dir in Dir::ALL {
                    if edges.between(tile, dir).is_some_and(|e| {
                        matches!(e.feature, EdgeFeature::Door | EdgeFeature::Opening)
                    }) {
                        let neighbour = dir.from(tile);
                        if let Some(i) = rooms.iter().position(|r| r.tiles.contains(&neighbour)) {
                            next.insert(i);
                        }
                    }
                }
            }
        }
        if next == included {
            break;
        }
        included = next;
    }
    let base_floor: BTreeSet<_> = included
        .iter()
        .flat_map(|&i| rooms[i].tiles.iter().map(|&(x, y)| MapTile::new(x, y)))
        .collect();
    let mut out = BuildingDescription {
        floors: BTreeMap::from([(plane, base_floor)]),
        members: vec![],
        boundaries: vec![],
        portals: vec![],
        connections: vec![],
        review: vec![],
    };
    if included.len() != rooms.len() {
        out.review.push("A shared wall separates this building from other enclosed rooms; review boundary ownership.".into());
    }
    for upper in plane + 1..4 {
        let lower = out.floors.get(&(upper - 1))?;
        let candidates: Vec<_> = buildings
            .iter()
            .filter(|b| {
                b.plane == upper && b.tiles().any(|(x, y)| lower.contains(&MapTile::new(x, y)))
            })
            .collect();
        if candidates.is_empty() {
            break;
        }
        if candidates.len() > 1 {
            out.review.push(format!(
                "Multiple structures overlap on plane {upper}; split or exclude their floors."
            ));
        }
        let tiles = candidates
            .iter()
            .flat_map(|b| b.tiles().map(|(x, y)| MapTile::new(x, y)))
            .collect();
        out.floors.insert(upper, tiles);
        out.review.push(format!(
            "Confirm that the candidate floor on plane {upper} belongs to this building."
        ));
    }
    Some(describe_floors(area, placements, table, out))
}

/// Recompute placement evidence for explicitly reviewed room/floor membership without re-running
/// the flood fill. Human membership overrides belong to the capture controller, not this analysis.
pub fn describe_floors(
    area: &CensusArea,
    placements: &[Placed],
    table: &ObjectTable,
    mut out: BuildingDescription,
) -> BuildingDescription {
    out.members.clear();
    out.boundaries.clear();
    out.portals.clear();
    out.connections.clear();
    let mut ordered = placements.to_vec();
    ordered.sort_by_key(|p| {
        (
            p.plane,
            p.tile.1,
            p.tile.0,
            p.kind,
            p.id.get(),
            p.orientation,
        )
    });
    ordered.dedup();
    for p in ordered {
        let tile = MapTile::new(p.tile.0, p.tile.1);
        let floor = out.floors.get(&p.plane);
        let roof_floor = p.plane.checked_sub(1).and_then(|v| out.floors.get(&v));
        let role = if floor.is_some_and(|f| {
            arms(&p).iter().any(|&(t, d)| incident(f, t, d)) || corner_incident(&p, f, placements)
        }) {
            Some((MemberRole::Boundary, Evidence::BoundaryIncident))
        } else if (4..=8).contains(&p.kind)
            && floor.is_some_and(|f| decoration_on(&p, f, placements))
        {
            Some((MemberRole::Decoration, Evidence::SameAnchorBoundary))
        } else if floor.is_some_and(|f| inside(&p, f, table)) {
            Some(if stair(&p, table) {
                (MemberRole::Stair, Evidence::StairVerb)
            } else if p.kind == 22 {
                (MemberRole::GroundDecoration, Evidence::FootprintInside)
            } else if (4..=8).contains(&p.kind) {
                (MemberRole::Decoration, Evidence::FootprintInside)
            } else {
                (MemberRole::Furniture, Evidence::FootprintInside)
            })
        } else if (12..=21).contains(&p.kind)
            && roof_floor.is_some_and(|f| {
                (-1..=1).any(|dx| {
                    (-1..=1).any(|dy| f.contains(&MapTile::new(tile.x + dx, tile.y + dy)))
                })
            })
        {
            Some((MemberRole::Roof, Evidence::AboveFloor))
        } else {
            None
        };
        if let Some((role, evidence)) = role {
            out.members.push(Member {
                placement: p,
                role,
                evidence,
            });
        }
    }
    for (index, member) in out.members.iter().enumerate() {
        if member.role != MemberRole::Boundary {
            continue;
        }
        let p = member.placement;
        let floor = &out.floors[&p.plane];
        for (tile, side) in arms(&p) {
            if !incident(floor, tile, side) {
                continue;
            }
            let other = side.from((tile.x, tile.y));
            let exterior = floor.contains(&tile) != floor.contains(&MapTile::new(other.0, other.1));
            let feature = super::edges::feature_of(table.get(&p.id));
            let (anchor, dir) = canonical_edge(tile, side);
            out.boundaries.push(Boundary {
                member: index,
                tile: anchor,
                side: dir,
                exterior,
                feature,
            });
            if feature == EdgeFeature::Door {
                out.portals.push(Portal {
                    member: index,
                    approaches: [tile, MapTile::new(other.0, other.1)],
                    exterior,
                });
            }
        }
    }
    for (a, from) in out.members.iter().enumerate() {
        for (b, to) in out.members.iter().enumerate() {
            if a == b {
                continue;
            }
            let p = from.placement;
            let q = to.placement;
            let distance = (p.tile.0 - q.tile.0).abs() + (p.tile.1 - q.tile.1).abs();
            let relation = if p.plane == q.plane
                && from.role == MemberRole::Decoration
                && to.role == MemberRole::Boundary
                && p.tile == q.tile
            {
                Some((ConnectionKind::Decorates, Evidence::SameAnchorBoundary))
            } else if p.plane == q.plane
                && from.role == MemberRole::Boundary
                && to.role == MemberRole::Boundary
                && a < b
            {
                let shared = arms(&p).iter().any(|&(t, d)| {
                    arms(&q).iter().any(|&(u, e)| {
                        let (t, d) = canonical_edge(t, d);
                        let (u, e) = canonical_edge(u, e);
                        d == e
                            && match d {
                                Dir::West => t.x == u.x && (t.y - u.y).abs() == 1,
                                _ => t.y == u.y && (t.x - u.x).abs() == 1,
                            }
                    })
                });
                shared.then_some((
                    if p.kind == 2 || q.kind == 2 {
                        ConnectionKind::CornerArm
                    } else {
                        ConnectionKind::Continues
                    },
                    Evidence::BoundaryIncident,
                ))
            } else if p.plane == q.plane
                && matches!(from.role, MemberRole::Furniture | MemberRole::Stair)
                && to.role == MemberRole::Boundary
                && distance <= 1
            {
                Some((ConnectionKind::AgainstWall, Evidence::FootprintInside))
            } else if from.role == MemberRole::Roof
                && q.plane.checked_add(1) == Some(p.plane)
                && to.role == MemberRole::Boundary
                && distance <= 1
            {
                Some((ConnectionKind::RoofAbove, Evidence::AboveFloor))
            } else if from.role == MemberRole::Stair
                && to.role == MemberRole::Stair
                && p.plane.checked_add(1) == Some(q.plane)
                && distance <= 1
            {
                Some((ConnectionKind::StairPair, Evidence::StairVerb))
            } else {
                None
            };
            if let Some((kind, evidence)) = relation {
                out.connections.push(Connection {
                    from: a,
                    to: b,
                    kind,
                    evidence,
                });
            }
        }
    }
    for floor in out.floors.values() {
        if floor
            .iter()
            .any(|t| Dir::ALL.iter().any(|d| !area.contains(d.from((t.x, t.y)))))
        {
            out.review.push(
                "The building touches the available analysis boundary; load neighboring regions."
                    .into(),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests;
