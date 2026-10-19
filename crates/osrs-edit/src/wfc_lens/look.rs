//! One symbolic extraction contract shared by the lens and multi-prefab catalog learner.
use super::*;

/// `(dx, dy, direction)` for the four arms of a boundary piece, indexed by orientation
/// (west, north, east, south); the direction is north 0, east 1, south 2, west 3.
const ARMS: [(i32, i32, usize); 4] = [(-1, 0, 3), (0, 1, 0), (1, 0, 1), (0, -1, 2)];

/// The solver's view of one plane of `prefab`. `classify` says what an object (by id and kind) is.
pub fn look(
    prefab: &Prefab,
    plane: u8,
    classify: &dyn Fn(u32, u8) -> PieceClass,
) -> Result<Lens, String> {
    let (w, h) = (usize::from(prefab.width), usize::from(prefab.height));
    let whole_rectangle = prefab.building.is_none();
    let mut floor = vec![false; w * h];
    match &prefab.building {
        Some(b) => {
            for cell in b.floors.iter().filter(|c| c.plane == plane) {
                if usize::from(cell.x) < w && usize::from(cell.y) < h {
                    floor[usize::from(cell.y) * w + usize::from(cell.x)] = true;
                }
            }
        }
        None => floor.iter_mut().for_each(|f| *f = true),
    }
    let mut sockets = vec![[0_u16; 4]; w * h];
    let mut ignored = LensIgnored::default();
    let (mut doors, mut walls) = (0_u32, 0_u32);
    for (index, object) in prefab
        .objects
        .iter()
        .enumerate()
        .filter(|(_, o)| o.plane == plane)
    {
        let labelled = prefab
            .building
            .as_ref()
            .and_then(|b| {
                b.labels
                    .get(&format!("human:member:{index}:role"))
                    .or_else(|| {
                        b.labels
                            .get(&format!("human:object:{}:{}", object.id, object.kind))
                    })
                    .or_else(|| b.labels.get(&format!("inferred:member:{index}:role")))
            })
            .map(|role| role.to_lowercase());
        match object.kind {
            0 | 2 => {
                let class = match labelled.as_deref() {
                    Some("door") => PieceClass::Door,
                    Some("window") => PieceClass::Window,
                    Some("wall") => PieceClass::Wall,
                    _ => classify(object.id, object.kind),
                };
                let feature = match class {
                    PieceClass::Door => 2,
                    PieceClass::Window => 3,
                    _ => 1,
                };
                if class == PieceClass::Door {
                    doors += 1;
                } else {
                    walls += 1;
                }
                let mut arms = vec![usize::from(object.orientation & 3)];
                if object.kind == 2 {
                    arms.push(usize::from((object.orientation + 1) & 3));
                }
                for arm in arms {
                    let (dx, dy, dir) = ARMS[arm];
                    let here = (i32::from(object.x), i32::from(object.y));
                    let there = (here.0 + dx, here.1 + dy);
                    let at = |t: (i32, i32)| {
                        (t.0 >= 0 && t.1 >= 0 && (t.0 as usize) < w && (t.1 as usize) < h)
                            .then(|| t.1 as usize * w + t.0 as usize)
                    };
                    if let Some(a) = at(here) {
                        sockets[a][dir] = feature;
                    }
                    if let Some(a) = at(there) {
                        sockets[a][(dir + 2) % 4] = feature;
                    }
                }
            }
            4..=9 => ignored.decorations += 1,
            10 | 11 => ignored.furniture += 1,
            12..=21 => ignored.roof += 1,
            _ => ignored.other += 1,
        }
    }
    let mut tiles: BTreeMap<(u16, u16), (u32, u32, u8, u8)> = BTreeMap::new();
    for t in prefab.tiles.iter().filter(|t| t.plane == plane) {
        tiles.insert(
            (t.x, t.y),
            (
                t.underlay.unwrap_or(0),
                t.overlay.as_ref().map_or(0, |o| o.id),
                t.overlay.as_ref().map_or(0, |o| o.shape),
                t.overlay.as_ref().map_or(0, |o| o.rotation),
            ),
        );
    }
    let mut counts: BTreeMap<LearnedPattern, u32> = BTreeMap::new();
    let mut cell_pattern: Vec<Option<LearnedPattern>> = vec![None; w * h];
    for y in 0..h {
        for x in 0..w {
            if !floor[y * w + x] {
                continue;
            }
            let (underlay, overlay, shape, rotation) = tiles
                .get(&(x as u16, y as u16))
                .copied()
                .unwrap_or((0, 0, 0, 0));
            if underlay > 0xffff || overlay > 0xffff {
                return Err("a floor material number is too large".into());
            }
            let mut edges = sockets[y * w + x];
            for (d, dx, dy) in [(0, 0, 1), (1, 1, 0), (2, 0, -1), (3, -1, 0)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                let outside = nx < 0
                    || ny < 0
                    || nx >= w as i32
                    || ny >= h as i32
                    || !floor[ny as usize * w + nx as usize];
                if outside {
                    edges[d] = if prefab.building.as_ref().is_some_and(|b| !b.reviewed) {
                        8
                    } else {
                        edges[d] + 4
                    };
                }
            }
            let pattern = LearnedPattern {
                shape,
                rotation,
                floor_class: (underlay << 16) | overlay,
                sockets: edges,
            };
            *counts.entry(pattern.clone()).or_default() += 1;
            cell_pattern[y * w + x] = Some(pattern);
        }
    }
    let floors = counts.values().sum::<u32>();
    if counts.is_empty() {
        return Err("this building has no floor tiles on that floor".into());
    }
    let patterns: Vec<LearnedPattern> = counts.keys().cloned().collect();
    let weights: Vec<u32> = counts.values().copied().collect();
    let ids: BTreeMap<&LearnedPattern, u16> = patterns
        .iter()
        .enumerate()
        .map(|(i, p)| (p, i as u16))
        .collect();
    let mut adjacency = BTreeSet::new();
    for y in 0..h {
        for x in 0..w {
            let Some(a) = &cell_pattern[y * w + x] else {
                continue;
            };
            for (d, dx, dy) in [(0_u8, 0_i32, 1_i32), (1, 1, 0), (2, 0, -1), (3, -1, 0)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx >= 0
                    && ny >= 0
                    && (nx as usize) < w
                    && (ny as usize) < h
                    && let Some(b) = &cell_pattern[ny as usize * w + nx as usize]
                {
                    adjacency.insert((ids[a], d, ids[b]));
                }
            }
        }
    }
    let catalog = LearnedCatalog {
        format: "rustosrs-learned-catalog".into(),
        version: 2,
        style: prefab.name.clone(),
        role: "building".into(),
        source: prefab.name.clone(),
        patterns: patterns.clone(),
        weights,
        adjacency: adjacency.into_iter().collect(),
    };
    catalog.compile().map_err(|e| e.to_string())?;
    let mut kinds: Vec<LensKind> = patterns
        .iter()
        .enumerate()
        .map(|(i, p)| LensKind {
            id: i as u16,
            count: catalog.weights[i],
            underlay: p.floor_class >> 16,
            overlay: p.floor_class & 0xffff,
            edges: p.sockets.map(LensEdge::from_socket),
            beside: [vec![], vec![], vec![], vec![]],
        })
        .collect();
    for &(a, d, b) in &catalog.adjacency {
        kinds[usize::from(a)].beside[usize::from(d)].push(b);
    }
    let cells = cell_pattern
        .iter()
        .map(|p| p.as_ref().map(|p| ids[p]))
        .collect();
    Ok(Lens {
        width: prefab.width,
        height: prefab.height,
        plane,
        cells,
        kinds,
        catalog,
        ignored,
        floors,
        doors,
        walls,
        whole_rectangle,
    })
}
