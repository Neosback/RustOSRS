//! Picking is a query over the same census used by capture, never over renderer membership.
use super::*;

/// A reviewable building side. A shared boundary can yield more than one candidate.
#[derive(Debug, Clone)]
pub struct BuildingCandidate {
    pub seed: MapTile,
    pub plane: u8,
    pub description: BuildingDescription,
}

/// Resolve a floor, wall anchor, or roof click against canonical placements. Exact wall arms
/// take priority over proximity. Disconnected rooms on opposite sides remain separate choices.
pub fn pick_candidates(
    area: &CensusArea,
    placements: &[Placed],
    table: &ObjectTable,
    tile: MapTile,
    plane: u8,
    hit: Option<Placed>,
) -> Vec<BuildingCandidate> {
    let buildings = analyze(area, placements, table);
    let incident: BTreeSet<_> = hit
        .iter()
        .flat_map(arms)
        .flat_map(|(anchor, side)| {
            let other = side.from((anchor.x, anchor.y));
            [anchor, MapTile::new(other.0, other.1)]
        })
        .collect();
    let mut seeds = Vec::new();
    for candidate_plane in [Some(plane), plane.checked_sub(1)].into_iter().flatten() {
        for building in buildings.iter().filter(|b| b.plane == candidate_plane) {
            for room in &building.rooms {
                if let Some(&(x, y)) = room.tiles.iter().min_by_key(|&&(x, y)| {
                    (
                        if incident.is_empty() {
                            0
                        } else {
                            u8::from(!incident.contains(&MapTile::new(x, y)))
                        },
                        (x - tile.x).abs() + (y - tile.y).abs(),
                        x,
                        y,
                    )
                }) {
                    let exact = (x, y) == (tile.x, tile.y);
                    let on_arm = incident.contains(&MapTile::new(x, y));
                    let near = (x - tile.x).abs() <= 1 && (y - tile.y).abs() <= 1;
                    if on_arm || (incident.is_empty() && (exact || near)) {
                        seeds.push((
                            !on_arm,
                            !exact,
                            plane - candidate_plane,
                            x,
                            y,
                            candidate_plane,
                        ));
                    }
                }
            }
        }
    }
    seeds.sort();
    let mut result: Vec<BuildingCandidate> = Vec::new();
    for (_, _, _, x, y, plane) in seeds {
        let seed = MapTile::new(x, y);
        if let Some(description) =
            describe_with_census(area, placements, table, &buildings, seed, plane)
            && !result
                .iter()
                .any(|c| c.plane == plane && c.description.floors == description.floors)
        {
            result.push(BuildingCandidate {
                seed,
                plane,
                description,
            });
        }
    }
    result
}

/// An open shell is offered as bounded evidence, never as a silently closed building.
/// Matching ground is flood-filled through canonical wall edges within eight tiles of the pick.
pub fn incomplete_candidate(
    area: &CensusArea,
    placements: &[Placed],
    table: &ObjectTable,
    tile: MapTile,
    plane: u8,
    hit: Option<Placed>,
) -> Option<BuildingCandidate> {
    let mut seeds: BTreeSet<_> = hit
        .iter()
        .flat_map(arms)
        .flat_map(|(t, d)| {
            let n = d.from((t.x, t.y));
            [t, MapTile::new(n.0, n.1)]
        })
        .collect();
    seeds.insert(tile);
    let seed = seeds
        .into_iter()
        .filter(|t| {
            area.ground(plane, (t.x, t.y))
                .is_some_and(|g| g.height_known)
        })
        .max_by_key(|t| {
            area.ground(plane, (t.x, t.y))
                .map_or((false, false), |g| (g.overlay != 0, *t == tile))
        })?;
    let ground = area.ground(plane, (seed.x, seed.y))?;
    let edges = Edges::of_plane(placements, plane, table);
    let mut queue = std::collections::VecDeque::from([seed]);
    let mut floor = BTreeSet::new();
    while let Some(t) = queue.pop_front() {
        if (t.x - seed.x).abs() > 8 || (t.y - seed.y).abs() > 8 || floor.contains(&t) {
            continue;
        }
        if !area.ground(plane, (t.x, t.y)).is_some_and(|g| {
            g.height_known && g.overlay == ground.overlay && g.underlay == ground.underlay
        }) {
            continue;
        }
        floor.insert(t);
        for dir in Dir::ALL {
            if edges.between((t.x, t.y), dir).is_none() {
                let n = dir.from((t.x, t.y));
                queue.push_back(MapTile::new(n.0, n.1));
            }
        }
    }
    if floor.is_empty() {
        return None;
    }
    let description = describe_floors(area, placements, table, BuildingDescription {
        floors: BTreeMap::from([(plane,floor)]), members: vec![], boundaries:vec![], portals:vec![], connections:vec![],
        review:vec!["INCOMPLETE: no closed building was found. This candidate is limited to matching ground within eight tiles; paint its floor and review every member before capture.".into()],
    });
    Some(BuildingCandidate {
        seed,
        plane,
        description,
    })
}
